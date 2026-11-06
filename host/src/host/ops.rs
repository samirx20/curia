//! Client actions on a thread.
use super::{Host, Live, lock};
use crate::git::checkpoint;
use crate::store::{NewThread, ThreadRow};
use anyhow::{Context, Result, bail};
use curia_core::Decision;
use std::path::PathBuf;
use std::sync::Arc;

impl Host {
    pub fn create_thread(&self, t: &NewThread) -> Result<ThreadRow> {
        if t.cli != "claude" {
            bail!("only Claude Code threads exist so far");
        }
        self.store.thread_create(t)
    }

    /// Writes a message as a new turn, starting the thread's CLI first if needed. The turn's
    /// "before" checkpoint is taken first. Callers outside the host use `deliver`, which
    /// decides between starting, steering and queueing.
    /// `by`: who sent it when not the user (another thread's id, or `curia`).
    pub(crate) async fn start_turn(self: &Arc<Self>, thread_id: &str, text: &str, by: Option<&str>) -> Result<()> {
        let row = self.store.thread_get(thread_id)?;
        let live = self.live(&row).await?;
        if row.status != "running" {
            let before = snapshot(&row.cwd, "curia: before turn").await?;
            live.set_pending_before(before);
            live.set_pending_by(by.map(str::to_string));
        }
        live.session.send(text).await
    }

    /// Answers a waiting approval or question: a CLI's own, or an `ask_human` question.
    pub async fn answer(&self, thread_id: &str, request_id: &str, decision: &Decision) -> Result<()> {
        if self.answer_ask(thread_id, request_id, decision)? {
            return Ok(());
        }
        let live = self.live_of(thread_id).context("this request can't be answered any more: the thread's CLI is no longer running")?;
        live.session.answer(request_id, decision).await
    }

    /// Interrupts a thread's turn, and the turns of every team thread under it, deepest first.
    pub async fn interrupt(&self, thread_id: &str) -> Result<()> {
        let root = self.store.thread_get(thread_id)?;
        let mut below: Vec<ThreadRow> = self.store.run_threads(&root.root_id)?
            .into_iter()
            .filter(|t| t.id != thread_id && descends_from(&self.store, t, thread_id))
            .collect();
        below.sort_by_key(|t| std::cmp::Reverse(t.depth));
        for t in below.iter().filter(|t| t.status == "running") {
            if let Some(live) = self.live_of(&t.id) {
                live.session.interrupt().await?;
            }
        }
        let live = self.live_of(thread_id).context("the thread isn't running")?;
        live.session.interrupt().await
    }

    pub async fn set_effort(&self, thread_id: &str, level: &str) -> Result<()> {
        let live = self.live_of(thread_id).context("the thread isn't running")?;
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

    pub(crate) fn live_of(&self, thread_id: &str) -> Option<Arc<Live>> {
        lock(&self.live).get(thread_id).cloned()
    }
}

/// Whether `t` sits under `ancestor` in the run's tree.
fn descends_from(store: &crate::store::Store, t: &ThreadRow, ancestor: &str) -> bool {
    let mut parent = t.parent_id.clone();
    while let Some(p) = parent {
        if p == ancestor {
            return true;
        }
        parent = store.thread_get(&p).ok().and_then(|r| r.parent_id);
    }
    false
}

/// A checkpoint off the async threads (git can take a moment).
pub(crate) async fn snapshot(cwd: &str, label: &'static str) -> Result<Option<String>> {
    let cwd = PathBuf::from(cwd);
    tokio::task::spawn_blocking(move || checkpoint::snapshot(&cwd, label)).await?
}
