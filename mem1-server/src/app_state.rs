use crate::memory::embedding::Embedder;
use crate::memory::llm_extract::LlmExtractor;
use crate::memory::rerank::LlmReranker;
use crate::storage::SurrealMemoryStore;

pub struct AppState {
    pub store: SurrealMemoryStore,
    pub embedder: Embedder,
    pub extractor: Option<LlmExtractor>,
    pub reranker: Option<LlmReranker>,
}
