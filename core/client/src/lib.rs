//! A connection to a Curia host. `call` sends a request and waits for its reply; pushed
//! events arrive on the receiver `connect` returns.
use anyhow::{Context, Result, bail};
use curia_core::protocol::{Call, Notification, Request, RpcError, WireEvent};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;

/// Something the host pushed.
#[derive(Debug)]
pub enum Incoming {
    Event(WireEvent),
    /// The host is closing the connection, and why.
    Closing(String),
    /// Something happened in a thread this client doesn't follow (its id, and the event's type).
    Activity { thread_id: String, kind: String },
}

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value>>>>>;

pub struct Link {
    out: mpsc::UnboundedSender<Message>,
    pending: Pending,
    next_id: AtomicU64,
}

impl Link {
    /// Connects to the host whose data folder is `data` (it holds `link.json`) and signs in.
    pub async fn connect_local(data: &Path) -> Result<(Link, mpsc::UnboundedReceiver<Incoming>)> {
        let path = data.join("link.json");
        let text = std::fs::read_to_string(&path).with_context(|| format!("no running host found ({} is missing)", path.display()))?;
        let v: Value = serde_json::from_str(&text)?;
        let url = v["url"].as_str().context("link.json has no url")?;
        let token = v["token"].as_str().context("link.json has no token")?;
        Link::connect(url, token).await
    }

    pub async fn connect(url: &str, token: &str) -> Result<(Link, mpsc::UnboundedReceiver<Incoming>)> {
        let (ws, _) = tokio_tungstenite::connect_async(url).await.with_context(|| format!("could not reach the host at {url}"))?;
        let (mut sink, mut stream) = ws.split();
        let (out, mut out_rx) = mpsc::unbounded_channel::<Message>();
        let (events, events_rx) = mpsc::unbounded_channel();
        let pending: Pending = Arc::default();
        tokio::spawn(async move {
            while let Some(m) = out_rx.recv().await {
                if sink.send(m).await.is_err() {
                    break;
                }
            }
        });
        let waiting = pending.clone();
        tokio::spawn(async move {
            while let Some(Ok(msg)) = stream.next().await {
                if let Message::Text(text) = msg {
                    if let Ok(v) = serde_json::from_str::<Value>(&text) {
                        let all = if let Value::Array(a) = v { a } else { vec![v] };
                        for m in all {
                            dispatch(m, &waiting, &events);
                        }
                    }
                }
            }
            // The connection ended: nobody will answer the calls still waiting.
            for (_, tx) in waiting.lock().unwrap().drain() {
                let _ = tx.send(Err(anyhow::anyhow!("the connection to the host closed")));
            }
        });
        let link = Link { out, pending, next_id: AtomicU64::new(1) };
        link.call(Call::Hello { token: token.to_string() }).await.context("the host refused this client")?;
        Ok((link, events_rx))
    }

    /// Sends a request and waits for its reply.
    pub async fn call(&self, call: Call) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        let req = Request { jsonrpc: "2.0".into(), id, call };
        self.out.send(Message::Text(serde_json::to_string(&req)?.into())).ok().context("the connection to the host closed")?;
        rx.await.context("the connection to the host closed")?
    }
}

fn dispatch(m: Value, pending: &Pending, events: &mpsc::UnboundedSender<Incoming>) {
    if let Some(id) = m["id"].as_u64().filter(|_| m.get("method").is_none()) {
        let Some(tx) = pending.lock().unwrap().remove(&id) else { return };
        let result = match serde_json::from_value::<RpcError>(m["error"].clone()) {
            Ok(e) => Err(anyhow::anyhow!("{} (code {})", e.message, e.code)),
            Err(_) => Ok(m["result"].clone()),
        };
        let _ = tx.send(result);
        return;
    }
    match m["method"].as_str() {
        Some(Notification::EVENT) => {
            if let Ok(e) = serde_json::from_value(m["params"].clone()) {
                let _ = events.send(Incoming::Event(e));
            }
        }
        Some(Notification::ACTIVITY) => {
            let p = &m["params"];
            let (thread_id, kind) = (p["thread_id"].as_str().unwrap_or("").to_string(), p["kind"].as_str().unwrap_or("").to_string());
            let _ = events.send(Incoming::Activity { thread_id, kind });
        }
        Some(Notification::CLOSING) => {
            let reason = m["params"]["reason"].as_str().unwrap_or("no reason given").to_string();
            let _ = events.send(Incoming::Closing(reason));
        }
        _ => {}
    }
}

/// Parses a reply's result into a type.
pub fn parse<T: serde::de::DeserializeOwned>(v: Value) -> Result<T> {
    if v.is_null() {
        bail!("the host sent no result");
    }
    Ok(serde_json::from_value(v)?)
}
