//! Each team's usage windows (from the CLI's own reports) and when it is out of quota.
use super::{Store, now_ms};
use crate::quota::Candidate;
use anyhow::Result;
use curia_core::UsageWindow;
use rusqlite::params;

impl Store {
    /// Saves the windows a CLI reported for a team. `limited` marks the team out of quota
    /// until the latest reset among them (or 5 hours from now when none is known).
    pub fn team_usage_set(&self, team_id: &str, windows: &[UsageWindow], limited: bool) -> Result<()> {
        let now = now_ms();
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        for w in windows {
            tx.execute(
                "INSERT INTO team_usage (team_id, window, utilization, resets_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT (team_id, window) DO UPDATE SET utilization = ?3, resets_at = ?4, updated_at = ?5",
                params![team_id, w.name, w.utilization, w.resets_at_ms, now],
            )?;
        }
        if limited {
            let until = windows.iter().filter_map(|w| w.resets_at_ms).max().unwrap_or(now + 5 * 3_600_000);
            tx.execute("UPDATE teams SET limited_until = ?2 WHERE id = ?1", params![team_id, until])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Marks a team out of quota until `until` (Unix ms).
    pub fn team_set_limited(&self, team_id: &str, until: i64) -> Result<()> {
        self.conn().execute("UPDATE teams SET limited_until = ?2 WHERE id = ?1", params![team_id, until])?;
        Ok(())
    }

    /// The figures the quota picker reads for one team.
    pub fn team_candidate(&self, team_id: &str) -> Result<Candidate> {
        let conn = self.conn();
        let (name, limited_until): (String, Option<i64>) =
            conn.query_row("SELECT name, limited_until FROM teams WHERE id = ?1", [team_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let mut stmt = conn.prepare("SELECT window, utilization, resets_at FROM team_usage WHERE team_id = ?1")?;
        let rows = stmt
            .query_map([team_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<f64>>(1)?, r.get::<_, Option<i64>>(2)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let left = |u: Option<f64>| u.map(|u| (100.0 * (1.0 - u)).clamp(0.0, 100.0));
        let mut c = Candidate { team: name, five_hour_left: None, five_hour_resets_at: None, weekly_left: None, weekly_resets_at: None, limited_until };
        for (window, utilization, resets) in rows {
            if window == "five_hour" {
                (c.five_hour_left, c.five_hour_resets_at) = (left(utilization), resets);
            } else if window.starts_with("seven_day") {
                // The tightest weekly window is the one that stops work first.
                let l = left(utilization);
                if c.weekly_left.is_none() || l < c.weekly_left {
                    (c.weekly_left, c.weekly_resets_at) = (l, resets);
                }
            }
        }
        Ok(c)
    }
}
