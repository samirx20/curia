//! Host settings (Scope §5): saved on the host, the same for every device.
use super::Store;
use anyhow::Result;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// When a team runs out mid-turn, move the thread to another team of the same CLI that
    /// can resume its session. Otherwise the thread waits for the reset.
    pub reroute_when_out: bool,
    /// After a limit resets, send "Continue where you left off." to threads it stopped.
    pub continue_after_reset: bool,
    /// After a host restart, continue turns it cut off (at most 3 times in a row).
    pub continue_after_restart: bool,
    /// Deny an approval nobody answered after this many minutes (`None`: wait forever).
    pub deny_unanswered_after_minutes: Option<u32>,
    /// Approvals matching these are answered "allow" without asking (`Tool` or `Bash:prefix`).
    pub always_allow: Vec<String>,
    /// Approvals matching these are denied without asking.
    pub never_allow: Vec<String>,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            reroute_when_out: false,
            continue_after_reset: true,
            continue_after_restart: true,
            deny_unanswered_after_minutes: None,
            always_allow: Vec::new(),
            never_allow: Vec::new(),
        }
    }
}

impl Store {
    pub fn settings(&self) -> Result<Settings> {
        let text: Option<String> = self.conn().query_row("SELECT value FROM settings WHERE key = 'host'", [], |r| r.get(0)).optional()?;
        Ok(match text {
            Some(t) => serde_json::from_str(&t)?,
            None => Settings::default(),
        })
    }

    pub fn settings_set(&self, s: &Settings) -> Result<()> {
        self.conn().execute(
            "INSERT INTO settings (key, value) VALUES ('host', ?1) ON CONFLICT (key) DO UPDATE SET value = ?1",
            params![serde_json::to_string(s)?],
        )?;
        Ok(())
    }

    /// When a thread should be told to continue (after its team's limit resets).
    pub fn thread_set_resume_at(&self, thread_id: &str, at: Option<i64>) -> Result<()> {
        self.conn().execute("UPDATE threads SET resume_at = ?2 WHERE id = ?1", params![thread_id, at])?;
        Ok(())
    }

    /// Idle threads whose continue time has come.
    pub fn threads_due(&self, now: i64) -> Result<Vec<String>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT id FROM threads WHERE resume_at IS NOT NULL AND resume_at <= ?1 AND status = 'idle'")?;
        let rows = stmt.query_map([now], |r| r.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
        Ok(rows)
    }

    /// Counts an automatic continue after a restart; a message from a person resets it.
    pub fn thread_auto_continues(&self, thread_id: &str, reset: bool) -> Result<u32> {
        let conn = self.conn();
        if reset {
            conn.execute("UPDATE threads SET auto_continues = 0 WHERE id = ?1", [thread_id])?;
        } else {
            conn.execute("UPDATE threads SET auto_continues = auto_continues + 1 WHERE id = ?1", [thread_id])?;
        }
        Ok(conn.query_row("SELECT auto_continues FROM threads WHERE id = ?1", [thread_id], |r| r.get::<_, i64>(0))? as u32)
    }

    /// Moves a thread to another team (same CLI): its account changes, its session continues.
    pub fn thread_set_team(&self, thread_id: &str, team_id: &str, config_dir: &str) -> Result<()> {
        self.conn().execute(
            "UPDATE threads SET team_id = ?2, config_dir = ?3 WHERE id = ?1",
            params![thread_id, team_id, config_dir],
        )?;
        Ok(())
    }
}

impl Store {
    /// Changes a thread's model, effort or freedom level (whatever is given).
    pub fn thread_set_options(&self, thread_id: &str, model: Option<&str>, effort: Option<&str>, freedom: Option<&str>) -> Result<()> {
        let conn = self.conn();
        if let Some(m) = model {
            conn.execute("UPDATE threads SET model = ?2 WHERE id = ?1", params![thread_id, m])?;
        }
        if let Some(e) = effort {
            conn.execute("UPDATE threads SET effort = ?2 WHERE id = ?1", params![thread_id, e])?;
        }
        if let Some(f) = freedom {
            conn.execute("UPDATE threads SET permission_mode = ?2 WHERE id = ?1", params![thread_id, f])?;
        }
        Ok(())
    }
}
