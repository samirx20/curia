//! Getting a message to a thread. The rule (Scope §8): steer it into the running turn when
//! the CLI can take it there safely, otherwise queue it until the turn ends; never interrupt.
//! Claude can't steer in stream-json mode (see `claude::STEERS_MID_TURN`), so a busy Claude
//! thread's messages wait in the host's mailbox, which survives a restart and batches them.
use super::Host;
use anyhow::Result;
use curia_core::{Event, TurnOutcome};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// The `by` of a turn the host itself started.
pub const CURIA: &str = "curia";

impl Host {
    /// Delivers `text` to a thread from another thread (`from`) or from the user (`None`).
    /// Returns how: `started` a turn, `steered` into the running one, or `queued`.
    pub async fn deliver(self: &Arc<Self>, to: &str, from: Option<&str>, text: &str) -> Result<&'static str> {
        if from.is_none() && text != super::quota::CONTINUE {
            self.store.thread_auto_continues(to, true)?; // a person carried on: the count starts again
        }
        let framed = match from {
            Some(id) => {
                let sender = self.store.thread_get(id).map(|t| t.title).unwrap_or_else(|_| id.to_string());
                format!("Message from thread \"{sender}\" ({id}):\n{text}")
            }
            None => text.to_string(),
        };
        self.deliver_as_written(to, from, text, &framed).await
    }

    /// Delivers `framed` exactly as written (it already says who it is from); `text` is what
    /// the thread's history shows.
    pub(super) async fn deliver_as_written(self: &Arc<Self>, to: &str, from: Option<&str>, text: &str, framed: &str) -> Result<&'static str> {
        self.deliver_by(to, from, text, framed, from).await
    }

    /// Curia's own message to a thread (a continue): its turn shows it came from Curia.
    pub(super) async fn deliver_from_curia(self: &Arc<Self>, to: &str, text: &str) -> Result<&'static str> {
        self.deliver_by(to, None, text, text, Some(CURIA)).await
    }

    async fn deliver_by(self: &Arc<Self>, to: &str, from: Option<&str>, text: &str, framed: &str, by: Option<&str>) -> Result<&'static str> {
        let row = self.store.thread_get(to)?;
        let live = self.live_of(to);
        let steer = live.as_ref().is_some_and(|l| row.status == "running" && l.can_steer());
        let how = if steer { "steered" } else if row.status == "running" { "queued" } else { "started" };
        // Recorded before acting, so a message from another thread always comes before the turn
        // it starts. (A user message that starts a turn is shown by the turn's prompt.)
        if from.is_some() || how != "started" {
            self.record(to, &[Event::Message { from: from.map(str::to_string), text: text.to_string(), how: how.into() }])?;
        }
        let acted = match (how, live) {
            ("steered", Some(l)) => l.session.send(framed).await,
            ("queued", _) => self.store.mailbox_push(to, from, framed),
            _ => self.start_turn(to, framed, by).await,
        };
        if let Err(e) = acted {
            self.record(to, &[Event::Notice { text: format!("A message could not be delivered: {e:#}") }])?;
            return Err(e);
        }
        // The turn may have ended while this was being queued: don't strand the message.
        if how == "queued" && self.store.thread_get(to)?.status != "running" {
            self.flush_mailbox(to).await?;
        }
        Ok(how)
    }

    /// After a turn: a team thread hands back its result, then queued messages are delivered
    /// together as the next turn. Failures are reported in the thread, not swallowed.
    /// Boxed: delivering can start a thread, whose event task calls this again.
    pub(super) fn after_turn(self: Arc<Self>, thread_id: String, turn_id: String, outcome: TurnOutcome) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async move {
            if matches!(outcome, TurnOutcome::UsageLimited { .. }) {
                if let Err(e) = self.out_of_quota(&thread_id).await {
                    let _ = self.record(&thread_id, &[Event::Notice { text: format!("Curia could not handle the quota limit: {e:#}") }]);
                }
            }
            if let Err(e) = self.hand_back(&thread_id, &turn_id, &outcome).await {
                let text = format!("Curia could not hand this thread's result back: {e:#}");
                let _ = self.record(&thread_id, &[Event::Notice { text }]);
            }
            if let Err(e) = self.flush_mailbox(&thread_id).await {
                let text = format!("Curia could not deliver queued messages: {e:#}");
                let _ = self.record(&thread_id, &[Event::Notice { text }]);
            }
        })
    }

    /// Sends every queued message as one new turn (several results landing during one turn
    /// arrive together, as in Agent Hub).
    pub(super) async fn flush_mailbox(self: &Arc<Self>, thread_id: &str) -> Result<()> {
        let msgs = self.store.mailbox_take(thread_id)?;
        if msgs.is_empty() {
            return Ok(());
        }
        let text = if msgs.len() == 1 {
            msgs[0].1.clone()
        } else {
            let parts: Vec<String> = msgs.into_iter().map(|(_, t)| t).collect();
            format!("{} messages arrived during your last turn:\n\n{}", parts.len(), parts.join("\n\n---\n\n"))
        };
        self.start_turn(thread_id, &text, Some(CURIA)).await
    }
}
