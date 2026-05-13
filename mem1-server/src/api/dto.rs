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
        #[serde(default)]
        metadata: HashMap<String, serde_json::Value>,
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
    #[serde(default)]
    pub user_id: Option<String>,
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub memory_type: Option<String>,
    #[serde(default)]
    pub filters: HashMap<String, serde_json::Value>,
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
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMemoryRequest {
    pub user_id: String,
    #[serde(default, alias = "data")]
    pub content: Option<String>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
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

#[derive(Debug, Serialize)]
pub struct DeleteAllResponse {
    pub deleted: u64,
}

#[derive(Debug, Serialize)]
pub struct UsersResponse {
    pub users: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct MemoryHistoryResult {
    pub id: String,
    pub memory_id: String,
    pub user_id: String,
    pub operation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<MemoryResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<MemoryResult>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct HistoryResponse {
    pub results: Vec<MemoryHistoryResult>,
}

#[cfg(test)]
mod tests {
    use super::{AddMemoryRequest, ListMemoriesQuery, SearchRequest};
    use serde_json::json;

    #[test]
    fn add_request_accepts_by_content_payload() {
        let req: AddMemoryRequest = serde_json::from_value(json!({
            "user_id": "u1",
            "content": "Alice likes Rust.",
            "metadata": {"scope": "profile"}
        }))
        .unwrap();

        match req {
            AddMemoryRequest::ByContent {
                user_id,
                content,
                metadata,
            } => {
                assert_eq!(user_id, "u1");
                assert_eq!(content, "Alice likes Rust.");
                assert_eq!(
                    metadata.get("scope").and_then(|v| v.as_str()),
                    Some("profile")
                );
            }
            AddMemoryRequest::ByMessages { .. } => panic!("expected by-content add request"),
        }
    }

    #[test]
    fn add_request_accepts_by_messages_payload_with_optional_metadata() {
        let req: AddMemoryRequest = serde_json::from_value(json!({
            "user_id": "u1",
            "messages": [
                {"role": "user", "content": "I prefer tea."}
            ],
            "metadata": {"agent_id": "agent-a"}
        }))
        .unwrap();

        match req {
            AddMemoryRequest::ByMessages {
                user_id,
                messages,
                metadata,
            } => {
                assert_eq!(user_id, "u1");
                assert_eq!(messages.len(), 1);
                assert_eq!(messages[0].role, "user");
                assert_eq!(messages[0].content, "I prefer tea.");
                assert_eq!(
                    metadata.get("agent_id").and_then(|v| v.as_str()),
                    Some("agent-a")
                );
            }
            AddMemoryRequest::ByContent { .. } => panic!("expected by-messages add request"),
        }
    }

    #[test]
    fn search_request_keeps_old_shape_and_defaults_filters() {
        let req: SearchRequest =
            serde_json::from_value(json!({"user_id": "u1", "query": "alice"})).unwrap();

        assert_eq!(req.user_id.as_deref(), Some("u1"));
        assert_eq!(req.query, "alice");
        assert_eq!(req.limit, 10);
        assert_eq!(req.scope, None);
        assert_eq!(req.memory_type, None);
        assert!(req.filters.is_empty());
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
        assert_eq!(req.user_id.as_deref(), Some("u1"));
        assert_eq!(req.scope.as_deref(), Some("project"));
        assert_eq!(req.memory_type.as_deref(), Some("decision"));
    }

    #[test]
    fn search_request_accepts_mem0_style_filters() {
        let req: SearchRequest = serde_json::from_value(json!({
            "query": "alice",
            "filters": {
                "user_id": "u1",
                "agent_id": "agent-a",
                "run_id": "run-1"
            }
        }))
        .unwrap();

        assert_eq!(req.user_id, None);
        assert_eq!(
            req.filters.get("user_id").and_then(|v| v.as_str()),
            Some("u1")
        );
        assert_eq!(
            req.filters.get("agent_id").and_then(|v| v.as_str()),
            Some("agent-a")
        );
        assert_eq!(
            req.filters.get("run_id").and_then(|v| v.as_str()),
            Some("run-1")
        );
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
        assert_eq!(q.agent_id, None);
        assert_eq!(q.run_id, None);
    }
}
