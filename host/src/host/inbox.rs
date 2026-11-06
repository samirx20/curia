//! What happens to a request that needs the user: the always/never lists may answer it at
//! once; otherwise it waits in the inbox, and is denied after a while if the host is set to
//! (Scope §3.4). Also: continuing turns a restart cut off (CARRY_OVER §19).
use super::Host;
use super::quota::CONTINUE;
use crate::store::Recovered;
use anyhow::Result;
use curia_core::{Decision, Event};
use serde_json::Value;
use std::sync::Arc;

/// A turn is continued automatically after at most this many restarts in a row.
pub const MAX_AUTO_CONTINUES: u32 = 3;

impl Host {
    /// A CLI asked before acting. Called once per request, as it is stored.
    pub(super) fn request_arrived(self: &Arc<Self>, thread_id: &str, request_id: &str, tool: &str, input: &Value) {
        let settings = match self.store.settings() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("curia-host: could not read settings: {e:#}");
                return;
            }
        };
        if let Some(decision) = crate::rules::decide(tool, input, &settings.always_allow, &settings.never_allow) {
            let (me, t, r) = (self.clone(), thread_id.to_string(), request_id.to_string());
            let why = match &decision {
                Decision::Allow => format!("{tool} was allowed by the host's always-allow list."),
                _ => format!("{tool} was denied by the host's never-allow list."),
            };
            tokio::spawn(async move {
                let _ = me.record(&t, &[Event::Notice { text: why }]);
                if let Err(e) = me.answer(&t, &r, &decision).await {
                    let _ = me.record(&t, &[Event::Notice { text: format!("Could not apply the host's rule: {e:#}") }]);
                }
            });
            return;
        }
        self.deny_later(thread_id, request_id, settings.deny_unanswered_after_minutes);
    }

    /// Denies the request after `minutes` if nobody has answered it by then.
    pub(super) fn deny_later(self: &Arc<Self>, thread_id: &str, request_id: &str, minutes: Option<u32>) {
        let Some(minutes) = minutes else { return };
        let (me, t, r) = (self.clone(), thread_id.to_string(), request_id.to_string());
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(u64::from(minutes) * 60)).await;
            if me.store.request_pending(&t, &r).unwrap_or(false) {
                let message = Some(format!("No one answered within {minutes} minute(s), so it was denied."));
                if let Err(e) = me.answer(&t, &r, &Decision::Deny { message }).await {
                    let _ = me.record(&t, &[Event::Notice { text: format!("Could not deny the unanswered request: {e:#}") }]);
                }
            }
        });
    }

    /// After a restart: continue the turns it cut off, unless an approval or question was
    /// waiting (the user should see it first) or this has happened too often in a row.
    pub async fn continue_after_restart(self: &Arc<Self>, recovered: &Recovered) -> Result<()> {
        if !self.store.settings()?.continue_after_restart {
            return Ok(());
        }
        for thread in recovered.cut.iter().filter(|t| !recovered.waiting.contains(t)) {
            let n = self.store.thread_auto_continues(thread, false)?;
            if n > MAX_AUTO_CONTINUES {
                let text = format!("Not continued automatically: restarts cut this thread off {MAX_AUTO_CONTINUES} times in a row. Send a message to carry on.");
                self.record(thread, &[Event::Notice { text }])?;
                continue;
            }
            if let Err(e) = self.deliver(thread, None, CONTINUE).await {
                self.record(thread, &[Event::Notice { text: format!("Could not continue after the restart: {e:#}") }])?;
            }
        }
        Ok(())
    }
}
