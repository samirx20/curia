//! A thread whose CLI is running: its session, and the task that stores its events.
use super::ops::snapshot;
use super::{Host, HostEvent, lock};
use crate::claude::CONTROL_ROOM;
use crate::cli::{self, ControlRoom, Session};
use crate::record::Recorder;
use crate::store::ThreadRow;
use anyhow::{Context, Result};
use curia_core::Event;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

pub struct Live {
    pub session: Session,
    /// The checkpoint taken just before a message that starts a turn; the turn claims it.
    pending_before: Mutex<Option<String>>,
    /// Who sent the message that starts the next turn (a thread id, or `curia`); the turn claims it.
    pending_by: Mutex<Option<String>>,
    /// Tool calls started and not yet finished. Steering a message in while one runs would
    /// cut it off, so delivery queues instead.
    open_tools: AtomicI64,
}

impl Live {
    pub fn set_pending_before(&self, commit: Option<String>) {
        *lock(&self.pending_before) = commit;
    }

    pub fn set_pending_by(&self, by: Option<String>) {
        *lock(&self.pending_by) = by;
    }

    /// Whether a message can be slipped into the running turn now: the CLI must take
    /// mid-turn messages into the turn, and no tool may be running (it would be cut off).
    pub fn can_steer(&self) -> bool {
        self.session.steers_mid_turn() && self.open_tools.load(Ordering::SeqCst) == 0
    }
}

impl Host {
    /// The thread's running CLI, started (resuming its session) if needed.
    pub(super) async fn live(self: &Arc<Self>, row: &ThreadRow) -> Result<Arc<Live>> {
        if let Some(live) = self.live_of(&row.id) {
            return Ok(live);
        }
        let control = match self.mcp_url.get() {
            Some(url) => {
                let (config_file, token) = self.write_mcp_config(&row.id, url)?;
                Some(ControlRoom { url: url.clone(), token, config_file, home: self.thread_home(&row.id) })
            }
            None => None,
        };
        let token = control.as_ref().map(|c| c.token.clone());
        let instructions = control.as_ref().map(|_| super::control::instructions(row));
        let recorder = Recorder::create(&self.transcript_path(&row.id)?)?;
        let started = cli::start(row, control.as_ref(), instructions, recorder).await;
        let (session, rx) = match started {
            Ok(s) => s,
            Err(e) => {
                self.revoke(&row.id, token.as_deref());
                return Err(e);
            }
        };
        let live = Arc::new(Live { session, pending_before: Mutex::new(None), pending_by: Mutex::new(None), open_tools: AtomicI64::new(0) });
        lock(&self.live).insert(row.id.clone(), live.clone());
        let (me, id, session) = (self.clone(), row.id.clone(), live.session.clone());
        tokio::spawn(async move { me.refresh_usage(&id, &session).await });
        tokio::spawn(self.clone().pump(row.id.clone(), row.cwd.clone(), live.clone(), rx, token));
        Ok(live)
    }

    /// Issues a token for this thread and writes the MCP config Claude reads it from.
    fn write_mcp_config(&self, thread_id: &str, url: &str) -> Result<(PathBuf, String)> {
        let token = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
        let dir = self.data_dir.join("mcp");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{thread_id}.json"));
        let config = serde_json::json!({"mcpServers": {CONTROL_ROOM: {
            "type": "http", "url": url, "headers": {"Authorization": format!("Bearer {token}")}}}});
        std::fs::write(&path, config.to_string()).context("could not write the thread's MCP config")?;
        lock(&self.tokens).insert(token.clone(), thread_id.to_string());
        Ok((path, token))
    }

    /// The thread's CLI is gone: its token stops working and its config file goes.
    fn revoke(&self, thread_id: &str, token: Option<&str>) {
        if let Some(t) = token {
            lock(&self.tokens).remove(t);
        }
        let _ = std::fs::remove_file(self.data_dir.join("mcp").join(format!("{thread_id}.json")));
        if let Err(e) = crate::links::remove_tree(&self.thread_home(thread_id)) {
            eprintln!("curia-host: could not remove thread {thread_id}'s folder: {e:#}");
        }
    }

    /// A folder of the thread's own for launch files (agy's overlay); gone when the CLI stops.
    fn thread_home(&self, thread_id: &str) -> PathBuf {
        self.data_dir.join("homes").join(thread_id)
    }

    fn transcript_path(&self, thread_id: &str) -> Result<PathBuf> {
        let dir = self.data_dir.join("transcripts").join(thread_id);
        std::fs::create_dir_all(&dir)?;
        Ok(dir.join(format!("{}.jsonl", crate::store::now_ms())))
    }

    /// Stores each event the CLI produces, takes turn checkpoints, tells subscribers, and
    /// after each turn hands results back and delivers queued messages.
    async fn pump(self: Arc<Self>, thread_id: String, cwd: String, live: Arc<Live>, mut rx: mpsc::UnboundedReceiver<Event>, token: Option<String>) {
        while let Some(event) = rx.recv().await {
            match &event {
                Event::TextDelta { .. } => {
                    let _ = self.events.send(HostEvent { thread_id: thread_id.clone(), seq: None, event });
                    continue;
                }
                Event::ToolCall { .. } => {
                    live.open_tools.fetch_add(1, Ordering::SeqCst);
                }
                Event::ToolResult { .. } => {
                    live.open_tools.fetch_sub(1, Ordering::SeqCst);
                }
                Event::TurnFinished { .. } => live.open_tools.store(0, Ordering::SeqCst),
                Event::Usage { windows, limited } => {
                    if let Err(e) = self.usage_seen(&thread_id, windows, *limited) {
                        eprintln!("curia-host: could not save usage: {e:#}");
                    }
                }
                _ => {}
            }
            // A turn says who started it (the adapter can't know; the host does).
            let event = match event {
                Event::TurnStarted { prompt, by: None } => Event::TurnStarted { prompt, by: lock(&live.pending_by).take() },
                other => other,
            };
            let exited = matches!(event, Event::Exited { .. });
            let asked = match &event {
                Event::ApprovalRequested { request_id, tool_name, input, .. } => Some((request_id.clone(), tool_name.clone(), input.clone())),
                _ => None,
            };
            let stored = self.store_one(&thread_id, &cwd, &live, event).await;
            if let (Ok(()), Some((request_id, tool, input))) = (&stored, asked) {
                self.request_arrived(&thread_id, &request_id, &tool, &input);
            }
            if let Err(e) = stored {
                eprintln!("curia-host: could not store an event for thread {thread_id}: {e:#}");
                let text = format!("Curia could not save part of this thread: {e:#}");
                let _ = self.events.send(HostEvent { thread_id: thread_id.clone(), seq: None, event: Event::Notice { text } });
            }
            if exited {
                break;
            }
        }
        lock(&self.live).remove(&thread_id);
        self.revoke(&thread_id, token.as_deref());
    }

    async fn store_one(self: &Arc<Self>, thread_id: &str, cwd: &str, live: &Live, event: Event) -> Result<()> {
        let stored = self.store.record(thread_id, &[event])?;
        self.store.thread_set_items_used(thread_id, live.session.items_used())?;
        for s in stored {
            match (&s.event, &s.turn_id) {
                (Event::TurnStarted { .. }, Some(turn)) => {
                    // A turn Claude started by itself had no message of ours: snapshot now.
                    let claimed = lock(&live.pending_before).take();
                    let before = match claimed {
                        Some(c) => Some(c),
                        None => snapshot(cwd, "curia: before turn").await?,
                    };
                    if let Some(c) = before {
                        self.store.turn_set_checkpoint(turn, true, &c)?;
                    }
                }
                (Event::TurnFinished { outcome, .. }, Some(turn)) => {
                    if let Some(c) = snapshot(cwd, "curia: after turn").await? {
                        self.store.turn_set_checkpoint(turn, false, &c)?;
                    }
                    tokio::spawn(self.clone().after_turn(thread_id.to_string(), turn.clone(), outcome.clone()));
                }
                _ => {}
            }
            let _ = self.events.send(HostEvent { thread_id: thread_id.to_string(), seq: Some(s.seq), event: s.event });
        }
        Ok(())
    }
}
