//! mem1-mcp: stdio MCP server for mem1.
//!
//! Embeds the mem1 store in-process (AppState::from_env) and serves the memory
//! tools over stdio JSON-RPC. Logs go to STDERR — stdout is reserved for the
//! MCP protocol stream, and any stray byte on stdout breaks the client
//! handshake.

mod server;

use std::sync::Arc;

use mem1_server::app_state::AppState;
use rmcp::transport::io::stdio;
use rmcp::ServiceExt;
use tracing_subscriber::EnvFilter;

use server::Mem1Mcp;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("mem1_mcp=info,mem1_server=info")),
        )
        .init();

    let state = Arc::new(AppState::from_env().await?);
    tracing::info!("mem1-mcp starting (stdio)");

    let service = Mem1Mcp::new(state).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
