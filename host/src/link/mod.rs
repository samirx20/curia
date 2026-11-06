//! The client link: a WebSocket on `127.0.0.1` speaking the protocol in `curia_core::protocol`.
//! Local clients find the address and the token in `<data>/link.json` (private: the token
//! signs a client in). Pairing, device keys and TLS for remote clients come later.
mod conn;
mod open;
mod outbox;

use crate::host::Host;
use anyhow::{Context, Result};
use axum::Router;
use axum::extract::{State, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::get;
use std::sync::Arc;
use tokio::net::TcpListener;

type AppState = (Arc<Host>, Arc<String>);

/// Serves the link until `shutdown` completes.
pub async fn serve(host: Arc<Host>, port: u16, shutdown: impl std::future::Future<Output = ()> + Send + 'static) -> Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port)).await.with_context(|| format!("could not listen on port {port}"))?;
    let addr = listener.local_addr()?;
    let token = Arc::new(new_token());
    let url = format!("ws://{addr}/ws");
    let link = serde_json::json!({"url": url, "token": token.as_str()});
    std::fs::write(host.data_dir().join("link.json"), link.to_string()).context("could not write link.json")?;
    println!("Curia host listening on {url}");
    let app = Router::new().route("/ws", get(upgrade)).with_state((host, token));
    axum::serve(listener, app).with_graceful_shutdown(shutdown).await?;
    Ok(())
}

async fn upgrade(ws: WebSocketUpgrade, State((host, token)): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| conn::handle(host, token, socket))
}

#[cfg(test)]
mod tests;

/// 244 random bits from two v4 UUIDs.
fn new_token() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}
