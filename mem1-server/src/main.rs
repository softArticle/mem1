//! Binary entrypoint: axum HTTP server (T013).

use axum::{routing::get, Router};
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

    let db_path =
        std::env::var("MEM1_DB_PATH").unwrap_or_else(|_| "mem1.db".to_string());
    let db = storage::connect(&db_path).await?;
    storage::ensure_schema(&db).await?;
    let store = storage::store(db);
    let embedder = Embedder::from_env()?;
    let state = Arc::new(AppState { store, embedder });

    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/memories", axum::routing::post(handlers::add_memory))
        .route("/memories/search", axum::routing::post(handlers::search_memories))
        .route(
            "/memories/:id",
            get(handlers::get_memory).delete(handlers::delete_memory),
        )
        .with_state(state)
        .layer(axum::middleware::from_fn(middleware::trace_layer));

    tracing::info!(%addr, "mem1-server listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
