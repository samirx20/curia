//! One live Claude Code process: writes turns and answers to stdin, reads stdout through the
//! mapper, and reports shared events. Every line in both directions can be recorded.
use super::{Launch, Mapper, wire};
use crate::process::spawn_contained;
use crate::record::{Entry, Recorder};
use anyhow::{Context, Result};
use curia_core::{Decision, Event};
use serde_json::Value;
use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::ChildStdin;
use tokio::sync::{mpsc, oneshot};

/// What the session sends to its owner.
pub type Out = mpsc::UnboundedReceiver<Event>;

pub struct ClaudeSession {
    stdin: tokio::sync::Mutex<Option<ChildStdin>>,
    mapper: Mutex<Mapper>,
    events: mpsc::UnboundedSender<Event>,
    recorder: Option<Recorder>,
    stderr_tail: Mutex<VecDeque<String>>,
    kill: Mutex<Option<oneshot::Sender<()>>>,
    next_control: AtomicU64,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl ClaudeSession {
    /// Starts Claude. `items_so_far` is how many item ids the thread already used (0 for a new one).
    pub async fn start(exe: &Path, launch: &Launch, items_so_far: u64, recorder: Option<Recorder>) -> Result<(Arc<ClaudeSession>, Out)> {
        if let Some(r) = &recorder {
            r.write(&Entry::Start { ms: r.ms(), items_so_far })?;
        }
        let mut proc = spawn_contained(launch.command(exe), "Claude Code")?;
        let stdin = proc.child.stdin.take().context("no stdin")?;
        let stdout = proc.child.stdout.take().context("no stdout")?;
        let stderr = proc.child.stderr.take().context("no stderr")?;
        let (tx, rx) = mpsc::unbounded_channel();
        let (kill_tx, kill_rx) = oneshot::channel();
        let s = Arc::new(ClaudeSession {
            stdin: tokio::sync::Mutex::new(Some(stdin)),
            mapper: Mutex::new(Mapper::starting_at(items_so_far)),
            events: tx,
            recorder,
            stderr_tail: Mutex::new(VecDeque::new()),
            kill: Mutex::new(Some(kill_tx)),
            next_control: AtomicU64::new(0),
        });

        let me = s.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                me.record(Entry::Err { ms: me.ms(), line: line.clone() });
                let mut tail = lock(&me.stderr_tail);
                if tail.len() == 20 {
                    tail.pop_front();
                }
                tail.push_back(line);
            }
        });

        // The waiter owns the process: it ends on exit or on a kill request, whatever stdout is doing.
        let (exit_tx, exit_rx) = oneshot::channel();
        tokio::spawn(async move {
            let (mut child, job) = (proc.child, proc.job);
            let code = tokio::select! {
                status = child.wait() => status.ok().and_then(|s| s.code()),
                _ = kill_rx => { let _ = child.start_kill(); child.wait().await.ok().and_then(|s| s.code()) }
            };
            drop(job); // anything the CLI left running dies with its job, which also closes stdout
            let _ = exit_tx.send(code);
        });

        let me = s.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                me.on_line(line).await;
            }
            // Every line is handled before the exit is reported.
            let code = exit_rx.await.ok().flatten();
            me.record(Entry::Exit { ms: me.ms(), code });
            let tail = lock(&me.stderr_tail).iter().rev().take(3).rev().cloned().collect::<Vec<_>>().join(" | ");
            let events = lock(&me.mapper).on_exit(code, &tail);
            me.emit(events);
        });

        s.control(wire::initialize()).await?;
        Ok((s, rx))
    }

    async fn on_line(&self, line: String) {
        self.record(Entry::Out { ms: self.ms(), line: line.clone() });
        let step = match serde_json::from_str::<Value>(&line) {
            Ok(v) => lock(&self.mapper).handle(&v),
            Err(_) => {
                self.emit(vec![Event::Notice { text: format!("Unreadable line from Claude: {line}") }]);
                return;
            }
        };
        for reply in &step.replies {
            if let Err(e) = self.write(reply).await {
                self.emit(vec![Event::Notice { text: format!("Could not answer Claude: {e:#}") }]);
            }
        }
        self.emit(step.events);
    }

    /// Starts a turn (or adds to the running one).
    pub async fn send(&self, text: &str) -> Result<()> {
        let session_id = lock(&self.mapper).session_id().map(str::to_string);
        self.write(&wire::user(Value::String(text.into()), session_id.as_deref())).await?;
        let events = lock(&self.mapper).begin_turn(text);
        self.emit(events);
        Ok(())
    }

    pub async fn answer(&self, request_id: &str, decision: &Decision) -> Result<()> {
        let (reply, event) = lock(&self.mapper).resolve(request_id, decision)?;
        self.write(&reply).await?;
        self.emit(vec![event]);
        Ok(())
    }

    pub async fn interrupt(&self) -> Result<()> {
        self.control(wire::interrupt()).await
    }

    pub async fn set_effort(&self, level: &str) -> Result<()> {
        self.control(wire::set_effort(level)).await
    }

    /// Item ids the thread has used so far (store it; the next process continues from it).
    pub fn items_used(&self) -> u64 {
        lock(&self.mapper).items_used()
    }

    /// Closes stdin (Claude exits about a second later); kills the tree if it's still there after 3 s.
    pub async fn close(self: &Arc<Self>) {
        self.stdin.lock().await.take();
        let me = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            if let Some(kill) = lock(&me.kill).take() {
                let _ = kill.send(());
            }
        });
    }

    async fn control(&self, request: Value) -> Result<()> {
        let id = format!("curia-{}", self.next_control.fetch_add(1, Ordering::Relaxed) + 1);
        self.write(&wire::control(&id, request)).await
    }

    async fn write(&self, frame: &Value) -> Result<()> {
        let line = frame.to_string();
        let mut guard = self.stdin.lock().await;
        let stdin = guard.as_mut().context("Claude's input is closed")?;
        stdin.write_all(line.as_bytes()).await.context("writing to Claude")?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        self.record(Entry::In { ms: self.ms(), line });
        Ok(())
    }

    fn emit(&self, events: Vec<Event>) {
        for event in events {
            self.record(Entry::Event { ms: self.ms(), event: event.clone() });
            let _ = self.events.send(event);
        }
    }

    fn ms(&self) -> u64 {
        self.recorder.as_ref().map_or(0, Recorder::ms)
    }

    fn record(&self, entry: Entry) {
        if let Some(r) = &self.recorder {
            if let Err(e) = r.write(&entry) {
                let _ = self.events.send(Event::Notice { text: format!("Recording failed: {e:#}") });
            }
        }
    }
}
