use crate::api::dto::{AddMemoryRequest, AddResponse, MemoryResult, SearchRequest, SearchResponse};
use crate::app_state::AppState;
use crate::error::Error;
use crate::memory::model::Memory;
use crate::storage::MemoryStore;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(serde::Deserialize)]
pub struct UserScopeQuery {
    pub user_id: String,
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
        results: vec![MemoryResult {
            id: created.id,
            content: created.content,
            user_id: created.user_id,
            metadata: created.metadata,
            created_at: created.created_at,
            score: None,
        }],
    };
    Ok((StatusCode::CREATED, Json(out)))
}

pub async fn search_memories(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, Error> {
    if req.user_id.trim().is_empty() {
        return Err(Error::InvalidInput("user_id is required".to_string()));
    }
    if req.query.trim().is_empty() {
        return Err(Error::InvalidInput("query is required".to_string()));
    }

    let query_vec = state.embedder.embed_text(&req.query).await?;
    let rows = state
        .store
        .search(&req.user_id, &req.query, query_vec, req.limit)
        .await?;
    let results = rows
        .into_iter()
        .map(|(m, score)| MemoryResult {
            id: m.id,
            content: m.content,
            user_id: m.user_id,
            metadata: m.metadata,
            created_at: m.created_at,
            score,
        })
        .collect();

    Ok(Json(SearchResponse { results }))
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

    Ok(Json(MemoryResult {
        id: memory.id,
        content: memory.content,
        user_id: memory.user_id,
        metadata: memory.metadata,
        created_at: memory.created_at,
        score: None,
    }))
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

