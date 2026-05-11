use crate::api::dto::{
    AddMemoryRequest, AddResponse, DeleteAllResponse, HistoryResponse, ListMemoriesQuery,
    MemoryHistoryResult, MemoryResult, SearchRequest, SearchResponse, UpdateMemoryRequest,
    UsersResponse,
};
use crate::app_state::AppState;
use crate::error::Error;
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
    let mut entities = Vec::with_capacity(memories.len());
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
        let range = if invalid.is_empty() {
            format!("Date: {}", valid)
        } else {
            format!("Date range: {} - {}", valid, invalid)
        };
        facts.push(format!("{} ({})", m.content.trim(), range));
        entities.push(format!("{}: {}", m.id, m.content.trim()));
    }
    format!(
        "FACTS and ENTITIES represent relevant context (Zep/Graphiti-style).\nformat: FACT (Date range: from - to)\n<FACTS>\n{}\n</FACTS>\nThese are the most relevant entities.\n<ENTITIES>\n{}\n</ENTITIES>",
        facts.join("\n"),
        entities.join("\n")
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

pub async fn add_memory(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AddMemoryRequest>,
) -> Result<(StatusCode, Json<AddResponse>), Error> {
    let (user_id, content, metadata) = match req {
        AddMemoryRequest::ByContent {
            user_id,
            content,
            metadata,
        } => (user_id, content, metadata),
        AddMemoryRequest::ByMessages { user_id, messages } => {
            let content = messages
                .into_iter()
                .map(|m| m.content)
                .filter(|s| !s.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            (user_id, content, HashMap::new())
        }
    };

    if user_id.trim().is_empty() {
        return Err(Error::InvalidInput("user_id is required".to_string()));
    }
    if content.trim().is_empty() {
        return Err(Error::InvalidInput("content is required".to_string()));
    }

    let mut memory = Memory::new(content, user_id, metadata);
    if let Some(vec) = state.embedder.embed_text(&memory.content).await? {
        memory.embedding = Some(vec);
    }
    let created = state.store.add(&memory).await?;

    let out = AddResponse {
        results: vec![memory_to_result(created, None)],
    };
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
    let rows = state
        .store
        .search(&user_id, &req.query, query_vec, req.limit, &filters)
        .await?;
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

    let updated = state
        .store
        .update(
            &id,
            &req.user_id,
            req.content.map(|s| s.trim().to_string()),
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
