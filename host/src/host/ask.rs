//! `ask_human`: an agent's question to the user, shown and answered like any CLI question.
use super::{Host, lock};
use crate::store::new_id;
use anyhow::{Context, Result};
use curia_core::{Decision, Event, RequestKind, Resolution};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::oneshot;

impl Host {
    /// Records the question in the caller's thread and waits for the user's answer.
    pub async fn ask_human(self: &Arc<Self>, caller: &str, question: &str, options: &[String]) -> Result<Value> {
        let request_id = new_id();
        let (tx, rx) = oneshot::channel();
        lock(&self.asks).insert(request_id.clone(), (caller.to_string(), tx));
        let input = json!({"questions": [{"question": question, "options": options.iter().map(|o| json!({"label": o})).collect::<Vec<_>>()}]});
        let asked = self.record(caller, &[Event::ApprovalRequested {
            request_id: request_id.clone(), kind: RequestKind::Question, tool_name: "ask_human".into(), input,
        }]);
        if let Err(e) = asked {
            lock(&self.asks).remove(&request_id);
            return Err(e);
        }
        self.deny_later(caller, &request_id, self.store.settings()?.deny_unanswered_after_minutes);
        let decision = rx.await.context("the question was dropped before anyone answered (the host is stopping)")?;
        Ok(match decision {
            Decision::Answer { answers } => json!({"answer": answers.get(question).cloned().unwrap_or(Value::Object(answers))}),
            Decision::Allow => json!({"answer": "yes"}),
            Decision::Deny { message } => json!({"declined": true, "message": message}),
        })
    }

    /// Answers an `ask_human` question, if `request_id` is one. Returns false otherwise.
    pub(super) fn answer_ask(&self, thread_id: &str, request_id: &str, decision: &Decision) -> Result<bool> {
        let waiting = {
            let mut asks = lock(&self.asks);
            match asks.get(request_id) {
                Some((t, _)) if t == thread_id => asks.remove(request_id),
                _ => None,
            }
        };
        let Some((_, tx)) = waiting else { return Ok(false) };
        let resolution = match decision {
            Decision::Answer { .. } => Resolution::Answered,
            Decision::Allow => Resolution::Allowed,
            Decision::Deny { .. } => Resolution::Denied,
        };
        self.record(thread_id, &[Event::ApprovalResolved { request_id: request_id.to_string(), resolution }])?;
        let _ = tx.send(decision.clone());
        Ok(true)
    }
}
