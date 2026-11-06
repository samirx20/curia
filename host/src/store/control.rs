//! Control-room records: results a team owes its parent, queued messages, and merges.
use super::{Store, now_ms};
use anyhow::{Result, bail};
use curia_core::{Event, ItemId, TextKind};
use rusqlite::{OptionalExtension, params};
use std::collections::HashMap;

/// A team thread's result as the parent will receive it.
#[derive(Debug, Clone, PartialEq)]
pub struct Claimed {
    pub parent_id: String,
    /// `None`: the team never called report_result.
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MergeRow {
    pub id: String,
    pub thread_id: String,
    pub from_thread: String,
    pub cwd: String,
    pub branch: String,
    pub pre_head: String,
    pub merge_commit: String,
    pub checkpoint: String,
    pub state: String,
}

impl Store {
    /// A hand-off: the new team thread owes `parent_id` a result.
    pub fn result_expect(&self, thread_id: &str, parent_id: &str) -> Result<()> {
        self.conn().execute(
            "INSERT INTO results (thread_id, parent_id, state, created_at) VALUES (?1, ?2, 'awaiting', ?3)",
            params![thread_id, parent_id, now_ms()],
        )?;
        Ok(())
    }

    /// The team's own summary. Reporting again before delivery replaces it.
    pub fn result_report(&self, thread_id: &str, summary: &str) -> Result<()> {
        let n = self.conn().execute(
            "UPDATE results SET summary = ?2, state = 'reported' WHERE thread_id = ?1 AND state IN ('awaiting', 'reported')",
            params![thread_id, summary],
        )?;
        if n == 0 {
            bail!("this thread owes no result: only a thread started by hand_off reports one, and only once per task");
        }
        Ok(())
    }

    /// Claims the result for delivery, exactly once: a second claim finds nothing.
    pub fn result_claim(&self, thread_id: &str) -> Result<Option<Claimed>> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let found = tx
            .query_row(
                "SELECT parent_id, summary FROM results WHERE thread_id = ?1 AND state IN ('awaiting', 'reported')",
                [thread_id],
                |r| Ok(Claimed { parent_id: r.get(0)?, summary: r.get(1)? }),
            )
            .optional()?;
        if found.is_some() {
            tx.execute("UPDATE results SET state = 'delivered', delivered_at = ?2 WHERE thread_id = ?1", params![thread_id, now_ms()])?;
        }
        tx.commit()?;
        Ok(found)
    }

    /// Queues a message for a thread that is mid-turn.
    pub fn mailbox_push(&self, thread_id: &str, from: Option<&str>, text: &str) -> Result<()> {
        self.conn().execute(
            "INSERT INTO mailbox (thread_id, from_thread, text, state, created_at) VALUES (?1, ?2, ?3, 'queued', ?4)",
            params![thread_id, from, text, now_ms()],
        )?;
        Ok(())
    }

    /// Takes every queued message for a thread, oldest first.
    pub fn mailbox_take(&self, thread_id: &str) -> Result<Vec<(Option<String>, String)>> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let taken = {
            let mut stmt = tx.prepare("SELECT from_thread, text FROM mailbox WHERE thread_id = ?1 AND state = 'queued' ORDER BY id")?;
            stmt.query_map([thread_id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?
        };
        tx.execute("UPDATE mailbox SET state = 'delivered' WHERE thread_id = ?1 AND state = 'queued'", [thread_id])?;
        tx.commit()?;
        Ok(taken)
    }

    pub fn merge_add(&self, m: &MergeRow) -> Result<()> {
        self.conn().execute(
            "INSERT INTO merges (id, thread_id, from_thread, cwd, branch, pre_head, merge_commit, checkpoint, state, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![m.id, m.thread_id, m.from_thread, m.cwd, m.branch, m.pre_head, m.merge_commit, m.checkpoint, m.state, now_ms()],
        )?;
        Ok(())
    }

    pub fn merge_get(&self, id: &str) -> Result<Option<MergeRow>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT id, thread_id, from_thread, cwd, branch, pre_head, merge_commit, checkpoint, state FROM merges WHERE id = ?1",
                [id],
                |r| {
                    Ok(MergeRow {
                        id: r.get(0)?, thread_id: r.get(1)?, from_thread: r.get(2)?, cwd: r.get(3)?, branch: r.get(4)?,
                        pre_head: r.get(5)?, merge_commit: r.get(6)?, checkpoint: r.get(7)?, state: r.get(8)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn merge_set_state(&self, id: &str, state: &str) -> Result<()> {
        self.conn().execute("UPDATE merges SET state = ?2 WHERE id = ?1", params![id, state])?;
        Ok(())
    }

    /// The last finished text (not thinking) of a turn: a team's result when it never reported.
    pub fn last_text_of_turn(&self, turn_id: &str) -> Result<Option<String>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT body FROM events WHERE turn_id = ?1 ORDER BY seq")?;
        let bodies = stmt.query_map([turn_id], |r| r.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let mut kinds: HashMap<ItemId, TextKind> = HashMap::new();
        let mut last = None;
        for body in bodies {
            match serde_json::from_str::<Event>(&body)? {
                Event::TextStarted { item, kind, .. } => {
                    kinds.insert(item, kind);
                }
                Event::TextDone { item, text } if kinds.get(&item) != Some(&TextKind::Thinking) => last = Some(text),
                _ => {}
            }
        }
        Ok(last)
    }
}
