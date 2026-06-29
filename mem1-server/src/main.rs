//! Binary entrypoint: axum HTTP server (T013).

use axum::{routing::get, routing::post, Router};
use mem1_server::api::{handlers, middleware};
use mem1_server::app_state::AppState;
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

    let state = Arc::new(AppState::from_env().await?);

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
