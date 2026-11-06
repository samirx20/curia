//! The host: owns the data folder, the store and the live CLI processes. Every client
//! action, and every control-room tool an agent calls, is one of its operations.
mod ask;
mod control;
mod deliver;
mod files;
mod inbox;
mod live;
mod ops;
mod quota;
mod tree;
pub use tree::committed;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_quota;

use crate::store::{Recovered, Store};
use anyhow::{Context, Result, bail};
use curia_core::{Decision, Event};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::{broadcast, oneshot};

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
    live: Mutex<HashMap<String, Arc<Live>>>,
    events: broadcast::Sender<HostEvent>,
    /// The control room's MCP address, once it is serving.
    mcp_url: OnceLock<String>,
    /// Control-room token → the thread it was issued to. Valid only while that thread's CLI runs.
    tokens: Mutex<HashMap<String, String>>,
    /// `ask_human` questions waiting for an answer: request id → (thread, answer channel).
    asks: Mutex<HashMap<String, (String, oneshot::Sender<Decision>)>>,
    /// The host's own Web Push key (VAPID).
    pub push: crate::push::Keys,
    /// Shells running in threads' folders.
    pub terms: Arc<crate::term::Terminals>,
    /// Short-lived links to files' bytes (`file_link`).
    file_links: Mutex<HashMap<String, tree::Grant>>,
    _lock: std::fs::File,
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
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
        // No CLI runs yet, so every thread folder (agy's overlays) is left from a host that stopped.
        crate::links::remove_tree(&data_dir.join("homes")).context("could not clear the threads' old folders")?;
        let host = Host {
            store,
            data_dir: data_dir.to_path_buf(),
            live: Mutex::new(HashMap::new()),
            events: broadcast::channel(4096).0,
            mcp_url: OnceLock::new(),
            tokens: Mutex::new(HashMap::new()),
            asks: Mutex::new(HashMap::new()),
            push: crate::push::Keys::load_or_make(data_dir)?,
            terms: Arc::default(),
            file_links: Mutex::new(HashMap::new()),
            _lock: lock_file,
        };
        Ok((Arc::new(host), recovered))
    }

    pub fn subscribe(&self) -> broadcast::Receiver<HostEvent> {
        self.events.subscribe()
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn is_live(&self, thread_id: &str) -> bool {
        lock(&self.live).contains_key(thread_id)
    }

    /// Serves the control room's MCP tools from now on; threads started later get them.
    pub fn set_mcp_url(&self, url: String) {
        let _ = self.mcp_url.set(url);
    }

    /// The thread a control-room token belongs to, while that thread's CLI runs.
    pub fn thread_for_token(&self, token: &str) -> Option<String> {
        lock(&self.tokens).get(token).cloned()
    }

    /// Stores host-made events in a thread and tells subscribers.
    pub(crate) fn record(&self, thread_id: &str, events: &[Event]) -> Result<()> {
        for s in self.store.record(thread_id, events)? {
            let _ = self.events.send(HostEvent { thread_id: thread_id.to_string(), seq: Some(s.seq), event: s.event });
        }
        Ok(())
    }

    /// Stores and broadcasts events as if a CLI had produced them (tests of everything
    /// downstream of the adapters).
    #[cfg(test)]
    pub fn inject(&self, thread_id: &str, events: &[Event]) -> Result<()> {
        for e in events {
            if let Event::TextDelta { .. } = e {
                let _ = self.events.send(HostEvent { thread_id: thread_id.into(), seq: None, event: e.clone() });
                continue;
            }
            self.record(thread_id, std::slice::from_ref(e))?;
        }
        Ok(())
    }
}
