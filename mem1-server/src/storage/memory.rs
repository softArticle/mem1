//! Memory storage trait and SurrealDB implementation (T010).
//! Search supports hybrid retrieval: FULLTEXT (keyword) + vector in parallel, merged with RRF.

use crate::error::Error;
use crate::memory::model::Memory;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use surrealdb::RecordId;

use super::db::Db;

/// RRF constant (reciprocal rank fusion). score = 1/(k + rank).
const RRF_K: u32 = 60;

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

/// Abstraction for memory persistence (add, get, search).
#[async_trait]
pub trait MemoryStore: Send + Sync {
    async fn add(&self, memory: &Memory) -> Result<Memory, Error>;
    async fn get(&self, id: &str, user_id: &str) -> Result<Option<Memory>, Error>;
    async fn delete(&self, id: &str, user_id: &str) -> Result<bool, Error>;
    async fn search(
        &self,
        user_id: &str,
        query: &str,
        query_embedding: Option<Vec<f32>>,
        limit: u32,
    ) -> Result<Vec<(Memory, Option<f32>)>, Error>;
}

/// SurrealDB-backed memory store (embedded).
pub struct SurrealMemoryStore(pub Db);

#[derive(Serialize, Deserialize)]
struct MemoryRecord {
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

fn record_id_to_string(rid: &Option<RecordId>, fallback: &str) -> String {
    rid.as_ref()
        .map(|r| {
            let s = r.to_string();
            let s = strip_backticks(&s);
            s.strip_prefix("memories:")
                .map(String::from)
                .unwrap_or_else(|| s.to_string())
        })
        .unwrap_or_else(|| fallback.to_string())
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
    limit: u32,
) -> Vec<(Memory, Option<f32>)> {
    let k = RRF_K as f32;
    let mut scores: HashMap<String, f32> = HashMap::new();
    let mut memories: HashMap<String, Memory> = HashMap::new();
    for (rank_one_based, (id, mem)) in kw_list.into_iter().enumerate() {
        let r = (rank_one_based + 1) as f32;
        *scores.entry(id.clone()).or_default() += 1.0 / (k + r);
        memories.insert(id, mem);
    }
    for (rank_one_based, (id, mem)) in vec_list.into_iter().enumerate() {
        let r = (rank_one_based + 1) as f32;
        *scores.entry(id.clone()).or_default() += 1.0 / (k + r);
        memories.entry(id).or_insert(mem);
    }
    let mut out: Vec<(String, f32)> = scores.into_iter().collect();
    out.sort_by(|a, b| {
        let by_score = b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal);
        if by_score != std::cmp::Ordering::Equal {
            return by_score;
        }
        // Tie-break: prefer newer memories (helps temporal recency)
        let ca = memories.get(&a.0).map(|m| m.created_at.as_str()).unwrap_or("");
        let cb = memories.get(&b.0).map(|m| m.created_at.as_str()).unwrap_or("");
        cb.cmp(ca)
    });
    out.into_iter()
        .take(limit as usize)
        .filter_map(|(id, score)| {
            memories.remove(&id).map(|mem| (mem, Some(score)))
        })
        .collect()
}

impl SurrealMemoryStore {
    fn id_trim(id: &str) -> Result<&str, Error> {
        let id = strip_backticks(id).trim_start_matches("memories:");
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
}

#[async_trait]
impl MemoryStore for SurrealMemoryStore {
    async fn add(&self, memory: &Memory) -> Result<Memory, Error> {
        let id = Self::id_trim(&memory.id)?;
        let record = Self::to_record(memory);
        let created: Option<MemoryRecord> = self
            .0
            .create(("memories", id))
            .content(record)
            .await
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb create: {e}")))?;
        let created = created
            .ok_or_else(|| Error::Storage(anyhow::anyhow!("surrealdb create: no record returned")))?;
        let out_id = record_id_to_string(&created.id, id);
        Ok(Self::from_record(out_id, created))
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
        let _: Option<MemoryRecord> = self
            .0
            .delete(("memories", id_trim))
            .await
            .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb delete: {e}")))?;
        Ok(true)
    }

    async fn search(
        &self,
        user_id: &str,
        query: &str,
        query_embedding: Option<Vec<f32>>,
        limit: u32,
    ) -> Result<Vec<(Memory, Option<f32>)>, Error> {
        let limit = limit.min(100);

        if let Some(qvec) = query_embedding {
            // Parallel dual-path: keyword (FULLTEXT) + vector, then RRF merge.
            let db = self.0.clone();
            let user_id = user_id.to_string();
            let query = query.to_string();
            let fetch_limit = fetch_limit_for_rrf(limit);

            let kw_fut = SurrealMemoryStore::search_keyword_raw(&db, &user_id, &query, fetch_limit);
            let vec_fut =
                SurrealMemoryStore::search_vector_raw(&db, &user_id, qvec, fetch_limit);

            let (kw_list, vec_list) = tokio::join!(kw_fut, vec_fut);
            let kw_list = kw_list?;
            let vec_list = vec_list?;

            return Ok(rrf_merge(kw_list, vec_list, limit));
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
                kw_list =
                    SurrealMemoryStore::search_keyword_raw(&self.0, user_id, &fallback, fetch_limit)
                        .await?;
            }
        }
        Ok(kw_list
            .into_iter()
            .map(|(_, mem)| (mem, None))
            .take(limit as usize)
            .collect())
    }
}
