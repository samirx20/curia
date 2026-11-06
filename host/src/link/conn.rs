//! One client connection: sign-in, requests (each checked against the client's role), and the
//! threads it follows.
//!
//! Opening a thread can't miss or repeat an event: the connection listens to the host's
//! events before it reads the log, holds that thread's events while it reads, then passes on
//! only those newer than what the client was sent.
use super::auth::{self, Gate, Who};
use super::open::{info, last_seq, open, wire};
use super::signin;
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

pub async fn handle(host: Arc<Host>, gate: Arc<Gate>, socket: WebSocket) {
    let (sink, mut stream) = socket.split();
    let outbox = Outbox::start(sink);
    let subs: Subs = Arc::default();
    let forwarder = tokio::spawn(forward(host.clone(), subs.clone(), outbox.clone()));
    let mut revoked = gate.revoked.subscribe();
    let mut who: Option<Who> = None;
    let mut nonce: Option<String> = None;
    let mut watching: HashMap<String, tokio::task::JoinHandle<()>> = HashMap::new();
    loop {
        let msg = tokio::select! {
            m = stream.next() => match m { Some(Ok(m)) => m, _ => break },
            r = revoked.recv() => {
                if let (Ok(id), Some(Who::Device { id: mine, .. })) = (r, &who) {
                    if id == *mine {
                        outbox.close("this device was unpaired");
                        break;
                    }
                }
                continue;
            }
        };
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
        let Some(me) = who.clone() else {
            match signin::greet(&host, &gate, &req.call, &mut nonce) {
                Ok((value, signed)) => {
                    who = signed;
                    outbox.push(Out::Reply(ok(req.id, value)));
                }
                Err(refused) => {
                    outbox.push(Out::Reply(error(req.id, code::UNAUTHORIZED, &refused.message)));
                    if !refused.retry {
                        outbox.close("not signed in");
                        break;
                    }
                }
            }
            continue;
        };
        let needs = auth::required(&req.call);
        if needs > me.role() {
            let msg = format!("this device may only {}", if me.role() == auth::Role::View { "look" } else { "look and answer requests" });
            outbox.push(Out::Reply(error(req.id, code::UNAUTHORIZED, &msg)));
            continue;
        }
        if let Call::ThreadOpen { thread_id, after_seq } = &req.call {
            open_thread(&host, &subs, &outbox, req.id, thread_id, *after_seq);
            continue;
        }
        if let Call::TerminalOpen { id } = &req.call {
            let reply = match host.terms.get(id) {
                Ok(shell) => {
                    let view = crate::term::view(&shell);
                    if let Some(old) = watching.insert(id.clone(), tokio::spawn(watch_terminal(shell, outbox.clone()))) {
                        old.abort();
                    }
                    ok(req.id, view)
                }
                Err(e) => error(req.id, code::FAILED, &format!("{e:#}")),
            };
            outbox.push(Out::Reply(reply));
            continue;
        }
        let result = match signin::devices(&host, &gate, &me, &req.call) {
            Some(r) => r,
            None => answer_or_dispatch(&host, &me, req.call.clone()).await,
        };
        outbox.push(Out::Reply(match result {
            Ok(v) => ok(req.id, v),
            Err(e) => error(req.id, code::FAILED, &format!("{e:#}")),
        }));
    }
    forwarder.abort();
    for (_, w) in watching {
        w.abort();
    }
}

/// Sends a terminal's screen whenever it changes, at most about 25 times a second.
async fn watch_terminal(shell: Arc<crate::term::Shell>, outbox: Outbox) {
    let mut changed = shell.version.subscribe();
    while changed.changed().await.is_ok() {
        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
        changed.borrow_and_update();
        let params = crate::term::view(&shell);
        outbox.push(Out::Note(json!({"jsonrpc": "2.0", "method": Notification::TERMINAL, "params": params})));
    }
}

/// Runs a call; an answer is checked for the device's signature first and logged with who gave it.
async fn answer_or_dispatch(host: &Arc<Host>, who: &Who, call: Call) -> Result<Value> {
    signin::check_answer(host, who, &call)?;
    let answered = match &call {
        Call::ThreadAnswer { thread_id, request_id, .. } => Some((thread_id.clone(), request_id.clone())),
        _ => None,
    };
    let value = dispatch(host, call).await?;
    if let Some((thread_id, request_id)) = answered {
        host.store.request_answered_by(&thread_id, &request_id, who.label())?;
    }
    Ok(value)
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
        Call::ThreadAnswer { thread_id, request_id, decision, .. } => {
            host.answer(&thread_id, &request_id, &decision).await.map(|_| Value::Null)?
        }
        Call::ThreadInterrupt { thread_id } => host.interrupt(&thread_id).await.map(|_| Value::Null)?,
        Call::TerminalList { thread_id } => host.terms.list(&thread_id),
        Call::HostInfo => host.host_info().await?,
        Call::TeamUpdate { id, name, model, effort } => {
            host.store.team_update(&id, name.as_deref(), model.as_deref(), effort.as_deref()).map(|_| Value::Null)?
        }
        Call::TeamRemove { id } => host.store.team_remove(&id).map(|_| Value::Null)?,
        Call::TeamSignIn { cli, config_dir } => host.team_sign_in(&cli, &config_dir).map(|_| Value::Null)?,
        Call::AutostartSet { enabled } => host.autostart_set(enabled).map(|_| Value::Null)?,
        Call::Search { query } => json!(host.store.search(&query, 30)?),
        Call::RunSummary { thread_id } => host.store.run_summary(&host.store.thread_get(&thread_id)?.root_id)?,
        Call::ThreadTree { thread_id } => host.thread_tree(&thread_id).await?,
        Call::ThreadPorts { thread_id } => host.thread_ports(&thread_id)?,
        Call::FileLink { thread_id, path, rev } => host.file_link(&thread_id, &path, rev.as_deref())?,
        Call::TerminalStart { thread_id, cols, rows } => {
            let cwd = host.store.thread_get(&thread_id)?.cwd;
            let shell = host.terms.start(&thread_id, cwd.into(), crate::term::Size { cols: cols.clamp(20, 400), rows: rows.clamp(5, 200) }).await?;
            crate::term::view(&shell)
        }
        Call::TerminalInput { id, data } => host.terms.get(&id)?.input(&data).map(|_| Value::Null)?,
        Call::TerminalResize { id, cols, rows } => host.terms.get(&id)?.resize(crate::term::Size { cols, rows }).map(|_| Value::Null)?,
        Call::TerminalClose { id } => host.terms.close(&id).map(|_| Value::Null)?,
        Call::TerminalOpen { .. } => bail!("terminal_open is handled before dispatch"),
        Call::Hello { .. } | Call::Challenge | Call::HelloDevice { .. } | Call::Pair { .. } => bail!("already signed in"),
        Call::PairCode { .. } | Call::DevicesList | Call::DeviceRevoke { .. } | Call::DeviceSetRole { .. } | Call::PushKey | Call::PushSubscribe { .. } => {
            bail!("device calls are handled before dispatch")
        }
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
