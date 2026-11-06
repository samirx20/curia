//! One live `codex app-server`: a JSON-RPC client over `LineProcess`. Starts or resumes the
//! Codex thread, starts turns (or steers a message into the running one), answers approvals,
//! interrupts, and reads quota. Every request waits for its reply, with a time limit.
use super::mapper::{Mapper, rate_limits};
use super::{Launch, wire};
use crate::process::{LineProcess, Output};
use crate::record::Recorder;
use anyhow::{Context, Result, bail};
use curia_core::{Decision, Event, UsageWindow};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

type Reply = Result<Value, String>;

pub struct CodexSession {
    proc: Arc<LineProcess>,
    mapper: Mutex<Mapper>,
    next_id: AtomicU64,
    pending: Mutex<HashMap<u64, oneshot::Sender<Reply>>>,
    effort: Mutex<Option<String>>,
    thread_id: Mutex<Option<String>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl CodexSession {
    pub async fn start(exe: &Path, launch: &Launch, items_so_far: u64, recorder: Option<Recorder>) -> Result<(Arc<CodexSession>, mpsc::UnboundedReceiver<Event>)> {
        let (tx, rx) = mpsc::unbounded_channel();
        let (proc, out) = LineProcess::start(launch.command(exe), "Codex", "codex", recorder, items_so_far, tx)?;
        let s = Arc::new(CodexSession {
            proc,
            mapper: Mutex::new(Mapper::starting_at(items_so_far)),
            next_id: AtomicU64::new(0),
            pending: Mutex::new(HashMap::new()),
            effort: Mutex::new(launch.effort.clone()),
            thread_id: Mutex::new(None),
        });
        tokio::spawn(s.clone().read(out));
        s.call("initialize", wire::initialize_params(), 60).await.context("Codex didn't start")?;
        s.proc.write(&wire::initialized()).await?;
        let (approval, sandbox) = launch.policy();
        let mut params = json!({"cwd": launch.cwd, "approvalPolicy": approval, "sandbox": sandbox,
            "model": launch.model, "developerInstructions": launch.instructions});
        let method = match &launch.resume {
            Some(id) => {
                params["threadId"] = json!(id);
                "thread/resume"
            }
            None => "thread/start",
        };
        let started = s.call(method, params, 120).await.with_context(|| format!("Codex's {method} failed"))?;
        let id = started["thread"]["id"].as_str().context("Codex gave no thread id")?.to_string();
        *lock(&s.thread_id) = Some(id.clone());
        // A resumed thread sends no thread/started: announce the session ourselves.
        let model = started["model"].clone();
        let events = lock(&s.mapper).handle(&json!({"method": "thread/started", "params": {"thread": {"id": id}, "model": model}})).events;
        s.proc.emit(events);
        Ok((s, rx))
    }

    async fn read(self: Arc<Self>, mut out: Output) {
        while let Some(line) = out.lines.recv().await {
            self.on_line(line).await;
        }
        let code = out.exit.await.ok().flatten();
        let tail = self.proc.exited(code);
        for (_, tx) in lock(&self.pending).drain() {
            let _ = tx.send(Err("Codex stopped".into()));
        }
        let events = lock(&self.mapper).on_exit(code, &tail);
        self.proc.emit(events);
    }

    async fn on_line(&self, line: String) {
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            return self.proc.notice(format!("Unreadable line from Codex: {line}"));
        };
        // A reply to one of our requests.
        if v.get("method").is_none() {
            if let Some(tx) = v["id"].as_u64().and_then(|id| lock(&self.pending).remove(&id)) {
                let r = if v["error"].is_object() { Err(v["error"]["message"].as_str().unwrap_or("error").to_string()) } else { Ok(v["result"].clone()) };
                let _ = tx.send(r);
            }
            return;
        }
        let step = lock(&self.mapper).handle(&v);
        for reply in &step.replies {
            if let Err(e) = self.proc.write(reply).await {
                self.proc.notice(format!("Could not answer Codex: {e:#}"));
            }
        }
        self.proc.emit(step.events);
    }

    /// Starts a turn, or steers the message into the running one (Codex takes it mid-turn).
    pub async fn send(&self, text: &str) -> Result<()> {
        let thread = self.thread()?;
        let running = lock(&self.mapper).running_turn().map(str::to_string);
        if let Some(turn) = running {
            lock(&self.mapper).begin_turn(text, true);
            self.call("turn/steer", json!({"threadId": thread, "expectedTurnId": turn, "input": wire::text_input(text)}), 60).await?;
            return Ok(());
        }
        let events = lock(&self.mapper).begin_turn(text, false);
        self.proc.emit(events);
        let effort = lock(&self.effort).clone();
        self.call("turn/start", json!({"threadId": thread, "input": wire::text_input(text), "effort": effort}), 60).await?;
        Ok(())
    }

    pub async fn answer(&self, request_id: &str, decision: &Decision) -> Result<()> {
        let (reply, event) = lock(&self.mapper).resolve(request_id, decision)?;
        self.proc.write(&reply).await?;
        self.proc.emit(vec![event]);
        Ok(())
    }

    pub async fn interrupt(&self) -> Result<()> {
        let thread = self.thread()?;
        let turn = lock(&self.mapper).running_turn().map(str::to_string).context("no turn is running")?;
        self.call("turn/interrupt", json!({"threadId": thread, "turnId": turn}), 30).await.map(|_| ())
    }

    /// Codex takes effort per turn: the next turn uses it.
    pub async fn set_effort(&self, level: &str) -> Result<()> {
        *lock(&self.effort) = Some(level.to_string());
        Ok(())
    }

    pub async fn usage(&self) -> Result<Vec<UsageWindow>> {
        let r = self.call("account/rateLimits/read", json!({}), 30).await?;
        match rate_limits(&r["rateLimits"]) {
            Event::Usage { windows, .. } => Ok(windows),
            _ => Ok(Vec::new()),
        }
    }

    pub fn pid(&self) -> Option<u32> {
        self.proc.pid()
    }

    pub fn items_used(&self) -> u64 {
        lock(&self.mapper).items_used()
    }

    pub async fn close(&self) {
        self.proc.close().await
    }

    fn thread(&self) -> Result<String> {
        lock(&self.thread_id).clone().context("Codex has no thread yet")
    }

    /// One request and its reply, or an error if none comes within `secs`.
    async fn call(&self, method: &str, params: Value, secs: u64) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let (tx, rx) = oneshot::channel();
        lock(&self.pending).insert(id, tx);
        if let Err(e) = self.proc.write(&wire::request(id, method, params)).await {
            lock(&self.pending).remove(&id);
            return Err(e);
        }
        match tokio::time::timeout(Duration::from_secs(secs), rx).await {
            Ok(Ok(Ok(v))) => Ok(v),
            Ok(Ok(Err(e))) => bail!("Codex refused {method}: {e}"),
            Ok(Err(_)) => bail!("Codex stopped before answering {method}"),
            Err(_) => {
                lock(&self.pending).remove(&id);
                bail!("Codex did not answer {method} within {secs} s")
            }
        }
    }
}
