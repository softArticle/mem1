use crate::memory::embedding::Embedder;
use crate::storage::SurrealMemoryStore;

pub struct AppState {
    pub store: SurrealMemoryStore,
    pub embedder: Embedder,
}

