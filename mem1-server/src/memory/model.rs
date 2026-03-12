//! Memory entity per data-model.md (T008).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A single stored AI memory (id, content, user_id, embedding, metadata, timestamps).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub content: String,
    pub user_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
    pub created_at: String, // ISO 8601
    pub updated_at: String,
}

impl Memory {
    pub fn new(content: String, user_id: String, metadata: HashMap<String, serde_json::Value>) -> Self {
        let now: String = chrono::Utc::now().to_rfc3339();
        Self {
            id: Uuid::new_v4().to_string(),
            content,
            user_id,
            embedding: None,
            metadata,
            created_at: now.clone(),
            updated_at: now,
        }
    }
}
