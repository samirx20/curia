//! A thread whose CLI is running: its session, and the task that stores its events.
use super::{Host, HostEvent, lock, snapshot};
use crate::claude::{ClaudeSession, Launch};
use crate::record::Recorder;
use crate::store::ThreadRow;
use anyhow::Result;
use curia_core::Event;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

pub struct Live {
    pub session: Arc<ClaudeSession>,
    /// The checkpoint taken just before a message that starts a turn; the turn claims it.
    pending_before: Mutex<Option<String>>,
}

impl Live {
    pub fn set_pending_before(&self, commit: Option<String>) {
        *lock(&self.pending_before) = commit;
    }
}

impl Host {
    /// The thread's running CLI, started (resuming its session) if needed.
    pub(super) async fn live(self: &Arc<Self>, row: &ThreadRow) -> Result<Arc<Live>> {
        if let Some(live) = lock(&self.live).get(&row.id).cloned() {
            return Ok(live);
        }
        let launch = Launch {
            config_dir: row.config_dir.clone().into(),
            cwd: row.cwd.clone().into(),
            model: row.model.clone(),
            effort: row.effort.clone(),
            permission_mode: row.permission_mode.clone(),
            resume: row.native_session_id.clone(),
        };
        let recorder = Recorder::create(&self.transcript_path(&row.id)?)?;
        let (session, rx) = ClaudeSession::start(&self.claude_exe()?, &launch, row.items_used, Some(recorder)).await?;
        let live = Arc::new(Live { session, pending_before: Mutex::new(None) });
        lock(&self.live).insert(row.id.clone(), live.clone());
        tokio::spawn(self.clone().pump(row.id.clone(), row.cwd.clone(), live.clone(), rx));
        Ok(live)
    }

    /// Stores each event the CLI produces, takes turn checkpoints, and tells subscribers.
    async fn pump(self: Arc<Self>, thread_id: String, cwd: String, live: Arc<Live>, mut rx: mpsc::UnboundedReceiver<Event>) {
        while let Some(event) = rx.recv().await {
            if let Event::TextDelta { .. } = event {
                let _ = self.events.send(HostEvent { thread_id: thread_id.clone(), seq: None, event });
                continue;
            }
            let exited = matches!(event, Event::Exited { .. });
            if let Err(e) = self.store_one(&thread_id, &cwd, &live, event).await {
                // Fail loudly: the client sees it, and so does the log.
                eprintln!("curia-host: could not store an event for thread {thread_id}: {e:#}");
                let text = format!("Curia could not save part of this thread: {e:#}");
                let _ = self.events.send(HostEvent { thread_id: thread_id.clone(), seq: None, event: Event::Notice { text } });
            }
            if exited {
                break;
            }
        }
        lock(&self.live).remove(&thread_id);
    }

    async fn store_one(&self, thread_id: &str, cwd: &str, live: &Live, event: Event) -> Result<()> {
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
                (Event::TurnFinished { .. }, Some(turn)) => {
                    if let Some(c) = snapshot(cwd, "curia: after turn").await? {
                        self.store.turn_set_checkpoint(turn, false, &c)?;
                    }
                }
                _ => {}
            }
            let _ = self.events.send(HostEvent { thread_id: thread_id.to_string(), seq: Some(s.seq), event: s.event });
        }
        Ok(())
    }
}
