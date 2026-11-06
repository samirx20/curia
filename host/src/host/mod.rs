//! The host: owns the data folder, the store and the live CLI processes. Every client
//! action is one of these operations.
mod live;

use crate::claude::find_claude;
use crate::git::checkpoint;
use crate::store::{NewThread, Recovered, Store, ThreadRow};
use anyhow::{Context, Result, bail};
use curia_core::{Decision, Event};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

pub use live::Live;

/// An event as clients see it. `seq` is its place in the stored log; live-only events
/// (streamed text pieces) have none.
#[derive(Debug, Clone)]
pub struct HostEvent {
    pub thread_id: String,
    pub seq: Option<i64>,
    pub event: Event,
}

pub struct Host {
    pub store: Store,
    data_dir: PathBuf,
    claude_exe: PathBuf,
    live: Mutex<HashMap<String, Arc<Live>>>,
    events: broadcast::Sender<HostEvent>,
    _lock: std::fs::File,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl Host {
    /// Opens (or creates) a data folder. Only one host may use a folder at a time; the lock
    /// is held by the open file, so a stale lock can't outlive a crash or reboot.
    pub fn open(data_dir: &Path) -> Result<(Arc<Host>, Recovered)> {
        std::fs::create_dir_all(data_dir).with_context(|| format!("could not create {}", data_dir.display()))?;
        let lock_file = std::fs::File::create(data_dir.join("host.lock"))?;
        if lock_file.try_lock().is_err() {
            bail!("another Curia host is already using {}", data_dir.display());
        }
        let store = Store::open(&data_dir.join("curia.sqlite"))?;
        let recovered = store.recover()?;
        let host = Host {
            store,
            data_dir: data_dir.to_path_buf(),
            claude_exe: find_claude()?,
            live: Mutex::new(HashMap::new()),
            events: broadcast::channel(4096).0,
            _lock: lock_file,
        };
        Ok((Arc::new(host), recovered))
    }

    pub fn subscribe(&self) -> broadcast::Receiver<HostEvent> {
        self.events.subscribe()
    }

    pub fn create_thread(&self, t: &NewThread) -> Result<ThreadRow> {
        if t.cli != "claude" {
            bail!("only Claude Code threads exist so far");
        }
        self.store.thread_create(t)
    }

    /// Sends a message, starting the thread's CLI first if it isn't running. A message that
    /// starts a new turn takes the turn's "before" checkpoint first.
    pub async fn send(self: &Arc<Self>, thread_id: &str, text: &str) -> Result<()> {
        let row = self.store.thread_get(thread_id)?;
        let live = self.live(&row).await?;
        if row.status != "running" {
            let before = snapshot(&row.cwd, "curia: before turn").await?;
            live.set_pending_before(before);
        }
        live.session.send(text).await
    }

    pub async fn answer(&self, thread_id: &str, request_id: &str, decision: &Decision) -> Result<()> {
        let live = lock(&self.live).get(thread_id).cloned();
        let live = live.context("this request can't be answered any more: the thread's CLI is no longer running")?;
        live.session.answer(request_id, decision).await
    }

    pub async fn interrupt(&self, thread_id: &str) -> Result<()> {
        let live = lock(&self.live).get(thread_id).cloned().context("the thread isn't running")?;
        live.session.interrupt().await
    }

    pub async fn set_effort(&self, thread_id: &str, level: &str) -> Result<()> {
        let live = lock(&self.live).get(thread_id).cloned().context("the thread isn't running")?;
        live.session.set_effort(level).await
    }

    /// What a turn changed: from its "before" checkpoint to its "after" one (or to now).
    pub async fn turn_diff(&self, thread_id: &str, turn: usize) -> Result<checkpoint::Diff> {
        let row = self.store.thread_get(thread_id)?;
        let t = self.store.turns(thread_id)?.into_iter().nth(turn).context("no such turn")?;
        let before = t.checkpoint_before.context("this turn has no checkpoint (is the folder a git repo?)")?;
        let cwd = PathBuf::from(row.cwd);
        tokio::task::spawn_blocking(move || checkpoint::diff(&cwd, &before, t.checkpoint_after.as_deref())).await?
    }

    /// Puts the thread's folder back to how it was before `turn`. Returns the safety
    /// checkpoint that undoes this.
    pub async fn restore_before(&self, thread_id: &str, turn: usize) -> Result<String> {
        let row = self.store.thread_get(thread_id)?;
        if row.status == "running" {
            bail!("stop the thread's turn before restoring files");
        }
        let t = self.store.turns(thread_id)?.into_iter().nth(turn).context("no such turn")?;
        let target = t.checkpoint_before.context("this turn has no checkpoint")?;
        let cwd = PathBuf::from(row.cwd);
        tokio::task::spawn_blocking(move || checkpoint::restore(&cwd, &target)).await?
    }

    /// Closes every running CLI (each exits on its own or is killed after 3 s).
    pub async fn close_all(&self) {
        let all: Vec<Arc<Live>> = lock(&self.live).values().cloned().collect();
        for live in all {
            live.session.close().await;
        }
    }

    pub fn is_live(&self, thread_id: &str) -> bool {
        lock(&self.live).contains_key(thread_id)
    }

    fn transcript_path(&self, thread_id: &str) -> Result<PathBuf> {
        let dir = self.data_dir.join("transcripts").join(thread_id);
        std::fs::create_dir_all(&dir)?;
        Ok(dir.join(format!("{}.jsonl", crate::store::now_ms())))
    }
}

/// A checkpoint off the async threads (git can take a moment).
async fn snapshot(cwd: &str, label: &'static str) -> Result<Option<String>> {
    let cwd = PathBuf::from(cwd);
    tokio::task::spawn_blocking(move || checkpoint::snapshot(&cwd, label)).await?
}
