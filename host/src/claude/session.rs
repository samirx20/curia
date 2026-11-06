//! One live Claude Code process: writes turns and answers to stdin, reads stdout through the
//! mapper, and reports shared events. The process plumbing is `process::LineProcess`.
use super::{Launch, Mapper, wire};
use crate::process::{LineProcess, Output};
use crate::record::Recorder;
use anyhow::Result;
use curia_core::{Decision, Event};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};

pub struct ClaudeSession {
    proc: Arc<LineProcess>,
    mapper: Mutex<Mapper>,
    next_control: AtomicU64,
    /// Control requests waiting for Claude's reply, by request id.
    replies: Mutex<HashMap<String, oneshot::Sender<Result<Value, String>>>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl ClaudeSession {
    /// Starts Claude. `items_so_far` is how many item ids the thread already used (0 for a new one).
    pub async fn start(exe: &Path, launch: &Launch, items_so_far: u64, recorder: Option<Recorder>) -> Result<(Arc<ClaudeSession>, mpsc::UnboundedReceiver<Event>)> {
        let (tx, rx) = mpsc::unbounded_channel();
        let (proc, out) = LineProcess::start(launch.command(exe), "Claude Code", "claude", recorder, items_so_far, tx)?;
        let s = Arc::new(ClaudeSession {
            proc,
            mapper: Mutex::new(Mapper::starting_at(items_so_far)),
            next_control: AtomicU64::new(0),
            replies: Mutex::new(HashMap::new()),
        });
        tokio::spawn(s.clone().read(out));
        s.control(wire::initialize()).await?;
        Ok((s, rx))
    }

    /// Every line, in order; then the exit, which ends a turn still running.
    async fn read(self: Arc<Self>, mut out: Output) {
        while let Some(line) = out.lines.recv().await {
            self.on_line(line).await;
        }
        let code = out.exit.await.ok().flatten();
        let tail = self.proc.exited(code);
        let events = lock(&self.mapper).on_exit(code, &tail);
        self.proc.emit(events);
    }

    async fn on_line(&self, line: String) {
        let step = match serde_json::from_str::<Value>(&line) {
            Ok(v) if v["type"] == "control_response" && self.answered(&v) => return,
            Ok(v) => lock(&self.mapper).handle(&v),
            Err(_) => return self.proc.notice(format!("Unreadable line from Claude: {line}")),
        };
        for reply in &step.replies {
            if let Err(e) = self.proc.write(reply).await {
                self.proc.notice(format!("Could not answer Claude: {e:#}"));
            }
        }
        self.proc.emit(step.events);
    }

    /// Starts a turn (or adds to the running one).
    pub async fn send(&self, text: &str) -> Result<()> {
        let session_id = lock(&self.mapper).session_id().map(str::to_string);
        self.proc.write(&wire::user(Value::String(text.into()), session_id.as_deref())).await?;
        let events = lock(&self.mapper).begin_turn(text);
        self.proc.emit(events);
        Ok(())
    }

    pub async fn answer(&self, request_id: &str, decision: &Decision) -> Result<()> {
        let (reply, event) = lock(&self.mapper).resolve(request_id, decision)?;
        self.proc.write(&reply).await?;
        self.proc.emit(vec![event]);
        Ok(())
    }

    pub async fn interrupt(&self) -> Result<()> {
        self.control(wire::interrupt()).await
    }

    pub async fn set_effort(&self, level: &str) -> Result<()> {
        self.control(wire::set_effort(level)).await
    }

    /// Item ids the thread has used so far (store it; the next process continues from it).
    pub fn pid(&self) -> Option<u32> {
        self.proc.pid()
    }

    pub fn items_used(&self) -> u64 {
        lock(&self.mapper).items_used()
    }

    pub async fn close(&self) {
        self.proc.close().await
    }

    async fn control(&self, request: Value) -> Result<()> {
        let id = format!("curia-{}", self.next_control.fetch_add(1, Ordering::Relaxed) + 1);
        self.proc.write(&wire::control(&id, request)).await
    }

    /// The account's usage windows right now (no model turn, no quota spent).
    pub async fn usage(&self) -> Result<Vec<curia_core::UsageWindow>> {
        Ok(super::usage_reply(&self.ask(wire::get_usage()).await?))
    }

    /// Sends a control request and waits (up to 30 s) for Claude's reply.
    pub async fn ask(&self, request: Value) -> Result<Value> {
        let id = format!("curia-{}", self.next_control.fetch_add(1, Ordering::Relaxed) + 1);
        let (tx, rx) = oneshot::channel();
        lock(&self.replies).insert(id.clone(), tx);
        if let Err(e) = self.proc.write(&wire::control(&id, request)).await {
            lock(&self.replies).remove(&id);
            return Err(e);
        }
        match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
            Ok(Ok(Ok(v))) => Ok(v),
            Ok(Ok(Err(e))) => anyhow::bail!("Claude refused the request: {e}"),
            Ok(Err(_)) => anyhow::bail!("Claude closed before replying"),
            Err(_) => {
                lock(&self.replies).remove(&id);
                anyhow::bail!("Claude did not reply within 30 s")
            }
        }
    }

    /// Hands a control reply to the request waiting for it. False if nobody waits for it.
    fn answered(&self, v: &Value) -> bool {
        let r = &v["response"];
        let Some(tx) = r["request_id"].as_str().and_then(|id| lock(&self.replies).remove(id)) else { return false };
        let result = if r["subtype"] == "error" {
            Err(r["error"].as_str().unwrap_or("unknown error").to_string())
        } else {
            Ok(r["response"].clone())
        };
        let _ = tx.send(result);
        true
    }
}
