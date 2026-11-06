//! Reading a thread's event log in pages, for clients catching up or scrolling back.
use super::Store;
use super::record::Stored;
use anyhow::Result;
use rusqlite::params;

impl Store {
    /// Up to `limit` events after `after_seq`, oldest first.
    pub fn events_page(&self, thread_id: &str, after_seq: i64, limit: u32) -> Result<Vec<Stored>> {
        self.query_events(
            "SELECT seq, turn_id, body FROM events WHERE thread_id = ?1 AND seq > ?2 ORDER BY seq LIMIT ?3",
            params![thread_id, after_seq, limit],
            false,
        )
    }

    /// Up to `limit` events before `before_seq` (use `i64::MAX` for the newest), oldest first.
    pub fn events_before(&self, thread_id: &str, before_seq: i64, limit: u32) -> Result<Vec<Stored>> {
        self.query_events(
            "SELECT seq, turn_id, body FROM events WHERE thread_id = ?1 AND seq < ?2 ORDER BY seq DESC LIMIT ?3",
            params![thread_id, before_seq, limit],
            true,
        )
    }

    /// The highest event number the host has stored, across all threads (0 when empty).
    pub fn max_seq(&self) -> Result<i64> {
        Ok(self.conn().query_row("SELECT COALESCE(MAX(seq), 0) FROM events", [], |r| r.get(0))?)
    }

    fn query_events(&self, sql: &str, p: impl rusqlite::Params, newest_first: bool) -> Result<Vec<Stored>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(p, |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, String>(2)?)))?;
        let mut out = Vec::new();
        for row in rows {
            let (seq, turn_id, body) = row?;
            out.push(Stored { seq, turn_id, event: serde_json::from_str(&body)? });
        }
        if newest_first {
            out.reverse();
        }
        Ok(out)
    }
}
