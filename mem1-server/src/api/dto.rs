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
}

fn default_limit() -> u32 {
    10
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
}

