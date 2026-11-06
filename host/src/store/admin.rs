//! Queries for Settings, Search and the run summary: editing and removing teams, changing a
//! device's role, finding threads by what was said in them, and a run's totals.
use super::{Store, now_ms};
use anyhow::{Result, bail};
use rusqlite::params;
use serde_json::{Value, json};

impl Store {
    /// Renames a team or changes its default model and effort (whatever is given).
    pub fn team_update(&self, id: &str, name: Option<&str>, model: Option<&str>, effort: Option<&str>) -> Result<()> {
        let conn = self.conn();
        if let Some(n) = name.map(str::trim) {
            if n.is_empty() {
                bail!("a team needs a name");
            }
            conn.execute("UPDATE teams SET name = ?2 WHERE id = ?1", params![id, n])?;
        }
        if let Some(m) = model {
            conn.execute("UPDATE teams SET model = NULLIF(?2, '') WHERE id = ?1", params![id, m])?;
        }
        if let Some(e) = effort {
            conn.execute("UPDATE teams SET effort = NULLIF(?2, '') WHERE id = ?1", params![id, e])?;
        }
        Ok(())
    }

    /// Removes a team no thread uses (a thread stays on its team for life).
    pub fn team_remove(&self, id: &str) -> Result<()> {
        let conn = self.conn();
        let used: i64 = conn.query_row("SELECT COUNT(*) FROM threads WHERE team_id = ?1", [id], |r| r.get(0))?;
        if used > 0 {
            bail!("{used} thread(s) belong to this team, so it can't be removed");
        }
        conn.execute("DELETE FROM team_usage WHERE team_id = ?1", [id])?;
        if conn.execute("DELETE FROM teams WHERE id = ?1", [id])? == 0 {
            bail!("no such team");
        }
        Ok(())
    }

    pub fn device_set_role(&self, id: &str, role: &str) -> Result<()> {
        if !["owner", "approve", "view"].contains(&role) {
            bail!("unknown role {role:?}");
        }
        if self.conn().execute("UPDATE devices SET role = ?2 WHERE id = ?1", params![id, role])? == 0 {
            bail!("no such device");
        }
        Ok(())
    }

    /// Threads whose title, or something said in them, contains `query` (newest first).
    pub fn search(&self, query: &str, limit: u32) -> Result<Vec<Value>> {
        let q = query.trim();
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let like = format!("%{}%", q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
        let conn = self.conn();
        let mut out = Vec::new();
        let mut stmt = conn.prepare("SELECT id, title FROM threads WHERE title LIKE ?1 ESCAPE '\\' ORDER BY updated_at DESC LIMIT ?2")?;
        for r in stmt.query_map(params![like, limit], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (id, title) = r?;
            out.push(json!({"thread_id": id, "title": title, "seq": null, "snippet": null}));
        }
        let mut stmt = conn.prepare(
            "SELECT e.thread_id, t.title, e.seq, e.body FROM events e JOIN threads t ON t.id = e.thread_id
             WHERE e.body LIKE ?1 ESCAPE '\\'
               AND json_extract(e.body, '$.type') IN ('text_done', 'turn_started', 'hand_back', 'message')
             ORDER BY e.seq DESC LIMIT ?2",
        )?;
        for r in stmt.query_map(params![like, limit], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, String>(3)?)))? {
            let (id, title, seq, body) = r?;
            let v: Value = serde_json::from_str(&body).unwrap_or_default();
            let text = v["text"].as_str().or(v["prompt"].as_str()).or(v["summary"].as_str()).unwrap_or("");
            if let Some(snippet) = around(text, q) {
                out.push(json!({"thread_id": id, "title": title, "seq": seq, "snippet": snippet}));
            }
        }
        out.truncate(limit as usize);
        Ok(out)
    }

    /// A run's totals: its threads, hand-offs, merges, approvals, cost and time.
    pub fn run_summary(&self, root_id: &str) -> Result<Value> {
        let conn = self.conn();
        let one = |sql: &str| conn.query_row(sql, [root_id], |r| r.get::<_, Option<f64>>(0)).unwrap_or(None);
        let in_run = "SELECT id FROM threads WHERE COALESCE(root_id, id) = ?1";
        let cost = one(&format!("SELECT SUM(json_extract(body, '$.cost_usd')) FROM events WHERE thread_id IN ({in_run}) AND json_extract(body, '$.type') = 'turn_finished'"));
        let first = one(&format!("SELECT MIN(at_ms) FROM events WHERE thread_id IN ({in_run})"));
        let last = one(&format!("SELECT MAX(at_ms) FROM events WHERE thread_id IN ({in_run})"));
        let count = |sql: String| conn.query_row(&sql, [root_id], |r| r.get::<_, i64>(0)).unwrap_or(0);
        let hand_offs = count(format!("SELECT COUNT(*) FROM events WHERE thread_id IN ({in_run}) AND json_extract(body, '$.type') = 'handed_off'"));
        let results = count(format!("SELECT COUNT(*) FROM events WHERE thread_id IN ({in_run}) AND json_extract(body, '$.type') = 'hand_back'"));
        let merged = count(format!("SELECT COUNT(*) FROM merges WHERE thread_id IN ({in_run}) AND state = 'merged'"));
        let undone = count(format!("SELECT COUNT(*) FROM merges WHERE thread_id IN ({in_run}) AND state = 'undone'"));
        let approvals = count(format!("SELECT COUNT(*) FROM requests WHERE thread_id IN ({in_run})"));
        let waiting = count(format!("SELECT COUNT(*) FROM requests WHERE thread_id IN ({in_run}) AND status = 'pending'"));
        let mut stmt = conn.prepare(&format!("SELECT t.title, t.status, tm.name, tm.cli FROM threads t LEFT JOIN teams tm ON tm.id = t.team_id WHERE t.id IN ({in_run}) ORDER BY t.created_at"))?;
        let threads: Vec<Value> = stmt
            .query_map([root_id], |r| Ok(json!({"title": r.get::<_, String>(0)?, "status": r.get::<_, String>(1)?, "team": r.get::<_, Option<String>>(2)?, "cli": r.get::<_, Option<String>>(3)?})))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(json!({
            "threads": threads, "hand_offs": hand_offs, "results": results, "merged": merged, "undone": undone,
            "approvals": approvals, "waiting": waiting, "cost_usd": cost,
            "started_ms": first, "last_ms": last, "now_ms": now_ms(),
        }))
    }
}

/// A short piece of `text` around the first place `q` appears (ignoring case).
fn around(text: &str, q: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let at = lower.find(&q.to_lowercase())?;
    let start = lower[..at].char_indices().rev().nth(40).map_or(0, |(i, _)| i);
    let end = text[at..].char_indices().nth(q.chars().count() + 60).map_or(text.len(), |(i, _)| at + i);
    let mut s = text.get(start..end).unwrap_or(text).replace('\n', " ");
    if start > 0 {
        s.insert(0, '…');
    }
    if end < text.len() {
        s.push('…');
    }
    Some(s)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_snippet_shows_the_match_with_context() {
        let s = super::around("The harness is in and green.\nNext the sampler.", "SAMPLER").unwrap();
        assert!(s.contains("sampler") && !s.contains('\n'));
        assert!(super::around("nothing here", "missing").is_none());
    }
}
