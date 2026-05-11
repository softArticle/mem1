use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum AddMemoryRequest {
    ByMessages {
        user_id: String,
        messages: Vec<Message>,
    },
    ByContent {
        user_id: String,
        content: String,
        #[serde(default)]
        metadata: HashMap<String, serde_json::Value>,
    },
}

#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub user_id: String,
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub memory_type: Option<String>,
}

fn default_limit() -> u32 {
    10
}

#[derive(Debug, Deserialize)]
pub struct ListMemoriesQuery {
    pub user_id: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub memory_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MemoryResult {
    pub id: String,
    pub content: String,
    pub user_id: String,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,
}

#[derive(Debug, Serialize)]
pub struct AddResponse {
    pub results: Vec<MemoryResult>,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<MemoryResult>,
    /// Zep/Graphiti-style assembled context (FACTS with date ranges + ENTITIES). Optional for backward compatibility.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formatted_context: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{ListMemoriesQuery, SearchRequest};
    use serde_json::json;

    #[test]
    fn search_request_keeps_old_shape_and_defaults_filters() {
        let req: SearchRequest =
            serde_json::from_value(json!({"user_id": "u1", "query": "alice"})).unwrap();

        assert_eq!(req.user_id, "u1");
        assert_eq!(req.query, "alice");
        assert_eq!(req.limit, 10);
        assert_eq!(req.scope, None);
        assert_eq!(req.memory_type, None);
    }

    #[test]
    fn search_request_accepts_scope_and_memory_type_filters() {
        let req: SearchRequest = serde_json::from_value(json!({
            "user_id": "u1",
            "query": "alice",
            "limit": 3,
            "scope": "project",
            "memory_type": "decision"
        }))
        .unwrap();

        assert_eq!(req.limit, 3);
        assert_eq!(req.scope.as_deref(), Some("project"));
        assert_eq!(req.memory_type.as_deref(), Some("decision"));
    }

    #[test]
    fn list_query_accepts_pagination_and_filters() {
        let q: ListMemoriesQuery = serde_json::from_value(json!({
            "user_id": "u1",
            "limit": 25,
            "offset": 50,
            "scope": "session",
            "memory_type": "preference"
        }))
        .unwrap();

        assert_eq!(q.user_id, "u1");
        assert_eq!(q.limit, 25);
        assert_eq!(q.offset, 50);
        assert_eq!(q.scope.as_deref(), Some("session"));
        assert_eq!(q.memory_type.as_deref(), Some("preference"));
    }
}
