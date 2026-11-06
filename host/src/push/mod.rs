//! Notifications to paired phones (Tech Stack §5.1), sent by the host itself through each
//! browser's push service: no server of ours in between. The host has its own push key
//! (VAPID), kept in its data folder. A request that is still waiting two seconds after it
//! arrives (the host's own rules settle many at once) goes to every device that may answer.
//! The message says only which thread needs you; details load when the app opens.
mod crypto;
mod send;

use crate::host::{Host, HostEvent};
use anyhow::{Context, Result};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use curia_core::Event;
use p256::SecretKey;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;

/// The host's push key: made once, kept in `<data>/push-key`.
pub struct Keys {
    secret: SecretKey,
    /// The public half, base64url: what browsers subscribe with.
    pub public: String,
}

impl Keys {
    pub fn load_or_make(data_dir: &Path) -> Result<Keys> {
        let path = data_dir.join("push-key");
        let secret = match std::fs::read_to_string(&path) {
            Ok(text) => SecretKey::from_slice(&B64URL.decode(text.trim()).context("push-key is damaged")?).context("push-key is damaged")?,
            Err(_) => {
                let k = crypto::new_key()?;
                std::fs::write(&path, B64URL.encode(k.to_bytes())).context("could not save the push key")?;
                k
            }
        };
        let public = B64URL.encode(crypto::public_bytes(&secret));
        Ok(Keys { secret, public })
    }
}

/// Watches the host's events and sends a notification for each request left waiting.
pub fn start(host: Arc<Host>) {
    let mut rx = host.subscribe();
    tokio::spawn(async move {
        loop {
            let e: HostEvent = match rx.recv().await {
                Ok(e) => e,
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return,
            };
            if let Event::ApprovalRequested { request_id, kind, tool_name, .. } = e.event {
                let host = host.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    if !host.store.request_pending(&e.thread_id, &request_id).unwrap_or(false) {
                        return;
                    }
                    let question = matches!(kind, curia_core::RequestKind::Question);
                    if let Err(err) = notify(&host, &e.thread_id, &request_id, question, &tool_name).await {
                        eprintln!("curia-host: could not send a notification: {err:#}");
                    }
                });
            }
        }
    });
}

async fn notify(host: &Host, thread_id: &str, request_id: &str, question: bool, tool: &str) -> Result<()> {
    let thread = host.store.thread_get(thread_id)?;
    let team = thread.team_id.as_deref().and_then(|t| host.store.team(t).ok());
    let who = team.map(|t| format!("{} · {}", cli_label(&t.cli), t.name)).unwrap_or_else(|| cli_label(&thread.cli).to_string());
    let tool = tool.strip_prefix("mcp__").map(|t| t.replacen("__", ": ", 1)).unwrap_or_else(|| tool.to_string());
    let message = serde_json::json!({
        "title": format!("{} needs you", thread.title),
        "body": if question { format!("{who} asks you a question") } else { format!("{who} wants to use {tool}") },
        "thread_id": thread_id,
        "request_id": request_id,
        "question": question,
    });
    let targets = host.store.push_targets()?;
    for (endpoint, p256dh, auth) in targets {
        let body = crypto::encrypt(message.to_string().as_bytes(), &p256dh, &auth)?;
        let (_, _, _, origin) = send::parse(&endpoint)?;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
        let authorization = crypto::vapid(&host.push.secret, &origin, now)?;
        match send::post(&endpoint, &authorization, &body).await {
            Ok(200..=299) => {}
            // The browser dropped this subscription (app uninstalled, permission withdrawn).
            Ok(404 | 410) => host.store.push_unsubscribe(&endpoint)?,
            Ok(code) => eprintln!("curia-host: the push service refused a notification (HTTP {code})"),
            Err(e) => eprintln!("curia-host: could not send a notification: {e:#}"),
        }
    }
    Ok(())
}

fn cli_label(cli: &str) -> &str {
    match cli {
        "claude" => "Claude",
        "codex" => "Codex",
        "agy" => "Antigravity",
        other => other,
    }
}
