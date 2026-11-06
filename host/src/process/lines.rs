//! A CLI that speaks one JSON message per line on stdin/stdout. The plumbing every adapter
//! shares: a contained process, a writer, stdout lines in order, the stderr tail, the exit
//! (noticed even if the CLI hangs), and a recording of every line and event.
use super::spawn_contained;
use crate::record::{Entry, Recorder};
use anyhow::{Context, Result};
use curia_core::Event;
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, Command};
use tokio::sync::{mpsc, oneshot};

pub struct LineProcess {
    /// Shown in errors ("Claude Code", "Codex").
    name: &'static str,
    stdin: tokio::sync::Mutex<Option<ChildStdin>>,
    recorder: Option<Recorder>,
    stderr_tail: Mutex<VecDeque<String>>,
    kill: Mutex<Option<oneshot::Sender<()>>>,
    events: mpsc::UnboundedSender<Event>,
}

/// What the process produces: stdout lines in order, then its exit code.
pub struct Output {
    pub lines: mpsc::UnboundedReceiver<String>,
    pub exit: oneshot::Receiver<Option<i32>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl LineProcess {
    /// Starts `cmd` contained. `items_so_far` goes in the recording, so a replay continues
    /// item ids where this process did. Events are sent on `events`.
    /// `cli` names the adapter in the recording (`claude`, `codex`), so a replay picks it.
    pub fn start(cmd: Command, name: &'static str, cli: &str, recorder: Option<Recorder>, items_so_far: u64, events: mpsc::UnboundedSender<Event>) -> Result<(Arc<LineProcess>, Output)> {
        if let Some(r) = &recorder {
            r.write(&Entry::Start { ms: r.ms(), items_so_far, cli: Some(cli.to_string()) })?;
        }
        let mut proc = spawn_contained(cmd, name)?;
        let stdin = proc.child.stdin.take().context("no stdin")?;
        let stdout = proc.child.stdout.take().context("no stdout")?;
        let stderr = proc.child.stderr.take().context("no stderr")?;
        let (kill_tx, kill_rx) = oneshot::channel();
        let p = Arc::new(LineProcess {
            name,
            stdin: tokio::sync::Mutex::new(Some(stdin)),
            recorder,
            stderr_tail: Mutex::new(VecDeque::new()),
            kill: Mutex::new(Some(kill_tx)),
            events,
        });

        let me = p.clone();
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

        let (line_tx, line_rx) = mpsc::unbounded_channel();
        let me = p.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                me.record(Entry::Out { ms: me.ms(), line: line.clone() });
                if line_tx.send(line).is_err() {
                    break;
                }
            }
        });
        Ok((p, Output { lines: line_rx, exit: exit_rx }))
    }

    /// Writes one message (and records it). If the CLI has stopped, the error says what it
    /// last printed, which is usually why.
    pub async fn write(&self, frame: &Value) -> Result<()> {
        let line = frame.to_string();
        let written = async {
            let mut guard = self.stdin.lock().await;
            let stdin = guard.as_mut().with_context(|| format!("{}'s input is closed", self.name))?;
            stdin.write_all(line.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
            stdin.flush().await?;
            anyhow::Ok(())
        }
        .await;
        if let Err(e) = written {
            let tail = self.stderr_last();
            return Err(if tail.is_empty() { e.context(format!("could not write to {}", self.name)) } else { e.context(format!("{} stopped: {tail}", self.name)) });
        }
        self.record(Entry::In { ms: self.ms(), line });
        Ok(())
    }

    /// Records a step Curia took that the CLI never reads (agy's stop is closing it), so a
    /// replay can take the same step.
    pub fn note(&self, frame: &Value) {
        self.record(Entry::In { ms: self.ms(), line: frame.to_string() });
    }

    /// Records events and sends them to the session's owner.
    pub fn emit(&self, events: Vec<Event>) {
        for event in events {
            self.record(Entry::Event { ms: self.ms(), event: event.clone() });
            let _ = self.events.send(event);
        }
    }

    pub fn notice(&self, text: String) {
        self.emit(vec![Event::Notice { text }]);
    }

    /// The process ended: recorded, and the last stderr lines returned for the error message.
    pub fn exited(&self, code: Option<i32>) -> String {
        self.record(Entry::Exit { ms: self.ms(), code });
        self.stderr_last()
    }

    /// The last few stderr lines, joined.
    fn stderr_last(&self) -> String {
        lock(&self.stderr_tail).iter().rev().take(3).rev().cloned().collect::<Vec<_>>().join(" | ")
    }

    /// Closes stdin (a CLI exits on its own soon after); kills the tree if it's still there after 3 s.
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
