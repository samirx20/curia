//! Teams and their quota: usage as the CLIs report it, picking a team, moving a thread to
//! another team, and what happens when a team runs out (Scope §3.2, §3.7; CARRY_OVER §6).
use super::Host;
use crate::quota;
use crate::store::{now_ms, TeamRow};
use anyhow::{Result, bail};
use curia_core::{Event, UsageWindow};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;

/// What a thread is told once its team's limit has reset.
pub const CONTINUE: &str = "Continue where you left off.";

impl Host {
    /// A CLI reported usage for a thread's team.
    pub(super) fn usage_seen(&self, thread_id: &str, windows: &[UsageWindow], limited: bool) -> Result<()> {
        if let Some(team) = self.store.thread_get(thread_id)?.team_id {
            self.store.team_usage_set(&team, windows, limited)?;
        }
        Ok(())
    }

    /// Asks a freshly started CLI for its usage, so a team's figures are current before it works.
    pub(super) async fn refresh_usage(&self, thread_id: &str, session: &crate::cli::Session) {
        match session.usage().await {
            Ok(windows) if !windows.is_empty() => {
                if let Err(e) = self.usage_seen(thread_id, &windows, false) {
                    eprintln!("curia-host: could not save usage: {e:#}");
                }
            }
            Ok(_) => {}
            Err(e) => eprintln!("curia-host: Claude did not report usage: {e:#}"),
        }
    }

    /// The team to use: by name, or `auto` for the one with the most quota left. `auto` only
    /// considers teams free in the caller's run (`run_root`), which rules out the caller's own
    /// team while it works; the caller's team also keeps a reserve.
    pub(super) fn choose_team(&self, requested: &str, caller_team: Option<&str>, run_root: Option<&str>) -> Result<TeamRow> {
        if requested != "auto" {
            return self.store.team(requested);
        }
        let mut teams = self.store.teams()?;
        if let Some(root) = run_root {
            let mut free = Vec::new();
            for t in teams {
                if self.store.team_busy_in_run(&t.id, root)?.is_empty() {
                    free.push(t);
                }
            }
            teams = free;
        }
        if teams.is_empty() {
            bail!("no team for `auto`: every team is busy in this run");
        }
        let candidates = teams.iter().map(|t| self.store.team_candidate(&t.id)).collect::<Result<Vec<_>>>()?;
        let caller_name = caller_team.and_then(|id| teams.iter().find(|t| t.id == id)).map(|t| t.name.as_str());
        let name = quota::pick(&candidates, caller_name, now_ms()).map_err(|e| anyhow::anyhow!("no team for `auto`: {e}"))?;
        self.store.team(&name)
    }

    /// Teams with their quota as agents and clients see it.
    pub fn teams_with_quota(&self, run_root: Option<&str>) -> Result<Vec<Value>> {
        let mut out = Vec::new();
        for t in self.store.teams()? {
            let c = self.store.team_candidate(&t.id)?;
            let busy = match run_root {
                Some(root) => !self.store.team_busy_in_run(&t.id, root)?.is_empty(),
                None => false,
            };
            let out_until = c.limited_until.filter(|u| *u > now_ms());
            out.push(json!({
                "id": t.id, "name": t.name, "cli": t.cli, "model": t.model, "effort": t.effort, "busy_in_this_run": busy,
                "config_dir": t.config_dir, "signed_in": super::admin::signed_in(&t.cli, &t.config_dir),
                "five_hour_percent_left": c.five_hour_left.map(f64::round), "five_hour_resets_at_ms": c.five_hour_resets_at,
                "weekly_percent_left": c.weekly_left.map(f64::round), "weekly_resets_at_ms": c.weekly_resets_at,
                "out_of_quota_until_ms": out_until,
            }));
        }
        Ok(out)
    }

    /// Moves an idle thread to another team of the same CLI. Its conversation continues, so
    /// the new team's account must be able to resume the CLI's session (Claude accounts that
    /// share their history folder can; CARRY_OVER §5). A move to another CLI needs a hand-off
    /// summary, which comes with the second CLI.
    pub async fn move_thread(&self, thread_id: &str, team: &str) -> Result<()> {
        let row = self.store.thread_get(thread_id)?;
        let to = self.store.team(team)?;
        if row.team_id.as_deref() == Some(to.id.as_str()) {
            return Ok(());
        }
        if row.status == "running" {
            bail!("the thread is mid-turn; move it once the turn ends");
        }
        if to.cli != row.cli {
            bail!("moving a thread to another CLI needs a hand-off summary, which isn't built yet");
        }
        if let Some(session) = &row.native_session_id {
            if !crate::cli::session_visible(&row.cli, Path::new(&to.config_dir), session) {
                bail!("team {:?} can't see this thread's session (its account doesn't share history), so the thread can't continue there", to.name);
            }
        }
        if let Some(live) = self.live_of(thread_id) {
            live.session.close().await;
            for _ in 0..50 {
                if !self.is_live(thread_id) {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            if self.is_live(thread_id) {
                bail!("the thread's CLI did not stop, so it can't be moved");
            }
        }
        self.store.thread_set_team(thread_id, &to.id, &to.config_dir)?;
        self.record(thread_id, &[Event::Notice { text: format!("Moved to team {}.", to.name) }])
    }

    /// The thread's team ran out mid-turn: move it to a team with quota (if allowed and
    /// possible), else wait for the reset and continue then.
    pub(super) async fn out_of_quota(self: &Arc<Self>, thread_id: &str) -> Result<()> {
        let row = self.store.thread_get(thread_id)?;
        let Some(team_id) = row.team_id.clone() else { return Ok(()) };
        let c = self.store.team_candidate(&team_id)?;
        let reset = c.limited_until.or(c.five_hour_resets_at).filter(|t| *t > now_ms()).unwrap_or(now_ms() + 5 * 3_600_000);
        self.store.team_set_limited(&team_id, reset)?;
        let settings = self.store.settings()?;
        if settings.reroute_when_out {
            if let Ok(to) = self.choose_team("auto", None, None) {
                if to.id != team_id && self.move_thread(thread_id, &to.name).await.is_ok() {
                    self.deliver_from_curia(thread_id, CONTINUE).await?;
                    return Ok(());
                }
            }
        }
        if settings.continue_after_reset {
            self.store.thread_set_resume_at(thread_id, Some(reset + 60_000))?;
            let text = format!("Team out of quota; this thread continues after its limit resets (in about {} min).", (reset - now_ms()) / 60_000 + 1);
            self.record(thread_id, &[Event::Notice { text }])?;
        }
        Ok(())
    }

    /// Every 30 s: continue threads whose limit has reset. A thread that fails to start is
    /// tried again 5 minutes later, and says so.
    pub fn start_ticker(self: &Arc<Self>) {
        let host = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                for id in host.store.threads_due(now_ms()).unwrap_or_default() {
                    let _ = host.store.thread_set_resume_at(&id, None);
                    if let Err(e) = host.deliver_from_curia(&id, CONTINUE).await {
                        let _ = host.store.thread_set_resume_at(&id, Some(now_ms() + 5 * 60_000));
                        let _ = host.record(&id, &[Event::Notice { text: format!("Could not continue after the reset: {e:#}. Trying again in 5 minutes.") }]);
                    }
                }
            }
        });
    }
}

