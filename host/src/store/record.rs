//! Appending a thread's events, and keeping turns, requests and subagents in step.
use super::{Store, new_id, now_ms};
use anyhow::Result;
use curia_core::{Event, TurnOutcome};
use rusqlite::{OptionalExtension, Transaction, params};

/// An event as stored: its place in the global order and the turn it belongs to.
#[derive(Debug, Clone, PartialEq)]
pub struct Stored {
    pub seq: i64,
    pub turn_id: Option<String>,
    pub event: Event,
}

/// A turn as the tables keep it.
#[derive(Debug, Clone, PartialEq)]
pub struct TurnRow {
    pub id: String,
    pub prompt: Option<String>,
    pub status: String,
    pub checkpoint_before: Option<String>,
    pub checkpoint_after: Option<String>,
}

impl Store {
    /// Appends events in one transaction. Text deltas are live-only and not stored: the
    /// finished text arrives as `TextDone`.
    pub fn record(&self, thread_id: &str, events: &[Event]) -> Result<Vec<Stored>> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let now = now_ms();
        let mut turn = running_turn(&tx, thread_id)?;
        let mut stored = Vec::new();
        for original in events.iter().filter(|e| !matches!(e, Event::TextDelta { .. })) {
            // Secrets never reach the database or a client.
            let mut redacted = original.clone();
            crate::redact::event(&mut redacted);
            let event = &redacted;
            if let Event::TurnStarted { prompt, .. } = event {
                let id = new_id();
                tx.execute(
                    "INSERT INTO turns (id, thread_id, prompt, status, started_at) VALUES (?1, ?2, ?3, 'running', ?4)",
                    params![id, thread_id, prompt, now],
                )?;
                tx.execute("UPDATE threads SET status = 'running' WHERE id = ?1", [thread_id])?;
                turn = Some(id);
            }
            tx.execute(
                "INSERT INTO events (thread_id, turn_id, at_ms, body) VALUES (?1, ?2, ?3, ?4)",
                params![thread_id, turn, now, serde_json::to_string(event)?],
            )?;
            let seq = tx.last_insert_rowid();
            apply(&tx, thread_id, turn.as_deref(), event, now)?;
            stored.push(Stored { seq, turn_id: turn.clone(), event: event.clone() });
            if matches!(event, Event::TurnFinished { .. }) {
                turn = None;
            }
        }
        tx.execute("UPDATE threads SET updated_at = ?2 WHERE id = ?1", params![thread_id, now])?;
        tx.commit()?;
        Ok(stored)
    }

    /// A thread's stored events after `after_seq`, oldest first.
    pub fn events_after(&self, thread_id: &str, after_seq: i64) -> Result<Vec<Stored>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT seq, turn_id, body FROM events WHERE thread_id = ?1 AND seq > ?2 ORDER BY seq")?;
        let rows = stmt.query_map(params![thread_id, after_seq], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, String>(2)?)))?;
        let mut out = Vec::new();
        for row in rows {
            let (seq, turn_id, body) = row?;
            out.push(Stored { seq, turn_id, event: serde_json::from_str(&body)? });
        }
        Ok(out)
    }

    pub fn turns(&self, thread_id: &str) -> Result<Vec<TurnRow>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, prompt, status, checkpoint_before, checkpoint_after FROM turns WHERE thread_id = ?1 ORDER BY started_at, rowid",
        )?;
        let rows = stmt.query_map([thread_id], |r| {
            Ok(TurnRow { id: r.get(0)?, prompt: r.get(1)?, status: r.get(2)?, checkpoint_before: r.get(3)?, checkpoint_after: r.get(4)? })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Records a turn's checkpoint (`before` or `after`).
    pub fn turn_set_checkpoint(&self, turn_id: &str, before: bool, commit: &str) -> Result<()> {
        let sql = if before {
            "UPDATE turns SET checkpoint_before = ?2 WHERE id = ?1"
        } else {
            "UPDATE turns SET checkpoint_after = ?2 WHERE id = ?1"
        };
        self.conn().execute(sql, params![turn_id, commit])?;
        Ok(())
    }
}

fn running_turn(tx: &Transaction, thread_id: &str) -> Result<Option<String>> {
    let sql = "SELECT id FROM turns WHERE thread_id = ?1 AND status = 'running' ORDER BY started_at DESC, rowid DESC LIMIT 1";
    Ok(tx.query_row(sql, [thread_id], |r| r.get(0)).optional()?)
}

/// Keeps the tables in step with one event.
fn apply(tx: &Transaction, thread: &str, turn: Option<&str>, e: &Event, now: i64) -> Result<()> {
    match e {
        Event::SessionStarted { native_session_id, .. } => {
            tx.execute("UPDATE threads SET native_session_id = ?2 WHERE id = ?1", params![thread, native_session_id])?;
        }
        Event::ApprovalRequested { request_id, kind, tool_name, input } => {
            tx.execute(
                "INSERT OR REPLACE INTO requests (thread_id, request_id, turn_id, kind, tool_name, input, status, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', ?7)",
                params![thread, request_id, turn, name(kind)?, tool_name, input.to_string(), now],
            )?;
        }
        Event::ApprovalResolved { request_id, resolution } => {
            tx.execute(
                "UPDATE requests SET status = ?3, resolved_at = ?4 WHERE thread_id = ?1 AND request_id = ?2",
                params![thread, request_id, name(resolution)?, now],
            )?;
        }
        Event::SubagentStarted { task_id, tool_use_id, description } => {
            tx.execute(
                "INSERT OR REPLACE INTO subagents (thread_id, task_id, tool_use_id, description, status, started_at)
                 VALUES (?1, ?2, ?3, ?4, 'running', ?5)",
                params![thread, task_id, tool_use_id, description, now],
            )?;
        }
        Event::SubagentEnded { task_id, status } => {
            tx.execute(
                "UPDATE subagents SET status = ?3, ended_at = ?4 WHERE thread_id = ?1 AND task_id = ?2",
                params![thread, task_id, status, now],
            )?;
        }
        Event::TurnFinished { outcome, cost_usd } => {
            let error = match outcome {
                TurnOutcome::Failed { message } => Some(message.as_str()),
                _ => None,
            };
            let status = serde_json::to_value(outcome)?["outcome"].as_str().unwrap_or("failed").to_string();
            tx.execute(
                "UPDATE turns SET status = ?2, ended_at = ?3, cost_usd = ?4, error = ?5 WHERE id = ?1",
                params![turn, status, now, cost_usd, error],
            )?;
            tx.execute("UPDATE threads SET status = 'idle' WHERE id = ?1", [thread])?;
        }
        _ => {}
    }
    Ok(())
}

/// The snake_case name serde gives a unit enum value.
fn name<T: serde::Serialize>(v: &T) -> Result<String> {
    Ok(serde_json::to_value(v)?.as_str().unwrap_or_default().to_string())
}
