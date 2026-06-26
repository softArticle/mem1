//! Binary entrypoint: axum HTTP server (T013).

use axum::{routing::get, routing::post, Router};
use mem1_server::api::{handlers, middleware};
use mem1_server::app_state::AppState;
use mem1_server::memory::embedding::Embedder;
use mem1_server::storage;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("mem1_server=info".parse()?))
        .init();

    let bind = std::env::var("MEM1_BIND").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let addr: SocketAddr = bind.parse()?;

    let db_path = std::env::var("MEM1_DB_PATH").unwrap_or_else(|_| "mem1.db".to_string());
    let db = storage::connect(&db_path).await?;
    storage::ensure_schema(&db).await?;
    let store = storage::store(db);
    let embedder = Embedder::from_env()?;
    let extractor = mem1_server::memory::llm_extract::LlmExtractor::from_env();
    if extractor.is_some() {
        tracing::info!("LLM fact extraction enabled (llm-v1)");
    }
    let reranker = mem1_server::memory::rerank::LlmReranker::from_env();
    if reranker.is_some() {
        tracing::info!("LLM listwise reranker enabled (RankGPT-style)");
    }

    #[cfg(feature = "local-embed")]
    let state = {
        let cross_encoder = mem1_server::memory::local_rerank::LocalCrossEncoder::from_env();
        if cross_encoder.is_some() {
            tracing::info!("embedded cross-encoder reranker enabled (tract, in-process)");
        }
        Arc::new(AppState {
            store,
            embedder,
            extractor,
            reranker,
            cross_encoder,
        })
    };
    #[cfg(not(feature = "local-embed"))]
    let state = Arc::new(AppState {
        store,
        embedder,
        extractor,
        reranker,
    });

    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route(
            "/memories",
            get(handlers::list_memories)
                .post(handlers::add_memory)
                .delete(handlers::delete_all_memories),
        )
        .route(
            "/memories/search",
            axum::routing::post(handlers::search_memories),
        )
        .route(
            "/memories/:id",
            get(handlers::get_memory)
                .patch(handlers::update_memory)
                .delete(handlers::delete_memory),
        )
        .route("/memories/:id/history", get(handlers::memory_history))
        .route("/users", get(handlers::list_users))
        .route("/reset", post(handlers::reset_memories))
        .with_state(state)
        .layer(axum::middleware::from_fn(middleware::trace_layer));

    tracing::info!(%addr, "mem1-server listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
