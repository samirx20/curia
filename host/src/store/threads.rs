//! Thread rows: which CLI and team a thread runs on, and where.
use super::{Store, new_id, now_ms};
use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, Row, params};

/// What's needed to start a thread.
#[derive(Debug, Clone)]
pub struct NewThread {
    pub title: String,
    pub cli: String,
    /// The team's config folder (`CLAUDE_CONFIG_DIR`, …).
    pub config_dir: String,
    pub cwd: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub permission_mode: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThreadRow {
    pub id: String,
    pub title: String,
    pub cli: String,
    pub config_dir: String,
    pub cwd: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub permission_mode: String,
    /// The CLI's own session id, used to resume.
    pub native_session_id: Option<String>,
    /// Item ids already handed out in this thread (the next process continues after them).
    pub items_used: u64,
    /// `idle` or `running`.
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

const COLUMNS: &str = "id, title, cli, config_dir, cwd, model, effort, permission_mode, native_session_id, items_used, status, created_at, updated_at";

fn row(r: &Row) -> rusqlite::Result<ThreadRow> {
    Ok(ThreadRow {
        id: r.get(0)?,
        title: r.get(1)?,
        cli: r.get(2)?,
        config_dir: r.get(3)?,
        cwd: r.get(4)?,
        model: r.get(5)?,
        effort: r.get(6)?,
        permission_mode: r.get(7)?,
        native_session_id: r.get(8)?,
        items_used: r.get::<_, i64>(9)? as u64,
        status: r.get(10)?,
        created_at: r.get(11)?,
        updated_at: r.get(12)?,
    })
}

impl Store {
    pub fn thread_create(&self, t: &NewThread) -> Result<ThreadRow> {
        let id = new_id();
        let now = now_ms();
        self.conn().execute(
            "INSERT INTO threads (id, title, cli, config_dir, cwd, model, effort, permission_mode, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            params![id, t.title, t.cli, t.config_dir, t.cwd, t.model, t.effort, t.permission_mode, now],
        )?;
        self.thread_get(&id)
    }

    pub fn thread_get(&self, id: &str) -> Result<ThreadRow> {
        let sql = format!("SELECT {COLUMNS} FROM threads WHERE id = ?1");
        self.conn().query_row(&sql, [id], row).optional()?.with_context(|| format!("no thread {id}"))
    }

    pub fn threads(&self) -> Result<Vec<ThreadRow>> {
        let sql = format!("SELECT {COLUMNS} FROM threads ORDER BY updated_at DESC");
        let conn = self.conn();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Item ids only grow, so a stale or repeated value never moves the count back.
    pub fn thread_set_items_used(&self, id: &str, items_used: u64) -> Result<()> {
        self.conn().execute(
            "UPDATE threads SET items_used = MAX(items_used, ?2) WHERE id = ?1",
            params![id, items_used as i64],
        )?;
        Ok(())
    }
}
