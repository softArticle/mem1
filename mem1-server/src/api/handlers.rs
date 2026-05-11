use crate::api::dto::{
    AddMemoryRequest, AddResponse, ListMemoriesQuery, MemoryResult, SearchRequest, SearchResponse,
};
use crate::app_state::AppState;
use crate::error::Error;
use crate::memory::model::Memory;
use crate::storage::MemoryStore;
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
    let scope = normalize_filter(req.scope);
    let memory_type = normalize_filter(req.memory_type);
    let rows = state
        .store
        .search(
            &req.user_id,
            &req.query,
            query_vec,
            req.limit,
            scope.as_deref(),
            memory_type.as_deref(),
        )
        .await?;
    let formatted_context = Some(build_formatted_context(&rows));
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

    let scope = normalize_filter(q.scope);
    let memory_type = normalize_filter(q.memory_type);
    let rows = state
        .store
        .list_by_user(
            &q.user_id,
            q.limit,
            q.offset,
            scope.as_deref(),
            memory_type.as_deref(),
        )
        .await?;
    let results = rows
        .into_iter()
        .map(|m| MemoryResult {
            id: m.id,
            content: m.content,
            user_id: m.user_id,
            metadata: m.metadata,
            created_at: m.created_at,
            score: None,
        })
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
