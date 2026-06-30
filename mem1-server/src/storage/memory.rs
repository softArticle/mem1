//! Memory storage trait and SurrealDB implementation (T010).
//! Search supports hybrid retrieval: FULLTEXT (keyword) + vector in parallel, merged with RRF.
//! Optional temporal validity (metadata valid_at/invalid_at) inspired by Zep/Graphiti.

use crate::error::Error;
use crate::memory::model::Memory;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use surrealdb::RecordId;
use uuid::Uuid;

use super::db::Db;

/// Returns true if the memory is considered valid at `now`. If metadata contains
/// "valid_at" / "invalid_at" (RFC3339), only memories within [valid_at, invalid_at) are valid.
/// Inspired by Zep/Graphiti temporal fact validity.
fn is_valid_at(mem: &Memory, now: &DateTime<Utc>) -> bool {
    let valid_from = mem.metadata.get("valid_at").and_then(|v| v.as_str());
    let valid_until = mem.metadata.get("invalid_at").and_then(|v| v.as_str());
    if let Some(s) = valid_from {
        if let Ok(t) = DateTime::parse_from_rfc3339(s) {
            let t = t.with_timezone(&Utc);
            if now < &t {
                return false;
            }
        }
    }
    if let Some(s) = valid_until {
        if let Ok(t) = DateTime::parse_from_rfc3339(s) {
            let t = t.with_timezone(&Utc);
            if now >= &t {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
fn metadata_matches(mem: &Memory, scope: Option<&str>, memory_type: Option<&str>) -> bool {
    fn value_matches(
        metadata: &HashMap<String, serde_json::Value>,
        key: &str,
        expected: Option<&str>,
    ) -> bool {
        let Some(expected) = expected else {
            return true;
        };
        metadata
            .get(key)
            .and_then(|v| v.as_str())
            .is_some_and(|actual| actual == expected)
    }

    value_matches(&mem.metadata, "scope", scope)
        && value_matches(&mem.metadata, "memory_type", memory_type)
}

/// Metadata filters shared by list/search/delete-all. `user_id` remains a first-class
/// storage argument because reads and destructive operations are scoped by user.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MemoryFilters {
    pub metadata: HashMap<String, String>,
}

impl MemoryFilters {
    pub fn from_scope_type(scope: Option<&str>, memory_type: Option<&str>) -> Self {
        let mut filters = Self::default();
        if let Some(scope) = scope {
            filters
                .metadata
                .insert("scope".to_string(), scope.to_string());
        }
        if let Some(memory_type) = memory_type {
            filters
                .metadata
                .insert("memory_type".to_string(), memory_type.to_string());
        }
        filters
    }

    fn matches(&self, mem: &Memory) -> bool {
        self.metadata.iter().all(|(key, expected)| {
            mem.metadata
                .get(key)
                .and_then(|v| v.as_str())
                .is_some_and(|actual| actual == expected)
        })
    }
}

/// RRF constant (reciprocal rank fusion). score = 1/(k + rank).
/// mem0 LOCOMO eval uses top_k=30 (we use same limit from client); mem0 has no RRF (vector + optional reranker).
///
/// Defaults are tuned for the LLM-extraction line (atomic facts): RRF_K=60 and an
/// active graph branch maximize LOCOMO here. The rule-v2 line (whole-message facts)
/// prefers RRF_K=20 with the graph branch off; both are reachable at runtime via
/// MEM1_RRF_K / MEM1_RRF_GRAPH_WEIGHT so a single binary serves either extraction mode.
const DEFAULT_RRF_K: u32 = 60;
const RRF_KEYWORD_WEIGHT: f32 = 1.0;
const RRF_VECTOR_WEIGHT: f32 = 1.0;
const DEFAULT_RRF_GRAPH_WEIGHT: f32 = 1.0;

fn rrf_k() -> u32 {
    std::env::var("MEM1_RRF_K")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_RRF_K)
}

fn rrf_graph_weight() -> f32 {
    std::env::var("MEM1_RRF_GRAPH_WEIGHT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_RRF_GRAPH_WEIGHT)
}
const MAX_GRAPH_ENTITIES_PER_MEMORY: usize = 16;
const MAX_GRAPH_SEEDS: usize = 8;

/// Build a shorter keyword query from the longest 2 terms (len >= 2), so FTS AND-semantics
/// can match when the full query has stopwords like "what does" that are not in the document.
fn significant_terms(query: &str) -> String {
    let mut words: Vec<&str> = query
        .split(|c: char| !c.is_alphabetic())
        .filter(|w| w.len() >= 2)
        .collect();
    words.sort_by_key(|w: &&str| std::cmp::Reverse(w.len()));
    words.truncate(2);
    words.join(" ")
}

fn normalize_entity(name: &str) -> String {
    name.split(|c: char| !c.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| part.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_query_stopword(normalized: &str) -> bool {
    crate::memory::stopwords::is_stopword(normalized)
}

fn is_acronym_token(token: &str) -> bool {
    let mut upper_count = 0;
    let mut has_alpha = false;
    for c in token.chars() {
        if c.is_ascii_alphabetic() {
            has_alpha = true;
            if c.is_ascii_uppercase() {
                upper_count += 1;
            } else {
                return false;
            }
        } else if !c.is_ascii_digit() {
            return false;
        }
    }
    has_alpha && upper_count >= 2 && token.len() <= 12
}

fn is_entity_token(token: &str) -> bool {
    let mut chars = token.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if token.len() < 2 || is_query_stopword(&token.to_ascii_lowercase()) {
        return false;
    }
    (first.is_uppercase() && chars.any(|c| c.is_lowercase())) || is_acronym_token(token)
}

fn content_without_speaker_prefix(content: &str) -> &str {
    let Some((prefix, rest)) = content.split_once(':') else {
        return content;
    };
    let parts: Vec<&str> = prefix.split_whitespace().collect();
    if parts.is_empty() || parts.len() > 3 || prefix.len() > 40 {
        return content;
    }
    if parts.iter().all(|part| {
        let token = part.trim_matches(|c: char| !c.is_alphanumeric());
        is_entity_token(token)
    }) {
        rest
    } else {
        content
    }
}

fn extract_graph_entities(content: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    for raw in content_without_speaker_prefix(content).split_whitespace() {
        let token = raw.trim_matches(|c: char| !c.is_alphanumeric());
        if is_entity_token(token) {
            seen.insert(token.to_string());
        }
        if seen.len() >= MAX_GRAPH_ENTITIES_PER_MEMORY {
            break;
        }
    }
    seen.into_iter().collect()
}

fn query_entity_terms(query: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    for entity in extract_graph_entities(query) {
        let normalized = normalize_entity(&entity);
        if !normalized.is_empty() {
            seen.insert(normalized);
        }
    }
    for raw in query.split(|c: char| !c.is_alphanumeric()) {
        let normalized = normalize_entity(raw);
        if normalized.len() >= 3 && !is_query_stopword(&normalized) {
            seen.insert(normalized);
        }
        if seen.len() >= MAX_GRAPH_ENTITIES_PER_MEMORY {
            break;
        }
    }
    seen.into_iter().collect()
}

/// Abstraction for memory persistence (add, get, search).
#[async_trait]
pub trait MemoryStore: Send + Sync {
    async fn add(&self, memory: &Memory) -> Result<Memory, Error>;
    async fn get(&self, id: &str, user_id: &str) -> Result<Option<Memory>, Error>;
    async fn update(
        &self,
        id: &str,
        user_id: &str,
        content: Option<String>,
        embedding: Option<Vec<f32>>,
        metadata: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<Option<Memory>, Error>;
    async fn delete(&self, id: &str, user_id: &str) -> Result<bool, Error>;
    async fn delete_all(&self, user_id: &str, filters: &MemoryFilters) -> Result<u64, Error>;
    async fn history(&self, id: &str, user_id: &str) -> Result<Vec<MemoryHistory>, Error>;
    async fn list_users(&self) -> Result<Vec<String>, Error>;
    async fn reset(&self) -> Result<u64, Error>;
    async fn search(
        &self,
        user_id: &str,
        query: &str,
        query_embedding: Option<Vec<f32>>,
        limit: u32,
        filters: &MemoryFilters,
    ) -> Result<Vec<(Memory, Option<f32>)>, Error>;
    async fn list_by_user(
        &self,
        user_id: &str,
        limit: u32,
        offset: u32,
        filters: &MemoryFilters,
    ) -> Result<Vec<Memory>, Error>;
}

/// SurrealDB-backed memory store (embedded).
pub struct SurrealMemoryStore(pub Db);

#[derive(Serialize, Deserialize)]
struct MemoryRecord {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<RecordId>,
    content: String,
    user_id: String,
    embedding: Option<Vec<f32>>,
    metadata: HashMap<String, serde_json::Value>,
    created_at: String,
    updated_at: String,
}

#[derive(Deserialize)]
struct SearchRow {
    id: Option<RecordId>,
    content: String,
    user_id: String,
    embedding: Option<Vec<f32>>,
    metadata: HashMap<String, serde_json::Value>,
    created_at: String,
    updated_at: String,
    #[allow(dead_code)]
    score: Option<f64>,
}

#[derive(Deserialize)]
struct IdRow {
    id: Option<RecordId>,
}

#[derive(Deserialize)]
struct EntityCountRow {
    entity_normalized: String,
    #[allow(dead_code)]
    n: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryHistory {
    pub id: String,
    pub memory_id: String,
    pub user_id: String,
    pub operation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<Memory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<Memory>,
    pub created_at: String,
}

#[derive(Serialize, Deserialize)]
struct MemoryHistoryRecord {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<RecordId>,
    memory_id: String,
    user_id: String,
    operation: String,
    previous: Option<Memory>,
    current: Option<Memory>,
    created_at: String,
}

#[derive(Serialize, Deserialize)]
struct GraphEntityRecord {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<RecordId>,
    user_id: String,
    name: String,
    normalized: String,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize, Deserialize)]
struct MemoryEntityRecord {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<RecordId>,
    user_id: String,
    memory_id: String,
    entity_id: String,
    entity_name: String,
    entity_normalized: String,
    created_at: String,
}

fn strip_backticks(s: &str) -> &str {
    s.trim().trim_matches('`')
}

/// Cosine similarity between two vectors (assumes same length). Returns 0 if empty or zero norm.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na < 1e-12 || nb < 1e-12 {
        return 0.0;
    }
    dot / (na * nb)
}

fn record_id_to_string_for_table(rid: &Option<RecordId>, table: &str, fallback: &str) -> String {
    rid.as_ref()
        .map(|r| {
            let s = r.to_string();
            let s = strip_backticks(&s);
            let prefix = format!("{table}:");
            let s = s.strip_prefix(prefix.as_str()).unwrap_or(s);
            strip_backticks(s).to_string()
        })
        .unwrap_or_else(|| fallback.to_string())
}

fn record_id_to_string(rid: &Option<RecordId>, fallback: &str) -> String {
    record_id_to_string_for_table(rid, "memories", fallback)
}

/// Fetch limit for each branch before RRF (take more then merge to top limit).
fn fetch_limit_for_rrf(limit: u32) -> u32 {
    (limit * 2).min(200)
}

/// RRF merge: combine keyword and vector ranked lists by Reciprocal Rank Fusion.
/// Returns top `limit` unique memories sorted by RRF score desc, with score in second element.
fn rrf_merge(
    kw_list: Vec<(String, Memory)>,
    vec_list: Vec<(String, Memory)>,
    graph_list: Vec<(String, Memory)>,
    limit: u32,
) -> Vec<(Memory, Option<f32>)> {
    let k = rrf_k() as f32;
    let mut scores: HashMap<String, f32> = HashMap::new();
    let mut memories: HashMap<String, Memory> = HashMap::new();
    for (weight, list) in [
        (RRF_KEYWORD_WEIGHT, kw_list),
        (RRF_VECTOR_WEIGHT, vec_list),
        (rrf_graph_weight(), graph_list),
    ] {
        for (rank_one_based, (id, mem)) in list.into_iter().enumerate() {
            let r = (rank_one_based + 1) as f32;
            *scores.entry(id.clone()).or_default() += weight / (k + r);
            memories.entry(id).or_insert(mem);
        }
    }
    let mut out: Vec<(String, f32)> = scores.into_iter().collect();
    out.sort_by(|a, b| {
        let by_score = b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal);
        if by_score != std::cmp::Ordering::Equal {
            return by_score;
        }
        // Tie-break: prefer newer memories (helps temporal recency)
        let ca = memories
            .get(&a.0)
            .map(|m| m.created_at.as_str())
            .unwrap_or("");
        let cb = memories
            .get(&b.0)
            .map(|m| m.created_at.as_str())
            .unwrap_or("");
        cb.cmp(ca)
    });
    out.into_iter()
        .take(limit as usize)
        .filter_map(|(id, score)| memories.remove(&id).map(|mem| (mem, Some(score))))
        .collect()
}

impl SurrealMemoryStore {
    fn id_trim(id: &str) -> Result<&str, Error> {
        let id = strip_backticks(id);
        let id = id.strip_prefix("memories:").unwrap_or(id);
        let id = strip_backticks(id);
        if id.is_empty() {
            return Err(Error::InvalidInput("id is required".to_string()));
        }
        if !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(Error::InvalidInput("invalid id".to_string()));
        }
        Ok(id)
    }

    fn to_record(m: &Memory) -> MemoryRecord {
        MemoryRecord {
            id: None,
            content: m.content.clone(),
            user_id: m.user_id.clone(),
            embedding: m.embedding.clone(),
            metadata: m.metadata.clone(),
            created_at: m.created_at.clone(),
            updated_at: m.updated_at.clone(),
        }
    }

    fn from_record(id: String, r: MemoryRecord) -> Memory {
        Memory {
            id,
            content: r.content,
            user_id: r.user_id,
            embedding: r.embedding,
            metadata: r.metadata,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }

    fn search_row_to_memory(r: SearchRow) -> (String, Memory) {
        let id = record_id_to_string(&r.id, "");
        let mem = MemoryRecord {
            id: r.id,
            content: r.content,
            user_id: r.user_id,
            embedding: r.embedding,
            metadata: r.metadata,
            created_at: r.created_at,
            updated_at: r.updated_at,
        };
        (id.clone(), Self::from_record(id, mem))
    }

    fn history_record_to_history(r: MemoryHistoryRecord) -> MemoryHistory {
        let id =
            r.id.as_ref()
                .map(|rid| {
                    let s = rid.to_string();
                    let s = strip_backticks(&s);
                    let s = s.strip_prefix("memory_history:").unwrap_or(s);
                    strip_backticks(s).to_string()
                })
                .unwrap_or_default();
        MemoryHistory {
            id,
            memory_id: r.memory_id,
            user_id: r.user_id,
            operation: r.operation,
            previous: r.previous,
            current: r.current,
            created_at: r.created_at,
        }
    }

    async fn record_history(
        &self,
        memory_id: &str,
        user_id: &str,
        operation: &str,
        previous: Option<&Memory>,
        current: Option<&Memory>,
    ) -> Result<(), Error> {
        let now = Utc::now().to_rfc3339();
        let record = MemoryHistoryRecord {
            id: None,
            memory_id: memory_id.to_string(),
            user_id: user_id.to_string(),
            operation: operation.to_string(),
            previous: previous.cloned(),
            current: current.cloned(),
            created_at: now,
        };
        let _: Option<MemoryHistoryRecord> = self
            .0
            .create(("memory_history", Uuid::new_v4().to_string()))
            .content(record)
            .await
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb history create: {e}")))?;
        Ok(())
    }

    async fn list_all_by_user(
        &self,
        user_id: &str,
        filters: &MemoryFilters,
    ) -> Result<Vec<Memory>, Error> {
        let sql = "SELECT * FROM memories WHERE user_id = $user_id ORDER BY created_at DESC";
        let mut response = self
            .0
            .query(sql)
            .bind(("user_id", user_id.to_string()))
            .await
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb list query: {e}")))?;
        let rows: Vec<SearchRow> = response
            .take(0)
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb take: {e}")))?;
        let now = Utc::now();
        Ok(rows
            .into_iter()
            .map(Self::search_row_to_memory)
            .map(|(_, mem)| mem)
            .filter(|m| is_valid_at(m, &now) && filters.matches(m))
            .collect())
    }

    async fn list_all_memories(&self) -> Result<Vec<Memory>, Error> {
        let mut response = self
            .0
            .query("SELECT * FROM memories")
            .await
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb list all query: {e}")))?;
        let rows: Vec<SearchRow> = response
            .take(0)
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb take: {e}")))?;
        Ok(rows
            .into_iter()
            .map(Self::search_row_to_memory)
            .map(|(_, mem)| mem)
            .collect())
    }

    async fn clear_memory_graph_edges(&self, memory_id: &str) -> Result<(), Error> {
        self.0
            .query("DELETE memory_entities WHERE memory_id = $memory_id")
            .bind(("memory_id", memory_id.to_string()))
            .await
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb graph edge delete: {e}")))?;
        Ok(())
    }

    async fn ensure_graph_entity(
        &self,
        user_id: &str,
        name: &str,
        normalized: &str,
    ) -> Result<String, Error> {
        let mut response = self
            .0
            .query(
                "SELECT * FROM graph_entities \
                 WHERE user_id = $user_id AND normalized = $normalized LIMIT 1",
            )
            .bind(("user_id", user_id.to_string()))
            .bind(("normalized", normalized.to_string()))
            .await
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb graph entity query: {e}")))?;
        let rows: Vec<GraphEntityRecord> = response
            .take(0)
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb graph entity take: {e}")))?;
        if let Some(row) = rows.into_iter().next() {
            return Ok(record_id_to_string_for_table(&row.id, "graph_entities", ""));
        }

        let now = Utc::now().to_rfc3339();
        let record = GraphEntityRecord {
            id: None,
            user_id: user_id.to_string(),
            name: name.to_string(),
            normalized: normalized.to_string(),
            created_at: now.clone(),
            updated_at: now,
        };
        let created: Option<GraphEntityRecord> = self
            .0
            .create(("graph_entities", Uuid::new_v4().to_string()))
            .content(record)
            .await
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb graph entity create: {e}")))?;
        let created = created.ok_or_else(|| {
            Error::Storage(anyhow::anyhow!(
                "surrealdb graph entity create: no record returned"
            ))
        })?;
        Ok(record_id_to_string_for_table(
            &created.id,
            "graph_entities",
            "",
        ))
    }

    async fn index_memory_graph(&self, memory: &Memory) -> Result<(), Error> {
        self.clear_memory_graph_edges(&memory.id).await?;
        let entities = extract_graph_entities(&memory.content);
        let now = Utc::now().to_rfc3339();
        for entity in entities {
            let normalized = normalize_entity(&entity);
            if normalized.is_empty() {
                continue;
            }
            let entity_id = self
                .ensure_graph_entity(&memory.user_id, &entity, &normalized)
                .await?;
            let edge = MemoryEntityRecord {
                id: None,
                user_id: memory.user_id.clone(),
                memory_id: memory.id.clone(),
                entity_id,
                entity_name: entity,
                entity_normalized: normalized,
                created_at: now.clone(),
            };
            let _: Option<MemoryEntityRecord> = self
                .0
                .create(("memory_entities", Uuid::new_v4().to_string()))
                .content(edge)
                .await
                .map_err(|e| {
                    Error::Storage(anyhow::anyhow!("surrealdb memory entity create: {e}"))
                })?;
        }
        Ok(())
    }

    async fn entity_ids_for_memory(
        &self,
        user_id: &str,
        memory_id: &str,
    ) -> Result<Vec<String>, Error> {
        let mut response = self
            .0
            .query(
                "SELECT * FROM memory_entities WHERE user_id = $user_id AND memory_id = $memory_id",
            )
            .bind(("user_id", user_id.to_string()))
            .bind(("memory_id", memory_id.to_string()))
            .await
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb memory entity query: {e}")))?;
        let rows: Vec<MemoryEntityRecord> = response
            .take(0)
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb memory entity take: {e}")))?;
        Ok(rows.into_iter().map(|row| row.entity_id).collect())
    }

    /// Names that prefix many stored "Name: ..." lines are conversation speakers;
    /// they recur in nearly every memory and carry little discriminating signal for
    /// ranking, so the graph scorer treats them as low-information overlap terms.
    async fn frequent_speaker_names(&self, user_id: &str) -> Result<Vec<String>, Error> {
        let mut response = self
            .0
            .query(
                "SELECT entity_normalized, count() AS n FROM memory_entities \
                 WHERE user_id = $user_id GROUP BY entity_normalized ORDER BY n DESC LIMIT 6",
            )
            .bind(("user_id", user_id.to_string()))
            .await
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb speaker names query: {e}")))?;
        let rows: Vec<EntityCountRow> = response
            .take(0)
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb speaker names take: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|r| r.entity_normalized)
            .filter(|n| !n.is_empty())
            .collect())
    }

    async fn entity_ids_for_query(&self, user_id: &str, query: &str) -> Result<Vec<String>, Error> {
        let mut ids = Vec::new();
        for normalized in query_entity_terms(query) {
            let mut response = self
                .0
                .query(
                    "SELECT * FROM graph_entities \
                     WHERE user_id = $user_id AND normalized = $normalized LIMIT 1",
                )
                .bind(("user_id", user_id.to_string()))
                .bind(("normalized", normalized))
                .await
                .map_err(|e| {
                    Error::Storage(anyhow::anyhow!("surrealdb graph query entity: {e}"))
                })?;
            let rows: Vec<GraphEntityRecord> = response.take(0).map_err(|e| {
                Error::Storage(anyhow::anyhow!("surrealdb graph query entity take: {e}"))
            })?;
            if let Some(row) = rows.into_iter().next() {
                ids.push(record_id_to_string_for_table(&row.id, "graph_entities", ""));
            }
        }
        Ok(ids)
    }

    async fn search_graph_raw(
        &self,
        user_id: &str,
        query: &str,
        seed_ids: &[String],
        fetch_limit: u32,
        filters: &MemoryFilters,
        now: &DateTime<Utc>,
    ) -> Result<Vec<(String, Memory)>, Error> {
        // Entities mentioned by the query carry the user's intent; entities shared
        // with seed (keyword/vector) hits widen the relational neighborhood.
        let query_entity_ids: BTreeSet<String> = self
            .entity_ids_for_query(user_id, query)
            .await?
            .into_iter()
            .collect();
        let mut entity_ids: BTreeSet<String> = query_entity_ids.clone();
        for seed_id in seed_ids.iter().take(MAX_GRAPH_SEEDS) {
            for entity_id in self.entity_ids_for_memory(user_id, seed_id).await? {
                entity_ids.insert(entity_id);
            }
        }

        // Collect candidate memory ids and how many distinct query-intent entities
        // each one is linked to. A memory that connects to several of the query's
        // entities (e.g. "Caroline" + "grandma" + "Sweden") is far more relevant
        // than one that merely shares a single high-frequency entity.
        let per_entity = fetch_limit.saturating_mul(4).max(fetch_limit);
        let mut query_entity_hits: HashMap<String, u32> = HashMap::new();
        let mut candidate_ids: Vec<String> = Vec::new();
        let mut seen_candidate: HashSet<String> = HashSet::new();
        // Candidates linked only through an entity shared with a seed hit (not a
        // query entity, no query-term overlap) are legitimate graph-context
        // expansions — they answer "what else is connected to this result". Keep
        // them even when they don't match the query text directly.
        let mut seed_linked: HashSet<String> = HashSet::new();
        for entity_id in &entity_ids {
            let is_query_entity = query_entity_ids.contains(entity_id);
            let mut response = self
                .0
                .query(
                    "SELECT * FROM memory_entities \
                     WHERE user_id = $user_id AND entity_id = $entity_id \
                     ORDER BY created_at DESC LIMIT $limit",
                )
                .bind(("user_id", user_id.to_string()))
                .bind(("entity_id", entity_id.clone()))
                .bind(("limit", per_entity))
                .await
                .map_err(|e| {
                    Error::Storage(anyhow::anyhow!("surrealdb graph candidate query: {e}"))
                })?;
            let rows: Vec<MemoryEntityRecord> = response.take(0).map_err(|e| {
                Error::Storage(anyhow::anyhow!("surrealdb graph candidate take: {e}"))
            })?;
            for row in rows {
                if is_query_entity {
                    *query_entity_hits.entry(row.memory_id.clone()).or_default() += 1;
                } else {
                    seed_linked.insert(row.memory_id.clone());
                }
                if seen_candidate.insert(row.memory_id.clone()) {
                    candidate_ids.push(row.memory_id);
                }
            }
        }

        // Multi-hop BFS over the entity-event graph (SAG-style). The 1-hop
        // collection above only reaches memories sharing an entity with the query
        // or a seed; multi-hop questions whose answer is scattered A->B->C need
        // further traversal. Behind MEM1_GRAPH_HOPS (default 1 == no change): for
        // each extra hop, take the memories gathered so far (the frontier), pull
        // their entities, and find NEW memories linked to those entities, marking
        // them seed_linked so they flow through the existing scorer. Bounded: skip
        // already-processed entities and cap memories added per hop.
        let graph_hops: u32 = std::env::var("MEM1_GRAPH_HOPS")
            .ok()
            .and_then(|v| v.trim().parse::<u32>().ok())
            .unwrap_or(1)
            .max(1);
        if graph_hops > 1 {
            let mut processed_entities: HashSet<String> = entity_ids.iter().cloned().collect();
            // Bound the frontier to the strongest candidates per hop (like the 1-hop
            // seed cap) so traversal stays cheap — expanding from every candidate is
            // O(candidates x entities) DB queries and explodes on dense graphs.
            let mut frontier: Vec<String> = candidate_ids
                .iter()
                .take(MAX_GRAPH_SEEDS)
                .cloned()
                .collect();
            for _ in 1..graph_hops {
                if frontier.is_empty() {
                    break;
                }
                let mut hop_entities: Vec<String> = Vec::new();
                for memory_id in &frontier {
                    for entity_id in self.entity_ids_for_memory(user_id, memory_id).await? {
                        if processed_entities.insert(entity_id.clone()) {
                            hop_entities.push(entity_id);
                        }
                    }
                }
                let mut next_frontier: Vec<String> = Vec::new();
                for entity_id in hop_entities {
                    let mut response = self
                        .0
                        .query(
                            "SELECT * FROM memory_entities \
                             WHERE user_id = $user_id AND entity_id = $entity_id \
                             ORDER BY created_at DESC LIMIT $limit",
                        )
                        .bind(("user_id", user_id.to_string()))
                        .bind(("entity_id", entity_id.clone()))
                        .bind(("limit", per_entity))
                        .await
                        .map_err(|e| {
                            Error::Storage(anyhow::anyhow!("surrealdb graph hop query: {e}"))
                        })?;
                    let rows: Vec<MemoryEntityRecord> = response.take(0).map_err(|e| {
                        Error::Storage(anyhow::anyhow!("surrealdb graph hop take: {e}"))
                    })?;
                    for row in rows {
                        if seen_candidate.insert(row.memory_id.clone()) {
                            seed_linked.insert(row.memory_id.clone());
                            candidate_ids.push(row.memory_id.clone());
                            next_frontier.push(row.memory_id);
                        }
                    }
                }
                next_frontier.truncate(MAX_GRAPH_SEEDS);
                frontier = next_frontier;
            }
        }

        // Entity extraction only recognizes capitalized proper nouns, so a question
        // like "what country is Caroline's grandma from" never links to the memory
        // "...grandma in my home country, Sweden" via entities alone. Widen the
        // graph candidate pool with a per-term FULLTEXT scan: SurrealDB's `@@` is
        // AND across a multi-word query (so the full question matches nothing), but
        // searching each significant term independently (OR semantics) surfaces the
        // memories that mention any intent word, which the scorer below then ranks.
        let query_terms: BTreeSet<String> = significant_terms(query)
            .split_whitespace()
            .map(|t| t.to_string())
            .collect();
        let intent_terms: Vec<String> = query
            .split(|c: char| !c.is_alphanumeric())
            .map(|w| w.to_ascii_lowercase())
            .filter(|w| w.len() >= 3 && !is_query_stopword(w))
            .collect();
        for term in &intent_terms {
            let mut response = self
                .0
                .query(
                    "SELECT id FROM memories \
                     WHERE user_id = $user_id AND content @@ $term LIMIT $limit",
                )
                .bind(("user_id", user_id.to_string()))
                .bind(("term", term.clone()))
                .bind(("limit", per_entity))
                .await
                .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb graph term query: {e}")))?;
            let rows: Vec<IdRow> = response
                .take(0)
                .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb graph term take: {e}")))?;
            for row in rows {
                let mid = record_id_to_string(&row.id, "");
                if mid.is_empty() {
                    continue;
                }
                if seen_candidate.insert(mid.clone()) {
                    candidate_ids.push(mid);
                }
            }
        }

        // Speaker names (the user's own name from `{speaker}_{idx}`, plus any name
        // that prefixes a stored "Name: ..." line) appear in almost every memory of
        // a conversation, so matching them is nearly free signal. Treat them as
        // low-information for the overlap score: content words like "grandma",
        // "camped", "moved" should drive ranking, not the omnipresent speaker names.
        let speaker_name = user_id
            .rsplit_once('_')
            .map(|(name, _)| name)
            .unwrap_or(user_id)
            .to_ascii_lowercase();
        let low_info_terms: BTreeSet<String> = self
            .frequent_speaker_names(user_id)
            .await?
            .into_iter()
            .chain(std::iter::once(speaker_name))
            .collect();

        // Fetch candidates, score by (distinct query entities matched, content-word
        // overlap), and return the strongest first instead of newest-first.
        let mut scored: Vec<(u32, u32, String, Memory)> = Vec::new();
        for memory_id in candidate_ids {
            if let Some(mem) = self.get(&memory_id, user_id).await? {
                if is_valid_at(&mem, now) && filters.matches(&mem) {
                    let entity_score = query_entity_hits.get(&memory_id).copied().unwrap_or(0);
                    let lower = mem.content.to_lowercase();
                    let content_terms = query_terms
                        .iter()
                        .map(|s| s.as_str())
                        .chain(intent_terms.iter().map(|s| s.as_str()))
                        .filter(|t| !low_info_terms.contains(*t));
                    let mut seen_term: BTreeSet<&str> = BTreeSet::new();
                    let mut overlap = 0u32;
                    for t in content_terms {
                        if seen_term.insert(t) && lower.contains(t) {
                            overlap += 1;
                        }
                    }
                    // The graph branch should only contribute memories that actually
                    // connect to the query — via a query-intent entity or a content
                    // word. A candidate matching neither is a high-frequency-entity
                    // coattail (e.g. "Caroline: Wow!") that dilutes ranking, so drop
                    // it. Exception: when the query names no entity at all, a seed-
                    // linked neighbor IS the graph-expansion signal (e.g. query
                    // "passport" pulling in a hotel memory sharing entity "Alice"),
                    // so keep it.
                    let keep_seed_expansion =
                        seed_linked.contains(&memory_id) && query_entity_ids.is_empty();
                    if entity_score == 0 && overlap == 0 && !keep_seed_expansion {
                        continue;
                    }
                    scored.push((entity_score, overlap, mem.id.clone(), mem));
                }
            }
        }
        scored.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then(b.0.cmp(&a.0))
                .then_with(|| b.3.created_at.cmp(&a.3.created_at))
        });
        Ok(scored
            .into_iter()
            .take(fetch_limit as usize)
            .map(|(_, _, id, mem)| (id, mem))
            .collect())
    }

    /// Keyword path: FULLTEXT on content with search::score, ordered by score desc.
    async fn search_keyword_raw(
        db: &Db,
        user_id: &str,
        query: &str,
        fetch_limit: u32,
    ) -> Result<Vec<(String, Memory)>, Error> {
        let sql = "SELECT *, search::score(0) AS score FROM memories \
                   WHERE user_id = $user_id AND content @@ $query \
                   ORDER BY score DESC LIMIT $limit";
        let mut response = db
            .query(sql)
            .bind(("user_id", user_id.to_string()))
            .bind(("query", query.to_string()))
            .bind(("limit", fetch_limit))
            .await
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb keyword query: {e}")))?;
        let rows: Vec<SearchRow> = response
            .take(0)
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb take: {e}")))?;
        Ok(rows.into_iter().map(Self::search_row_to_memory).collect())
    }

    /// Vector path: fetch candidates with embedding, compute cosine in Rust, then sort and take top-k.
    /// (SurrealDB KNN with bound $qvec can return empty; this brute-force path is reliable.)
    async fn search_vector_raw(
        db: &Db,
        user_id: &str,
        qvec: Vec<f32>,
        fetch_limit: u32,
    ) -> Result<Vec<(String, Memory)>, Error> {
        let sql = "SELECT * FROM memories WHERE user_id = $user_id AND embedding != NONE";
        let mut response = db
            .query(sql)
            .bind(("user_id", user_id.to_string()))
            .await
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb vector query: {e}")))?;
        let rows: Vec<SearchRow> = response
            .take(0)
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb take: {e}")))?;

        let mut with_scores: Vec<(f32, String, Memory)> = rows
            .into_iter()
            .filter_map(|r| {
                let emb = r.embedding.as_ref()?;
                let score = cosine_similarity(emb, &qvec);
                let (id, mem) = Self::search_row_to_memory(r);
                Some((score, id, mem))
            })
            .collect();
        with_scores.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        Ok(with_scores
            .into_iter()
            .take(fetch_limit as usize)
            .map(|(_, id, mem)| (id, mem))
            .collect())
    }

    /// Expands search results with related memories from metadata["related_ids"] (Zep-style graph context).
    async fn expand_with_related(
        &self,
        user_id: &str,
        mut items: Vec<(Memory, Option<f32>)>,
        limit: u32,
        now: &DateTime<Utc>,
        filters: &MemoryFilters,
    ) -> Result<Vec<(Memory, Option<f32>)>, Error> {
        let cap = limit as usize;
        if items.len() >= cap {
            return Ok(items);
        }
        let mut seen: HashSet<String> = items.iter().map(|(m, _)| m.id.clone()).collect();
        let mut related_ids: Vec<String> = Vec::new();
        for (m, _) in &items {
            if let Some(arr) = m.metadata.get("related_ids").and_then(|v| v.as_array()) {
                for v in arr {
                    if let Some(s) = v.as_str() {
                        let s = s.to_string();
                        if !seen.contains(&s) {
                            seen.insert(s.clone());
                            related_ids.push(s);
                        }
                    }
                }
            }
        }
        const MAX_RELATED: usize = 10;
        for id in related_ids.into_iter().take(MAX_RELATED) {
            if items.len() >= cap {
                break;
            }
            if let Ok(Some(mem)) = self.get(id.as_str(), user_id).await {
                if is_valid_at(&mem, now) && filters.matches(&mem) {
                    items.push((mem, None));
                }
            }
        }
        Ok(items)
    }
}

#[async_trait]
impl MemoryStore for SurrealMemoryStore {
    async fn add(&self, memory: &Memory) -> Result<Memory, Error> {
        let id = Self::id_trim(&memory.id)?;
        let record = Self::to_record(memory);
        let created: Option<MemoryRecord> =
            self.0
                .create(("memories", id))
                .content(record)
                .await
                .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb create: {e}")))?;
        let created = created.ok_or_else(|| {
            Error::Storage(anyhow::anyhow!("surrealdb create: no record returned"))
        })?;
        let out_id = record_id_to_string(&created.id, id);
        let created = Self::from_record(out_id, created);
        self.index_memory_graph(&created).await?;
        self.record_history(&created.id, &created.user_id, "ADD", None, Some(&created))
            .await?;
        Ok(created)
    }

    async fn get(&self, id: &str, user_id: &str) -> Result<Option<Memory>, Error> {
        let id_trim = Self::id_trim(id)?;
        let opt: Option<MemoryRecord> = self
            .0
            .select(("memories", id_trim))
            .await
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb select: {e}")))?;
        let Some(r) = opt else {
            return Ok(None);
        };
        if r.user_id != user_id {
            return Ok(None);
        }
        let out_id = record_id_to_string(&r.id, id_trim);
        Ok(Some(Self::from_record(out_id, r)))
    }

    async fn update(
        &self,
        id: &str,
        user_id: &str,
        content: Option<String>,
        embedding: Option<Vec<f32>>,
        metadata: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<Option<Memory>, Error> {
        let id_trim = Self::id_trim(id)?;
        let opt: Option<MemoryRecord> = self
            .0
            .select(("memories", id_trim))
            .await
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb select: {e}")))?;
        let Some(mut record) = opt else {
            return Ok(None);
        };
        if record.user_id != user_id {
            return Ok(None);
        }

        let previous_id = record_id_to_string(&record.id, id_trim);
        let previous = Self::from_record(
            previous_id.clone(),
            MemoryRecord {
                id: record.id.clone(),
                content: record.content.clone(),
                user_id: record.user_id.clone(),
                embedding: record.embedding.clone(),
                metadata: record.metadata.clone(),
                created_at: record.created_at.clone(),
                updated_at: record.updated_at.clone(),
            },
        );

        if let Some(content) = content {
            record.content = content;
            record.embedding = embedding;
        }
        if let Some(metadata) = metadata {
            record.metadata.extend(metadata);
        }
        record.updated_at = Utc::now().to_rfc3339();

        let updated: Option<MemoryRecord> = self
            .0
            .update(("memories", id_trim))
            .content(record)
            .await
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb update: {e}")))?;
        let updated = updated.ok_or_else(|| {
            Error::Storage(anyhow::anyhow!("surrealdb update: no record returned"))
        })?;
        let out_id = record_id_to_string(&updated.id, id_trim);
        let updated = Self::from_record(out_id, updated);
        self.index_memory_graph(&updated).await?;
        self.record_history(
            &updated.id,
            &updated.user_id,
            "UPDATE",
            Some(&previous),
            Some(&updated),
        )
        .await?;
        Ok(Some(updated))
    }

    async fn delete(&self, id: &str, user_id: &str) -> Result<bool, Error> {
        let id_trim = Self::id_trim(id)?;
        let opt: Option<MemoryRecord> = self
            .0
            .select(("memories", id_trim))
            .await
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb select: {e}")))?;
        let Some(r) = opt else {
            return Ok(false);
        };
        if r.user_id != user_id {
            return Ok(false);
        }
        let out_id = record_id_to_string(&r.id, id_trim);
        let deleted = Self::from_record(out_id, r);
        self.record_history(
            &deleted.id,
            &deleted.user_id,
            "DELETE",
            Some(&deleted),
            None,
        )
        .await?;
        self.clear_memory_graph_edges(&deleted.id).await?;
        let _: Option<MemoryRecord> = self
            .0
            .delete(("memories", id_trim))
            .await
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb delete: {e}")))?;
        Ok(true)
    }

    async fn delete_all(&self, user_id: &str, filters: &MemoryFilters) -> Result<u64, Error> {
        let memories = self.list_all_by_user(user_id, filters).await?;
        let mut deleted = 0;
        for memory in memories {
            if self.delete(&memory.id, user_id).await? {
                deleted += 1;
            }
        }
        Ok(deleted)
    }

    async fn history(&self, id: &str, user_id: &str) -> Result<Vec<MemoryHistory>, Error> {
        let id_trim = Self::id_trim(id)?;
        let sql = "SELECT * FROM memory_history \
                   WHERE memory_id = $memory_id AND user_id = $user_id \
                   ORDER BY created_at ASC";
        let mut response = self
            .0
            .query(sql)
            .bind(("memory_id", id_trim.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb history query: {e}")))?;
        let rows: Vec<MemoryHistoryRecord> = response
            .take(0)
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb history take: {e}")))?;
        Ok(rows
            .into_iter()
            .map(Self::history_record_to_history)
            .collect())
    }

    async fn list_users(&self) -> Result<Vec<String>, Error> {
        let users: BTreeSet<String> = self
            .list_all_memories()
            .await?
            .into_iter()
            .map(|m| m.user_id)
            .collect();
        Ok(users.into_iter().collect())
    }

    async fn reset(&self) -> Result<u64, Error> {
        let memories = self.list_all_memories().await?;
        let deleted = memories.len() as u64;
        for memory in memories {
            let id_trim = Self::id_trim(&memory.id)?;
            let _: Option<MemoryRecord> = self
                .0
                .delete(("memories", id_trim))
                .await
                .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb reset delete: {e}")))?;
        }
        let mut response = self
            .0
            .query("DELETE memory_history")
            .await
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb reset history: {e}")))?;
        let _: Vec<MemoryHistoryRecord> = response
            .take(0)
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb reset history take: {e}")))?;
        self.0
            .query("DELETE memory_entities")
            .await
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb reset graph edges: {e}")))?;
        self.0
            .query("DELETE graph_entities")
            .await
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb reset graph entities: {e}")))?;
        Ok(deleted)
    }

    async fn search(
        &self,
        user_id: &str,
        query: &str,
        query_embedding: Option<Vec<f32>>,
        limit: u32,
        filters: &MemoryFilters,
    ) -> Result<Vec<(Memory, Option<f32>)>, Error> {
        let limit = limit.min(100);

        if let Some(qvec) = query_embedding {
            // Parallel dual-path: keyword (FULLTEXT) + vector, then RRF merge.
            let db = self.0.clone();
            let user_id = user_id.to_string();
            let query = query.to_string();
            let fetch_limit = fetch_limit_for_rrf(limit);

            let kw_fut = SurrealMemoryStore::search_keyword_raw(&db, &user_id, &query, fetch_limit);
            let vec_fut = SurrealMemoryStore::search_vector_raw(&db, &user_id, qvec, fetch_limit);

            let (kw_list, vec_list) = tokio::join!(kw_fut, vec_fut);
            let mut kw_list = kw_list?;
            let vec_list = vec_list?;
            if kw_list.is_empty() {
                let fallback = significant_terms(&query);
                if !fallback.is_empty() {
                    kw_list = SurrealMemoryStore::search_keyword_raw(
                        &db,
                        &user_id,
                        &fallback,
                        fetch_limit,
                    )
                    .await?;
                }
            }
            let now = Utc::now();
            let seed_ids = kw_list
                .iter()
                .chain(vec_list.iter())
                .map(|(id, _)| id.clone())
                .collect::<Vec<_>>();
            let graph_list = self
                .search_graph_raw(&user_id, &query, &seed_ids, fetch_limit, filters, &now)
                .await?;
            let merged = rrf_merge(kw_list, vec_list, graph_list, limit);
            let filtered: Vec<_> = merged
                .into_iter()
                .filter(|(m, _)| is_valid_at(m, &now) && filters.matches(m))
                .take(limit as usize)
                .collect();
            return self
                .expand_with_related(&user_id, filtered, limit, &now, filters)
                .await;
        }

        // No embedding: keyword-only (FULLTEXT + search::score).
        // SurrealDB FTS often requires all query terms to match (AND); if the full query
        // returns nothing, retry with a shorter query of significant terms so e.g.
        // "What does Alice prefer?" can match via "alice prefer".
        let fetch_limit = fetch_limit_for_rrf(limit);
        let mut kw_list =
            SurrealMemoryStore::search_keyword_raw(&self.0, user_id, query, fetch_limit).await?;
        if kw_list.is_empty() {
            let fallback = significant_terms(query);
            if fallback != query && !fallback.is_empty() {
                kw_list = SurrealMemoryStore::search_keyword_raw(
                    &self.0,
                    user_id,
                    &fallback,
                    fetch_limit,
                )
                .await?;
            }
        }
        let now = Utc::now();
        let seed_ids = kw_list.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>();
        let graph_list = self
            .search_graph_raw(user_id, query, &seed_ids, fetch_limit, filters, &now)
            .await?;
        let merged = rrf_merge(kw_list, Vec::new(), graph_list, limit);
        let list: Vec<_> = merged
            .into_iter()
            .filter(|(m, _)| is_valid_at(m, &now) && filters.matches(m))
            .take(limit as usize)
            .collect();
        self.expand_with_related(user_id, list, limit, &now, filters)
            .await
    }

    async fn list_by_user(
        &self,
        user_id: &str,
        limit: u32,
        offset: u32,
        filters: &MemoryFilters,
    ) -> Result<Vec<Memory>, Error> {
        let limit = limit.min(100);
        Ok(self
            .list_all_by_user(user_id, filters)
            .await?
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::metadata_matches;
    use crate::memory::model::Memory;
    use serde_json::json;
    use std::collections::HashMap;

    fn memory_with_metadata(metadata: HashMap<String, serde_json::Value>) -> Memory {
        Memory::new("content".to_string(), "u1".to_string(), metadata)
    }

    #[test]
    fn metadata_matches_scope_and_memory_type_when_filters_are_present() {
        let mut metadata = HashMap::new();
        metadata.insert("scope".to_string(), json!("project"));
        metadata.insert("memory_type".to_string(), json!("decision"));
        let memory = memory_with_metadata(metadata);

        assert!(metadata_matches(&memory, Some("project"), Some("decision")));
        assert!(!metadata_matches(
            &memory,
            Some("session"),
            Some("decision")
        ));
        assert!(!metadata_matches(&memory, Some("project"), Some("fact")));
    }

    #[test]
    fn metadata_matches_allows_absent_filters() {
        let memory = memory_with_metadata(HashMap::new());

        assert!(metadata_matches(&memory, None, None));
        assert!(!metadata_matches(&memory, Some("project"), None));
        assert!(!metadata_matches(&memory, None, Some("fact")));
    }
}
