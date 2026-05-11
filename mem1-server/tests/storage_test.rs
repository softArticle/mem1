//! Unit tests for storage layer (T011). Run with: cargo test --test storage_test

use mem1_server::memory::model::Memory;
use mem1_server::storage::{self, MemoryFilters, MemoryStore};
use std::collections::HashMap;

#[test]
fn memory_new_sets_id_and_timestamps() {
    let m = Memory::new("hello".to_string(), "user1".to_string(), HashMap::new());
    assert!(!m.id.is_empty());
    assert_eq!(m.content, "hello");
    assert_eq!(m.user_id, "user1");
    assert!(!m.created_at.is_empty());
    assert_eq!(m.created_at, m.updated_at);
}

async fn test_store(name: &str) -> (String, storage::SurrealMemoryStore) {
    let db_path = std::env::temp_dir().join(format!(
        "mem1-storage-test-{}-{}",
        name,
        uuid::Uuid::new_v4()
    ));
    let db_path = db_path.to_string_lossy().to_string();
    let db = storage::connect(&db_path).await.unwrap();
    storage::ensure_schema(&db).await.unwrap();
    (db_path, storage::store(db))
}

#[tokio::test]
async fn update_changes_content_metadata_and_records_history() {
    let (db_path, store) = test_store("update-history").await;
    let created = store
        .add(&Memory::new(
            "old content".to_string(),
            "u1".to_string(),
            HashMap::new(),
        ))
        .await
        .unwrap();

    let mut metadata = HashMap::new();
    metadata.insert("scope".to_string(), serde_json::json!("project"));
    let updated = store
        .update(
            &created.id,
            "u1",
            Some("new content".to_string()),
            Some(metadata),
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(updated.content, "new content");
    assert_eq!(
        updated.metadata.get("scope").and_then(|v| v.as_str()),
        Some("project")
    );
    assert!(updated.updated_at >= updated.created_at);

    let history = store.history(&created.id, "u1").await.unwrap();
    let operations: Vec<_> = history.iter().map(|h| h.operation.as_str()).collect();
    assert_eq!(operations, vec!["ADD", "UPDATE"]);

    let _ = std::fs::remove_dir_all(db_path);
}

#[tokio::test]
async fn delete_all_is_scoped_to_user_and_filters() {
    let (db_path, store) = test_store("delete-all").await;
    let mut project_meta = HashMap::new();
    project_meta.insert("scope".to_string(), serde_json::json!("project"));
    let mut session_meta = HashMap::new();
    session_meta.insert("scope".to_string(), serde_json::json!("session"));

    store
        .add(&Memory::new(
            "project memory".to_string(),
            "u1".to_string(),
            project_meta,
        ))
        .await
        .unwrap();
    store
        .add(&Memory::new(
            "session memory".to_string(),
            "u1".to_string(),
            session_meta,
        ))
        .await
        .unwrap();
    store
        .add(&Memory::new(
            "other user memory".to_string(),
            "u2".to_string(),
            HashMap::new(),
        ))
        .await
        .unwrap();

    let mut filters = MemoryFilters::default();
    filters
        .metadata
        .insert("scope".to_string(), "project".to_string());
    let deleted = store.delete_all("u1", &filters).await.unwrap();

    assert_eq!(deleted, 1);
    assert_eq!(
        store
            .list_by_user("u1", 10, 0, &MemoryFilters::default())
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        store
            .list_by_user("u2", 10, 0, &MemoryFilters::default())
            .await
            .unwrap()
            .len(),
        1
    );

    let _ = std::fs::remove_dir_all(db_path);
}

#[tokio::test]
async fn users_and_reset_cover_all_memories() {
    let (db_path, store) = test_store("users-reset").await;
    store
        .add(&Memory::new(
            "one".to_string(),
            "u1".to_string(),
            HashMap::new(),
        ))
        .await
        .unwrap();
    store
        .add(&Memory::new(
            "two".to_string(),
            "u2".to_string(),
            HashMap::new(),
        ))
        .await
        .unwrap();

    let mut users = store.list_users().await.unwrap();
    users.sort();
    assert_eq!(users, vec!["u1", "u2"]);

    let deleted = store.reset().await.unwrap();
    assert_eq!(deleted, 2);
    assert!(store.list_users().await.unwrap().is_empty());

    let _ = std::fs::remove_dir_all(db_path);
}
