//! Teams: one signed-in CLI account (its own config folder), used by many threads.
use super::{Store, new_id, now_ms};
use anyhow::{Context, Result, bail};
use rusqlite::{OptionalExtension, Row, params};

#[derive(Debug, Clone, PartialEq)]
pub struct TeamRow {
    pub id: String,
    pub name: String,
    pub cli: String,
    pub config_dir: String,
    pub model: Option<String>,
    pub effort: Option<String>,
}

fn row(r: &Row) -> rusqlite::Result<TeamRow> {
    Ok(TeamRow { id: r.get(0)?, name: r.get(1)?, cli: r.get(2)?, config_dir: r.get(3)?, model: r.get(4)?, effort: r.get(5)? })
}

const COLUMNS: &str = "id, name, cli, config_dir, model, effort";

impl Store {
    pub fn team_create(&self, name: &str, cli: &str, config_dir: &str, model: Option<&str>, effort: Option<&str>) -> Result<TeamRow> {
        if name.trim().is_empty() {
            bail!("a team needs a name");
        }
        let id = new_id();
        self.conn()
            .execute(
                "INSERT INTO teams (id, name, cli, config_dir, model, effort, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![id, name.trim(), cli, config_dir, model, effort, now_ms()],
            )
            .with_context(|| format!("could not add team {name:?} (is the name taken?)"))?;
        self.team(&id)
    }

    /// A team by id or by name.
    pub fn team(&self, id_or_name: &str) -> Result<TeamRow> {
        let sql = format!("SELECT {COLUMNS} FROM teams WHERE id = ?1 OR name = ?1");
        self.conn().query_row(&sql, [id_or_name], row).optional()?.with_context(|| format!("no team {id_or_name:?}"))
    }

    pub fn teams(&self) -> Result<Vec<TeamRow>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!("SELECT {COLUMNS} FROM teams ORDER BY name"))?;
        let rows = stmt.query_map([], row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// The team's threads in this run that are mid-turn.
    pub fn team_busy_in_run(&self, team_id: &str, root_id: &str) -> Result<Vec<String>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id FROM threads WHERE team_id = ?1 AND COALESCE(root_id, id) = ?2 AND status = 'running'",
        )?;
        let rows = stmt.query_map(params![team_id, root_id], |r| r.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
        Ok(rows)
    }
}
