use crate::api::dto::{
    AddMemoryRequest, AddResponse, DeleteAllResponse, HistoryResponse, ListMemoriesQuery,
    MemoryHistoryResult, MemoryResult, SearchRequest, SearchResponse, UpdateMemoryRequest,
    UsersResponse,
};
use crate::app_state::AppState;
use crate::error::Error;
use crate::memory::extraction::{
    detect_language, extract_facts, ExtractedFact, SourceText, EXTRACTOR_VERSION,
};
use crate::memory::model::Memory;
use crate::storage::{MemoryFilters, MemoryStore};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use std::collections::HashMap;
use std::sync::Arc;

/// Build Zep/Graphiti-style context string: FACTS (with date range) + ENTITIES (id: content).
/// Paper: "For each e_i, χ returns the fact and t_valid, t_invalid; for each n_i, name and summary."
fn build_formatted_context(memories: &[(Memory, Option<f32>)]) -> String {
    if memories.is_empty() {
        return String::new();
    }
    let mut facts = Vec::with_capacity(memories.len());
    for (m, _) in memories {
        let valid = m
            .metadata
            .get("valid_at")
            .and_then(|v| v.as_str())
            .unwrap_or(&m.created_at);
        let invalid = m
            .metadata
            .get("invalid_at")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let speaker = m
            .metadata
            .get("source_role")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty() && *s != "message");
        let range = if invalid.is_empty() {
            format!("Date: {}", valid)
        } else {
            format!("Date range: {} - {}", valid, invalid)
        };
        let fact = match speaker {
            Some(role) => format!("{} — {} ({})", role, m.content.trim(), range),
            None => format!("{} ({})", m.content.trim(), range),
        };
        facts.push(fact);
    }
    format!(
        "FACTS represent relevant context (Zep/Graphiti-style).\nformat: SPEAKER — FACT (Date range: from - to)\n<FACTS>\n{}\n</FACTS>",
        facts.join("\n"),
    )
}

#[derive(serde::Deserialize)]
pub struct UserScopeQuery {
    pub user_id: String,
}

fn normalize_filter(value: Option<String>) -> Option<String> {
    value.and_then(|s| {
        let s = s.trim();
        if s.is_empty() {
            None
        } else {
            Some(s.to_string())
        }
    })
}

fn filters_from_query(q: &ListMemoriesQuery) -> MemoryFilters {
    let mut filters = MemoryFilters::from_scope_type(q.scope.as_deref(), q.memory_type.as_deref());
    if let Some(agent_id) = normalize_filter(q.agent_id.clone()) {
        filters.metadata.insert("agent_id".to_string(), agent_id);
    }
    if let Some(run_id) = normalize_filter(q.run_id.clone()) {
        filters.metadata.insert("run_id".to_string(), run_id);
    }
    filters
}

fn filters_from_search(req: &SearchRequest) -> Result<(String, MemoryFilters), Error> {
    let user_id = normalize_filter(req.user_id.clone())
        .or_else(|| {
            req.filters
                .get("user_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .ok_or_else(|| Error::InvalidInput("user_id is required".to_string()))?;

    let mut filters =
        MemoryFilters::from_scope_type(req.scope.as_deref(), req.memory_type.as_deref());
    for (key, value) in &req.filters {
        if key == "user_id" {
            continue;
        }
        if let Some(value) = value
            .as_str()
            .and_then(|s| normalize_filter(Some(s.to_string())))
        {
            filters.metadata.insert(key.clone(), value);
        }
    }
    Ok((user_id, filters))
}

fn memory_to_result(memory: Memory, score: Option<f32>) -> MemoryResult {
    MemoryResult {
        id: memory.id,
        content: memory.content,
        user_id: memory.user_id,
        metadata: memory.metadata,
        created_at: memory.created_at,
        score,
    }
}

fn sources_original_content(sources: &[SourceText]) -> String {
    sources
        .iter()
        .map(|source| source.text.trim())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn fallback_fact(sources: &[SourceText]) -> ExtractedFact {
    let content = sources_original_content(sources);
    let (source_role, source_index) = if sources.len() == 1 {
        let source = &sources[0];
        let role = source.role.trim();
        (
            if role.is_empty() {
                "message".to_string()
            } else {
                role.to_string()
            },
            source.index,
        )
    } else {
        ("messages".to_string(), 0)
    };

    ExtractedFact {
        language: detect_language(&content).to_string(),
        source_text: content.clone(),
        content,
        source_role,
        source_index,
    }
}

fn metadata_for_fact(
    base: &HashMap<String, serde_json::Value>,
    fact: &ExtractedFact,
) -> HashMap<String, serde_json::Value> {
    let mut metadata = base.clone();
    metadata.insert(
        "source_text".to_string(),
        serde_json::Value::String(fact.source_text.clone()),
    );
    metadata.insert(
        "source_role".to_string(),
        serde_json::Value::String(fact.source_role.clone()),
    );
    metadata.insert(
        "source_index".to_string(),
        serde_json::json!(fact.source_index),
    );
    metadata.insert(
        "language".to_string(),
        serde_json::Value::String(fact.language.clone()),
    );
    metadata.insert(
        "extractor_version".to_string(),
        serde_json::Value::String(EXTRACTOR_VERSION.to_string()),
    );
    metadata
}

pub async fn add_memory(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AddMemoryRequest>,
) -> Result<(StatusCode, Json<AddResponse>), Error> {
    let (user_id, sources, metadata) = match req {
        AddMemoryRequest::ByContent {
            user_id,
            content,
            metadata,
        } => (
            user_id,
            vec![SourceText {
                text: content,
                role: "content".to_string(),
                index: 0,
            }],
            metadata,
        ),
        AddMemoryRequest::ByMessages {
            user_id,
            messages,
            metadata,
        } => {
            let sources = messages
                .into_iter()
                .enumerate()
                .filter_map(|(index, m)| {
                    if m.content.trim().is_empty() {
                        None
                    } else {
                        Some(SourceText {
                            text: m.content,
                            role: m.role,
                            index,
                        })
                    }
                })
                .collect();
            (user_id, sources, metadata)
        }
    };

    if user_id.trim().is_empty() {
        return Err(Error::InvalidInput("user_id is required".to_string()));
    }
    if sources_original_content(&sources).trim().is_empty() {
        return Err(Error::InvalidInput("content is required".to_string()));
    }

    // Prefer LLM extraction (normalized atomic facts) when configured; degrade to
    // the deterministic rule-based splitter on any failure so writes never drop.
    let mut facts = match &state.extractor {
        Some(extractor) => extractor.extract(&sources).await,
        None => None,
    }
    .unwrap_or_else(|| extract_facts(&sources));
    if facts.is_empty() {
        facts.push(fallback_fact(&sources));
    }

    let mut results = Vec::with_capacity(facts.len());
    for fact in facts {
        let mut memory = Memory::new(
            fact.content.clone(),
            user_id.clone(),
            metadata_for_fact(&metadata, &fact),
        );
        if let Some(vec) = state.embedder.embed_text(&memory.content).await? {
            memory.embedding = Some(vec);
        }
        results.push(memory_to_result(state.store.add(&memory).await?, None));
    }

    let out = AddResponse { results };
    Ok((StatusCode::CREATED, Json(out)))
}

pub async fn search_memories(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, Error> {
    if req.query.trim().is_empty() {
        return Err(Error::InvalidInput("query is required".to_string()));
    }

    let (user_id, filters) = filters_from_search(&req)?;
    let query_vec = state.embedder.embed_text(&req.query).await?;
    let mmr_lambda = crate::memory::mmr::mmr_lambda_from_env();

    // Over-fetch a larger candidate pool when a reranker (LLM listwise) or MMR
    // (vector diversity) is active, re-sort it, then keep the top `req.limit`.
    // This lifts relevant-but-mid-ranked facts (the multi-hop failure mode) into
    // the answer window. Without either, behaviour is unchanged. The pool size is
    // env-tunable: a larger pool gives MMR more scattered same-entity facts to pull
    // into the answer window for multi-hop queries.
    let rerank_active = state.reranker.is_some() || mmr_lambda.is_some();
    let fetch_limit = if rerank_active {
        let extra = std::env::var("MEM1_RERANK_POOL_EXTRA")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(30);
        req.limit.saturating_add(extra).min(100)
    } else {
        req.limit
    };
    let query_vec_for_mmr = if mmr_lambda.is_some() {
        query_vec.clone()
    } else {
        None
    };
    let mut rows = state
        .store
        .search(&user_id, &req.query, query_vec, fetch_limit, &filters)
        .await?;

    // MMR diversity rerank (pure vector math, no LLM) — prefer when configured.
    if let (Some(lambda), Some(qvec)) = (mmr_lambda, query_vec_for_mmr.as_ref()) {
        let embs: Vec<Option<Vec<f32>>> = rows.iter().map(|(m, _)| m.embedding.clone()).collect();
        let order = crate::memory::mmr::mmr_order(
            qvec,
            &embs,
            lambda,
            req.limit as usize,
            crate::memory::mmr::mmr_protect(req.limit as usize),
        );
        let mut reordered: Vec<(Memory, Option<f32>)> = Vec::with_capacity(rows.len());
        let mut taken = vec![false; rows.len()];
        for idx in order {
            if idx < rows.len() && !taken[idx] {
                taken[idx] = true;
                reordered.push(rows[idx].clone());
            }
        }
        rows = reordered;
        rows.truncate(req.limit as usize);
    } else if let Some(reranker) = &state.reranker {
        let passages: Vec<String> = rows.iter().map(|(m, _)| m.content.clone()).collect();
        let order = reranker.rerank(&req.query, &passages).await;
        let mut reordered: Vec<(Memory, Option<f32>)> = Vec::with_capacity(rows.len());
        let mut taken = vec![false; rows.len()];
        for idx in order {
            if idx < rows.len() && !taken[idx] {
                taken[idx] = true;
                reordered.push(rows[idx].clone());
            }
        }
        rows = reordered;
        rows.truncate(req.limit as usize);
    }

    let formatted_context = Some(build_formatted_context(&rows));
    let results = rows
        .into_iter()
        .map(|(m, score)| memory_to_result(m, score))
        .collect();

    Ok(Json(SearchResponse {
        results,
        formatted_context,
    }))
}

pub async fn list_memories(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListMemoriesQuery>,
) -> Result<Json<AddResponse>, Error> {
    if q.user_id.trim().is_empty() {
        return Err(Error::InvalidInput("user_id is required".to_string()));
    }

    let filters = filters_from_query(&q);
    let rows = state
        .store
        .list_by_user(&q.user_id, q.limit, q.offset, &filters)
        .await?;
    let results = rows
        .into_iter()
        .map(|m| memory_to_result(m, None))
        .collect();

    Ok(Json(AddResponse { results }))
}

pub async fn get_memory(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(q): Query<UserScopeQuery>,
) -> Result<Json<MemoryResult>, Error> {
    if q.user_id.trim().is_empty() {
        return Err(Error::InvalidInput("user_id is required".to_string()));
    }
    let memory = state.store.get(&id, &q.user_id).await?;
    let memory = memory.ok_or(Error::NotFound)?;

    Ok(Json(memory_to_result(memory, None)))
}

pub async fn update_memory(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateMemoryRequest>,
) -> Result<Json<MemoryResult>, Error> {
    if req.user_id.trim().is_empty() {
        return Err(Error::InvalidInput("user_id is required".to_string()));
    }
    if req.content.as_ref().is_some_and(|s| s.trim().is_empty()) {
        return Err(Error::InvalidInput("content cannot be empty".to_string()));
    }
    if req.content.is_none() && req.metadata.is_empty() {
        return Err(Error::InvalidInput(
            "content or metadata is required".to_string(),
        ));
    }

    let content = req.content.map(|s| s.trim().to_string());
    let embedding = if let Some(content) = &content {
        state.embedder.embed_text(content).await?
    } else {
        None
    };
    let updated = state
        .store
        .update(
            &id,
            &req.user_id,
            content,
            embedding,
            if req.metadata.is_empty() {
                None
            } else {
                Some(req.metadata)
            },
        )
        .await?;
    let updated = updated.ok_or(Error::NotFound)?;
    Ok(Json(memory_to_result(updated, None)))
}

pub async fn delete_memory(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(q): Query<UserScopeQuery>,
) -> Result<StatusCode, Error> {
    if q.user_id.trim().is_empty() {
        return Err(Error::InvalidInput("user_id is required".to_string()));
    }
    let ok = state.store.delete(&id, &q.user_id).await?;
    if !ok {
        return Err(Error::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_all_memories(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListMemoriesQuery>,
) -> Result<Json<DeleteAllResponse>, Error> {
    if q.user_id.trim().is_empty() {
        return Err(Error::InvalidInput("user_id is required".to_string()));
    }
    let filters = filters_from_query(&q);
    let deleted = state.store.delete_all(&q.user_id, &filters).await?;
    Ok(Json(DeleteAllResponse { deleted }))
}

pub async fn memory_history(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(q): Query<UserScopeQuery>,
) -> Result<Json<HistoryResponse>, Error> {
    if q.user_id.trim().is_empty() {
        return Err(Error::InvalidInput("user_id is required".to_string()));
    }
    let rows = state.store.history(&id, &q.user_id).await?;
    let results = rows
        .into_iter()
        .map(|h| MemoryHistoryResult {
            id: h.id,
            memory_id: h.memory_id,
            user_id: h.user_id,
            operation: h.operation,
            previous: h.previous.map(|m| memory_to_result(m, None)),
            current: h.current.map(|m| memory_to_result(m, None)),
            created_at: h.created_at,
        })
        .collect();
    Ok(Json(HistoryResponse { results }))
}

pub async fn list_users(State(state): State<Arc<AppState>>) -> Result<Json<UsersResponse>, Error> {
    let users = state.store.list_users().await?;
    Ok(Json(UsersResponse { users }))
}

pub async fn reset_memories(
    State(state): State<Arc<AppState>>,
) -> Result<Json<DeleteAllResponse>, Error> {
    let deleted = state.store.reset().await?;
    Ok(Json(DeleteAllResponse { deleted }))
}

#[cfg(test)]
mod tests {
    use super::add_memory;
    use crate::api::dto::{AddMemoryRequest, Message};
    use crate::app_state::AppState;
    use crate::memory::embedding::Embedder;
    use crate::storage;
    use axum::extract::State;
    use axum::http::StatusCode;
    use axum::Json;
    use std::collections::HashMap;
    use std::sync::Arc;

    async fn test_state(name: &str) -> (String, Arc<AppState>) {
        let db_path = std::env::temp_dir().join(format!(
            "mem1-handler-test-{}-{}",
            name,
            uuid::Uuid::new_v4()
        ));
        let db_path = db_path.to_string_lossy().to_string();
        let db = storage::connect(&db_path).await.unwrap();
        storage::ensure_schema(&db).await.unwrap();
        let state = Arc::new(AppState {
            store: storage::store(db),
            embedder: Embedder::Off,
            extractor: None,
            reranker: None,
        });
        (db_path, state)
    }

    #[tokio::test]
    async fn add_content_stores_one_memory_per_extracted_fact_with_metadata() {
        let (db_path, state) = test_state("content-fanout").await;
        let mut metadata = HashMap::new();
        metadata.insert("scope".to_string(), serde_json::json!("profile"));

        let (status, Json(resp)) = add_memory(
            State(state),
            Json(AddMemoryRequest::ByContent {
                user_id: "u1".to_string(),
                content: " Alice likes Rust. Alice lives in Paris. ".to_string(),
                metadata,
            }),
        )
        .await
        .unwrap();

        assert_eq!(status, StatusCode::CREATED);
        // rule-v2 keeps the whole message as one context-rich fact.
        assert_eq!(resp.results.len(), 1);
        assert_eq!(
            resp.results[0].content,
            "Alice likes Rust. Alice lives in Paris."
        );

        for result in &resp.results {
            assert_eq!(
                result.metadata.get("scope").and_then(|v| v.as_str()),
                Some("profile")
            );
            assert_eq!(
                result.metadata.get("source_text").and_then(|v| v.as_str()),
                Some("Alice likes Rust. Alice lives in Paris.")
            );
            assert_eq!(
                result.metadata.get("source_role").and_then(|v| v.as_str()),
                Some("content")
            );
            assert_eq!(
                result.metadata.get("source_index").and_then(|v| v.as_u64()),
                Some(0)
            );
            assert_eq!(
                result.metadata.get("language").and_then(|v| v.as_str()),
                Some("en")
            );
            assert_eq!(
                result
                    .metadata
                    .get("extractor_version")
                    .and_then(|v| v.as_str()),
                Some("rule-v2")
            );
        }

        let _ = std::fs::remove_dir_all(db_path);
    }

    #[tokio::test]
    async fn add_messages_preserves_source_role_and_index_per_extracted_fact() {
        let (db_path, state) = test_state("message-fanout").await;

        let (status, Json(resp)) = add_memory(
            State(state),
            Json(AddMemoryRequest::ByMessages {
                user_id: "u1".to_string(),
                messages: vec![
                    Message {
                        role: "user".to_string(),
                        content: "I prefer tea. I live in Berlin.".to_string(),
                    },
                    Message {
                        role: "assistant".to_string(),
                        content: "Noted.".to_string(),
                    },
                ],
                metadata: HashMap::new(),
            }),
        )
        .await
        .unwrap();

        assert_eq!(status, StatusCode::CREATED);
        // rule-v2: one fact per message (whole message), so two messages -> two facts.
        assert_eq!(resp.results.len(), 2);
        assert_eq!(resp.results[0].content, "I prefer tea. I live in Berlin.");
        assert_eq!(resp.results[1].content, "Noted.");

        assert_eq!(
            resp.results[0]
                .metadata
                .get("source_role")
                .and_then(|v| v.as_str()),
            Some("user")
        );
        assert_eq!(
            resp.results[0]
                .metadata
                .get("source_index")
                .and_then(|v| v.as_u64()),
            Some(0)
        );
        assert_eq!(
            resp.results[1]
                .metadata
                .get("source_role")
                .and_then(|v| v.as_str()),
            Some("assistant")
        );
        assert_eq!(
            resp.results[1]
                .metadata
                .get("source_index")
                .and_then(|v| v.as_u64()),
            Some(1)
        );
        assert_eq!(
            resp.results[1]
                .metadata
                .get("source_text")
                .and_then(|v| v.as_str()),
            Some("Noted.")
        );

        let _ = std::fs::remove_dir_all(db_path);
    }

    #[tokio::test]
    async fn add_content_falls_back_to_trimmed_original_when_no_fact_is_extracted() {
        let (db_path, state) = test_state("fallback").await;

        let (status, Json(resp)) = add_memory(
            State(state),
            Json(AddMemoryRequest::ByContent {
                user_id: "u1".to_string(),
                content: " \n ... \t".to_string(),
                metadata: HashMap::new(),
            }),
        )
        .await
        .unwrap();

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(resp.results.len(), 1);
        assert_eq!(resp.results[0].content, "...");
        assert_eq!(
            resp.results[0]
                .metadata
                .get("source_text")
                .and_then(|v| v.as_str()),
            Some("...")
        );
        assert_eq!(
            resp.results[0]
                .metadata
                .get("source_role")
                .and_then(|v| v.as_str()),
            Some("content")
        );

        let _ = std::fs::remove_dir_all(db_path);
    }
}
