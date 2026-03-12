//! Unit tests for storage layer (T011). Run with: cargo test --test storage_test

use mem1_server::memory::model::Memory;
use std::collections::HashMap;

#[test]
fn memory_new_sets_id_and_timestamps() {
    let m = Memory::new(
        "hello".to_string(),
        "user1".to_string(),
        HashMap::new(),
    );
    assert!(!m.id.is_empty());
    assert_eq!(m.content, "hello");
    assert_eq!(m.user_id, "user1");
    assert!(!m.created_at.is_empty());
    assert_eq!(m.created_at, m.updated_at);
}
