//! Thread rows: which CLI and team a thread runs on, where, and where it sits in its run.
use super::{Store, new_id, now_ms};
use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, Row, params};

/// What's needed to start a thread.
#[derive(Debug, Clone, Default)]
pub struct NewThread {
    pub title: String,
    pub cli: String,
    /// The team's config folder (`CLAUDE_CONFIG_DIR`, …).
    pub config_dir: String,
    pub cwd: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub permission_mode: String,
    pub team_id: Option<String>,
    /// The thread that handed this one its task (none for a run's main thread).
    pub parent_id: Option<String>,
    pub depth: u32,
    /// The team thread's own branch, when it works in a worktree.
    pub branch: Option<String>,
    /// Lets a retried hand-off find the thread the first call made.
    pub client_request_id: Option<String>,
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
    pub team_id: Option<String>,
    pub parent_id: Option<String>,
    /// The run's main thread (itself for a main thread).
    pub root_id: String,
    pub depth: u32,
    pub branch: Option<String>,
}

const COLUMNS: &str = "id, title, cli, config_dir, cwd, model, effort, permission_mode, native_session_id, items_used, status, created_at, updated_at, team_id, parent_id, COALESCE(root_id, id), depth, branch";

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
        team_id: r.get(13)?,
        parent_id: r.get(14)?,
        root_id: r.get(15)?,
        depth: r.get::<_, i64>(16)? as u32,
        branch: r.get(17)?,
    })
}

impl Store {
    pub fn thread_create(&self, t: &NewThread) -> Result<ThreadRow> {
        let id = new_id();
        let now = now_ms();
        let root = match &t.parent_id {
            Some(p) => self.thread_get(p)?.root_id,
            None => id.clone(),
        };
        self.conn().execute(
            "INSERT INTO threads (id, title, cli, config_dir, cwd, model, effort, permission_mode, created_at, updated_at,
                                  team_id, parent_id, root_id, depth, branch, client_request_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![id, t.title, t.cli, t.config_dir, t.cwd, t.model, t.effort, t.permission_mode, now,
                t.team_id, t.parent_id, root, t.depth, t.branch, t.client_request_id],
        )?;
        self.thread_get(&id)
    }

    pub fn thread_get(&self, id: &str) -> Result<ThreadRow> {
        let sql = format!("SELECT {COLUMNS} FROM threads WHERE id = ?1");
        self.conn().query_row(&sql, [id], row).optional()?.with_context(|| format!("no thread {id}"))
    }

    pub fn threads(&self) -> Result<Vec<ThreadRow>> {
        self.threads_where("1 = 1 ORDER BY updated_at DESC", [])
    }

    /// The thread a hand-off with this request id already made, if any.
    pub fn thread_by_request(&self, parent_id: &str, client_request_id: &str) -> Result<Option<ThreadRow>> {
        Ok(self.threads_where("parent_id = ?1 AND client_request_id = ?2", [parent_id, client_request_id])?.pop())
    }

    /// Every thread in a run (the main thread and all team threads under it).
    pub fn run_threads(&self, root_id: &str) -> Result<Vec<ThreadRow>> {
        self.threads_where("COALESCE(root_id, id) = ?1 ORDER BY depth, created_at", [root_id])
    }

    fn threads_where<P: rusqlite::Params>(&self, filter: &str, p: P) -> Result<Vec<ThreadRow>> {
        let sql = format!("SELECT {COLUMNS} FROM threads WHERE {filter}");
        let conn = self.conn();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(p, row)?.collect::<rusqlite::Result<Vec<_>>>()?;
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
