//! The control room's MCP server: the tools agents use to work with other threads. Each
//! thread's CLI gets its own token; a tool call is matched to its thread by that token only,
//! and the token stops working when the thread's CLI stops. The tools are thin: each one is a
//! host operation.
mod tools;

use crate::host::Host;
use anyhow::{Context, Result};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use std::sync::Arc;
use tokio::net::TcpListener;

/// Starts serving the tools on a free local port and tells the host where they are.
pub async fn start(host: Arc<Host>) -> Result<String> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.context("could not open a port for the control room")?;
    let url = format!("http://{}/mcp", listener.local_addr()?);
    let h = host.clone();
    let service = StreamableHttpService::new(
        move || Ok(tools::ControlRoom::new(h.clone())),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default(),
    );
    let app = axum::Router::new().nest_service("/mcp", service);
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            eprintln!("curia-host: the control room stopped: {e}");
        }
    });
    host.set_mcp_url(url.clone());
    Ok(url)
}
