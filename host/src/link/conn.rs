//! One client connection: sign-in, requests, and the threads it follows.
//!
//! Opening a thread can't miss or repeat an event: the connection listens to the host's
//! events before it reads the log, holds that thread's events while it reads, then passes on
//! only those newer than what the client was sent.
use super::open::{info, last_seq, open, wire};
use super::outbox::{Out, Outbox};
use crate::host::{Host, HostEvent};
use crate::store::NewThread;
use anyhow::{Result, bail};
use axum::extract::ws::{Message, WebSocket};
use curia_core::Event;
use curia_core::protocol::{Call, Notification, Request, RpcError, TeamInfo, WireEvent, code};
use futures_util::StreamExt;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast::error::RecvError;

enum Sub {
    /// Being opened: events wait here until the client has its starting point.
    Opening(Vec<HostEvent>),
    /// Open: the newest stored event the client has been sent.
    Open { last: i64 },
}

type Subs = Arc<Mutex<HashMap<String, Sub>>>;

pub async fn handle(host: Arc<Host>, token: Arc<String>, socket: WebSocket) {
    let (sink, mut stream) = socket.split();
    let outbox = Outbox::start(sink);
    let subs: Subs = Arc::default();
    let forwarder = tokio::spawn(forward(host.clone(), subs.clone(), outbox.clone()));
    let mut signed_in = false;
    while let Some(Ok(msg)) = stream.next().await {
        let Message::Text(text) = msg else {
            if let Message::Close(_) = msg {
                break;
            }
            continue;
        };
        let req: Request = match serde_json::from_str(&text) {
            Ok(r) => r,
            Err(e) => {
                outbox.push(Out::Reply(error(0, code::INVALID, &format!("not a request: {e}"))));
                continue;
            }
        };
        match (&req.call, signed_in) {
            (Call::Hello { token: t }, _) if t.as_str() == token.as_str() => {
                signed_in = true;
                outbox.push(Out::Reply(ok(req.id, json!({"host": "curia", "version": env!("CARGO_PKG_VERSION")}))));
            }
            (_, false) => {
                outbox.push(Out::Reply(error(req.id, code::UNAUTHORIZED, "send hello with the host's token first")));
                outbox.close("not signed in");
                break;
            }
            (Call::ThreadOpen { thread_id, after_seq }, true) => open_thread(&host, &subs, &outbox, req.id, thread_id, *after_seq),
            (call, true) => {
                let reply = match dispatch(&host, call.clone()).await {
                    Ok(v) => ok(req.id, v),
                    Err(e) => error(req.id, code::FAILED, &format!("{e:#}")),
                };
                outbox.push(Out::Reply(reply));
            }
        }
    }
    forwarder.abort();
}

fn open_thread(host: &Host, subs: &Subs, outbox: &Outbox, id: u64, thread_id: &str, after: Option<i64>) {
    subs.lock().unwrap().insert(thread_id.to_string(), Sub::Opening(Vec::new()));
    let opened = match open(&host.store, thread_id, after) {
        Ok(o) => o,
        Err(e) => {
            subs.lock().unwrap().remove(thread_id);
            outbox.push(Out::Reply(error(id, code::FAILED, &format!("{e:#}"))));
            return;
        }
    };
    let last = last_seq(&opened).or(after).unwrap_or(0);
    let mut subs = subs.lock().unwrap();
    outbox.push(Out::Reply(ok(id, serde_json::to_value(&opened).unwrap_or(Value::Null))));
    if let Some(Sub::Opening(held)) = subs.insert(thread_id.to_string(), Sub::Open { last }) {
        for e in held {
            send(&mut subs, outbox, e);
        }
    }
}

/// Passes host events on for the threads this client follows.
async fn forward(host: Arc<Host>, subs: Subs, outbox: Outbox) {
    let mut rx = host.subscribe();
    loop {
        match rx.recv().await {
            Ok(e) => send(&mut subs.lock().unwrap(), &outbox, e),
            Err(RecvError::Lagged(_)) => {
                outbox.close("missed events; reconnect to get a fresh snapshot");
                return;
            }
            Err(RecvError::Closed) => return,
        }
    }
}

fn send(subs: &mut HashMap<String, Sub>, outbox: &Outbox, e: HostEvent) {
    match subs.get_mut(&e.thread_id) {
        None => {
            // Not followed here, but the inbox and the thread list span every thread.
            let kind = match &e.event {
                Event::TurnStarted { .. } => "turn_started",
                Event::TurnFinished { .. } => "turn_finished",
                Event::ApprovalRequested { .. } => "approval_requested",
                Event::ApprovalResolved { .. } => "approval_resolved",
                Event::HandedOff { .. } => "handed_off",
                Event::HandBack { .. } => "hand_back",
                _ => return,
            };
            let params = json!({"thread_id": e.thread_id, "kind": kind});
            outbox.push(Out::Note(json!({"jsonrpc": "2.0", "method": Notification::ACTIVITY, "params": params})));
        }
        Some(Sub::Opening(held)) => held.push(e),
        Some(Sub::Open { last }) => {
            if let Some(seq) = e.seq {
                if seq <= *last {
                    return; // already in what the client was sent
                }
                *last = seq;
            }
            outbox.push(Out::Event(WireEvent { thread_id: e.thread_id, seq: e.seq, event: e.event }));
        }
    }
}

async fn dispatch(host: &Arc<Host>, call: Call) -> Result<Value> {
    let result = match call {
        Call::ThreadsList => json!(host.store.threads()?.iter().map(info).collect::<Vec<_>>()),
        Call::ThreadCreate { thread: t } => {
            let team = host.store.team(&t.team)?;
            let row = host.create_thread(&NewThread {
                title: t.title, cli: team.cli, config_dir: team.config_dir, cwd: t.cwd,
                model: t.model.or(team.model), effort: t.effort.or(team.effort), permission_mode: t.permission_mode,
                team_id: Some(team.id), ..Default::default()
            })?;
            json!(info(&row))
        }
        Call::TeamsList => json!(host.teams_with_quota(None)?),
        Call::TeamCreate { team: t } => {
            let row = host.store.team_create(&t.name, &t.cli, &t.config_dir, t.model.as_deref(), t.effort.as_deref())?;
            json!(TeamInfo { id: row.id, name: row.name, cli: row.cli, model: row.model })
        }
        Call::MergeUndo { merge_id } => host.undo_merge(&merge_id).await.map(|_| Value::Null)?,
        Call::ThreadMove { thread_id, team } => host.move_thread(&thread_id, &team).await.map(|_| Value::Null)?,
        Call::SettingsGet => serde_json::to_value(host.store.settings()?)?,
        Call::SettingsSet { settings } => {
            let s: crate::store::Settings = serde_json::from_value(settings)?;
            host.store.settings_set(&s)?;
            serde_json::to_value(s)?
        }
        Call::InboxList => json!(host.store.pending_requests()?),
        Call::ThreadConfigure { thread_id, model, effort, freedom } => {
            host.thread_configure(&thread_id, model.as_deref(), effort.as_deref(), freedom.as_deref()).await.map(|_| Value::Null)?
        }
        Call::ThreadFiles { thread_id } => host.thread_files(&thread_id).await?,
        Call::FileRead { thread_id, path } => host.file_read(&thread_id, &path)?,
        Call::FileDiff { thread_id, path } => host.file_diff(&thread_id, &path).await?,
        Call::ThreadHistory { thread_id, before_seq, limit } => {
            let page = host.store.events_before(&thread_id, before_seq, limit.min(500))?;
            json!(page.into_iter().map(|s| wire(&thread_id, s)).collect::<Vec<_>>())
        }
        Call::ThreadSend { thread_id, text } => json!({"delivered": host.deliver(&thread_id, None, &text).await?}),
        Call::ThreadAnswer { thread_id, request_id, decision } => {
            host.answer(&thread_id, &request_id, &decision).await.map(|_| Value::Null)?
        }
        Call::ThreadInterrupt { thread_id } => host.interrupt(&thread_id).await.map(|_| Value::Null)?,
        Call::Hello { .. } => bail!("already signed in"),
        Call::ThreadOpen { .. } => bail!("thread_open is handled before dispatch"),
    };
    Ok(result)
}

fn ok(id: u64, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn error(id: u64, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": RpcError { code, message: message.to_string() }})
}
