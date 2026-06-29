use crate::memory::embedding::Embedder;
use crate::memory::llm_extract::LlmExtractor;
use crate::memory::query_rewrite::QueryRewriter;
use crate::memory::rerank::LlmReranker;
use crate::storage::SurrealMemoryStore;

pub struct AppState {
    pub store: SurrealMemoryStore,
    pub embedder: Embedder,
    pub extractor: Option<LlmExtractor>,
    pub reranker: Option<LlmReranker>,
    /// Multi-query rewriter (LLM, env-gated). Expands one query into focused
    /// sub-queries whose retrieval runs are fused. None = single-query search.
    pub query_rewriter: Option<QueryRewriter>,
    /// Embedded cross-encoder reranker (tract, in-process). Takes precedence over
    /// the HTTP/LLM `reranker` when present.
    #[cfg(feature = "local-embed")]
    pub cross_encoder: Option<crate::memory::local_rerank::LocalCrossEncoder>,
}
