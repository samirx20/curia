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

impl Host {
    /// Delivers `text` to a thread from another thread (`from`) or from the user (`None`).
    /// Returns how: `started` a turn, `steered` into the running one, or `queued`.
    pub async fn deliver(self: &Arc<Self>, to: &str, from: Option<&str>, text: &str) -> Result<&'static str> {
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
        let row = self.store.thread_get(to)?;
        let live = self.live_of(to);
        let how = match live {
            Some(l) if row.status == "running" && l.can_steer() => {
                l.session.send(framed).await?;
                "steered"
            }
            _ if row.status == "running" => {
                self.store.mailbox_push(to, from, framed)?;
                "queued"
            }
            _ => {
                self.start_turn(to, framed).await?;
                "started"
            }
        };
        // The turn's prompt already shows a user message that started it.
        if from.is_some() || how != "started" {
            self.record(to, &[Event::Message { from: from.map(str::to_string), text: text.to_string(), how: how.into() }])?;
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
        self.start_turn(thread_id, &text).await
    }
}
