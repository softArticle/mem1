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

impl AppState {
    /// Build the full application state from environment configuration: open the
    /// SurrealDB store (`MEM1_DB_PATH`), ensure the schema, and load the embedder
    /// and all optional enrichment components (extractor, reranker, query
    /// rewriter, embedded cross-encoder). Used by both the HTTP server and the
    /// MCP server so they share an identical store + retrieval pipeline.
    pub async fn from_env() -> anyhow::Result<Self> {
        let db_path = std::env::var("MEM1_DB_PATH").unwrap_or_else(|_| "mem1.db".to_string());
        let db = crate::storage::connect(&db_path).await?;
        crate::storage::ensure_schema(&db).await?;
        let store = crate::storage::store(db);
        let embedder = Embedder::from_env()?;

        let extractor = LlmExtractor::from_env();
        if extractor.is_some() {
            tracing::info!("LLM fact extraction enabled (llm-v1)");
        }
        let reranker = LlmReranker::from_env();
        if reranker.is_some() {
            tracing::info!("LLM listwise reranker enabled (RankGPT-style)");
        }
        let query_rewriter = QueryRewriter::from_env();
        if query_rewriter.is_some() {
            tracing::info!("LLM multi-query rewriter enabled");
        }

        #[cfg(feature = "local-embed")]
        {
            let cross_encoder = crate::memory::local_rerank::LocalCrossEncoder::from_env();
            if cross_encoder.is_some() {
                tracing::info!("embedded cross-encoder reranker enabled (tract, in-process)");
            }
            Ok(Self {
                store,
                embedder,
                extractor,
                reranker,
                query_rewriter,
                cross_encoder,
            })
        }
        #[cfg(not(feature = "local-embed"))]
        {
            Ok(Self {
                store,
                embedder,
                extractor,
                reranker,
                query_rewriter,
            })
        }
    }
}
