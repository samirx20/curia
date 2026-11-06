//! The host's state in one SQLite file. The `events` table is the source of truth; `turns`,
//! `requests`, `subagents` and the thread's status are kept in step with it in the same
//! transaction (`record`). Settings follow what Agent Hub learned (CARRY_OVER §8).
mod control;
mod pages;
mod record;
mod recover;
mod schema;
mod teams;
mod threads;

pub use control::MergeRow;
pub use record::Stored;
pub use recover::Recovered;

pub use threads::{NewThread, ThreadRow};

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

pub struct Store(Mutex<Connection>);

pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

impl Store {
    pub fn open(path: &Path) -> Result<Store> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("could not create {}", dir.display()))?;
        }
        let conn = Connection::open(path).with_context(|| format!("could not open {}", path.display()))?;
        Store::init(conn)
    }

    #[cfg(test)]
    pub fn in_memory() -> Store {
        Store::init(Connection::open_in_memory().unwrap()).unwrap()
    }

    fn init(conn: Connection) -> Result<Store> {
        // auto_vacuum must be chosen before the first table exists. NORMAL, not FULL: every
        // event is a commit, and FULL stalled Agent Hub's server.
        conn.execute_batch(
            "PRAGMA auto_vacuum = INCREMENTAL;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 5000;
             PRAGMA journal_size_limit = 33554432;
             PRAGMA foreign_keys = ON;",
        )?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        for (i, sql) in schema::MIGRATIONS.iter().enumerate().skip(version as usize) {
            conn.execute_batch(&format!("BEGIN; {sql}; PRAGMA user_version = {}; COMMIT;", i + 1))
                .with_context(|| format!("database upgrade {} failed", i + 1))?;
        }
        Ok(Store(Mutex::new(conn)))
    }

    fn conn(&self) -> MutexGuard<'_, Connection> {
        self.0.lock().unwrap_or_else(|p| p.into_inner())
    }
}

#[cfg(test)]
mod tests;
