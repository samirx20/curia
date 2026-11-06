//! Terminals (Tech Stack §2, Design 11): shells the host runs in a thread's folder, shared by
//! every client that opens them. The host keeps each screen; clients draw what it sends and
//! send keys back. A shell nobody has used for 15 minutes is closed (CARRY_OVER §21).
mod screen;
mod shell;

pub use screen::Size;
pub use shell::Shell;

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const IDLE_MS: i64 = 15 * 60 * 1000;

#[derive(Default)]
pub struct Terminals {
    shells: Mutex<HashMap<String, Arc<Shell>>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl Terminals {
    /// Starts a shell in `cwd` for a thread.
    pub async fn start(&self, thread_id: &str, cwd: PathBuf, size: Size) -> Result<Arc<Shell>> {
        let (id, thread) = (crate::store::new_id(), thread_id.to_string());
        let shell = tokio::task::spawn_blocking(move || Shell::start(id, thread, &cwd, size)).await??;
        lock(&self.shells).insert(shell.id.clone(), shell.clone());
        Ok(shell)
    }

    pub fn get(&self, id: &str) -> Result<Arc<Shell>> {
        lock(&self.shells).get(id).cloned().context("no such terminal (it may have been closed)")
    }

    /// A thread's terminals, oldest first.
    pub fn list(&self, thread_id: &str) -> Value {
        let mut shells: Vec<_> = lock(&self.shells).values().filter(|s| s.thread_id == thread_id).cloned().collect();
        shells.sort_by_key(|s| s.last_used.load(Ordering::Relaxed));
        json!(shells.iter().map(|s| json!({"id": s.id, "title": s.title, "exited": s.exited.load(Ordering::SeqCst)})).collect::<Vec<_>>())
    }

    /// The process ids of a thread's shells.
    pub fn pids(&self, thread_id: &str) -> Vec<u32> {
        lock(&self.shells).values().filter(|s| s.thread_id == thread_id).filter_map(|s| s.pid()).collect()
    }

    pub fn close(&self, id: &str) -> Result<()> {
        let shell = lock(&self.shells).remove(id).context("no such terminal")?;
        shell.close();
        Ok(())
    }

    /// Closes shells left idle, and forgets ones whose process has ended.
    pub fn start_reaper(self: &Arc<Self>) {
        let me = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                let now = crate::store::now_ms();
                let stale: Vec<String> = lock(&me.shells)
                    .values()
                    .filter(|s| s.exited.load(Ordering::SeqCst) || now - s.last_used.load(Ordering::Relaxed) > IDLE_MS)
                    .map(|s| s.id.clone())
                    .collect();
                for id in stale {
                    let _ = me.close(&id);
                }
            }
        });
    }
}

/// What a client gets for a terminal: its screen, and whether the shell has ended.
pub fn view(shell: &Shell) -> Value {
    json!({"id": shell.id, "title": shell.title, "screen": shell.screen(), "exited": shell.exited.load(Ordering::SeqCst)})
}
