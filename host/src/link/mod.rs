//! The client link: a WebSocket on `127.0.0.1` speaking the protocol in `curia_core::protocol`.
//! Local clients find the address and the token in `<data>/link.json` (private: the token
//! signs a client in as the owner). Paired devices sign in with their own keys (`auth`).
//! With `remote`, the same app is also served over HTTPS on the machine's Tailscale address
//! only (`crate::tls`), for phones.
mod auth;
mod conn;
mod filelink;
mod signin;
mod tls;
mod web;
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

type AppState = (Arc<Host>, Arc<auth::Gate>);

/// The link on the tailnet: this machine's certificate name, its address, a port, and TLS.
pub struct Remote {
    pub name: String,
    pub ip: std::net::IpAddr,
    pub port: u16,
    pub tls: Arc<rustls::ServerConfig>,
}

/// Serves the link until `shutdown` completes: on 127.0.0.1, and over HTTPS on the tailnet
/// when `remote` is given.
/// `public_url`: where phones reach this host when something else serves it to them (such as
/// `tailscale serve` on a device of its own); `curia pair` prints it.
pub async fn serve(
    host: Arc<Host>,
    port: u16,
    remote: Option<Remote>,
    public_url: Option<String>,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port)).await.with_context(|| format!("could not listen on port {port}"))?;
    let addr = listener.local_addr()?;
    let url = format!("ws://{addr}/ws");
    let remote_url = public_url.or_else(|| remote.as_ref().map(|r| format!("https://{}:{}/", r.name, r.port)));
    let mut gate = auth::Gate::new(new_token());
    gate.public_url = remote_url.clone();
    let gate = Arc::new(gate);
    let link = serde_json::json!({"url": url, "token": gate.token, "remote_url": remote_url});
    std::fs::write(host.data_dir().join("link.json"), link.to_string()).context("could not write link.json")?;
    println!("Curia host listening on {url}");
    println!("Web app at http://{addr}/");
    if let Some(u) = &remote_url {
        println!("Phones use {u}");
    }
    let app = Router::new()
        .route("/ws", get(upgrade))
        .route("/file/{token}", get(filelink::serve))
        .with_state((host, gate))
        .fallback(get(web::file));
    let (stop, stopped) = tokio::sync::watch::channel(false);
    if let Some(r) = remote {
        let tcp = TcpListener::bind((r.ip, r.port)).await.with_context(|| format!("could not listen on {}:{}", r.ip, r.port))?;
        let tls = tls::TlsListener::new(tcp, r.tls)?;
        println!("Phone app at https://{}:{}/ (Tailscale only)", r.name, r.port);
        let (app, mut stopped) = (app.clone(), stopped.clone());
        tokio::spawn(async move {
            let done = async move { let _ = stopped.wait_for(|s| *s).await; };
            if let Err(e) = axum::serve(tls, app).with_graceful_shutdown(done).await {
                eprintln!("curia-host: the Tailscale link stopped: {e:#}");
            }
        });
    }
    let result = axum::serve(listener, app).with_graceful_shutdown(shutdown).await;
    let _ = stop.send(true);
    drop(stopped);
    Ok(result?)
}

async fn upgrade(ws: WebSocketUpgrade, State((host, gate)): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| conn::handle(host, gate, socket))
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_devices;
#[cfg(test)]
mod tests_terminal;

/// 244 random bits from two v4 UUIDs.
fn new_token() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}
