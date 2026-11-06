//! The control room: handing work to teams, getting results back, merging, undoing.
//! Rules from Agent Hub (CARRY_OVER §2, §3, §20): a child gets only the task text, never more
//! freedom than its parent; a result is delivered exactly once; a failed step reports itself.
use super::Host;
use crate::git::{merge, worktree};
use crate::store::{MergeRow, NewThread, ThreadRow, new_id};
use anyhow::{Context, Result, bail};
use curia_core::{Event, MergeReport, TurnOutcome};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;

/// Main thread → team → team, and no deeper (Scope §3.2; a setting later).
pub const MAX_DEPTH: u32 = 2;

const MAIN: &str = "You are the main thread of a Curia run. Curia's tools are on the `curia` MCP server: list_teams, hand_off, send_message, read_messages, ask_human. hand_off gives one self-contained task to a team: it gets only the task text (not this conversation), works on its own git branch from your latest commit, and its result comes back to you as a message when it finishes, with its branch merged into your folder. Commit your own work first if the team needs it. Don't wait or poll for a result: end your turn, and Curia wakes you when it arrives.";
const TEAM: &str = "You are a team thread in a Curia run. Your task is the first message. Work only in your current folder: it is your own git branch. You don't need to commit: Curia commits what you leave when you hand back. When the task is done, call report_result on the `curia` MCP server with a short summary of what you did and anything the thread that gave you the task must know, then end your turn. send_message reaches that thread; ask_human asks the user.";

/// What the thread's system prompt adds, by its role in the run.
pub fn instructions(row: &ThreadRow) -> &'static str {
    if row.parent_id.is_some() { TEAM } else { MAIN }
}

impl Host {
    pub async fn hand_off(self: &Arc<Self>, caller: &str, team: &str, title: &str, task: &str, request_id: Option<&str>) -> Result<Value> {
        let parent = self.store.thread_get(caller)?;
        if let Some(r) = request_id {
            if let Some(done) = self.store.thread_by_request(caller, r)? {
                return Ok(json!({"thread_id": done.id, "note": "this request already started that thread"}));
            }
        }
        if parent.depth + 1 > MAX_DEPTH {
            bail!("hand-offs go at most {MAX_DEPTH} levels deep; do this part yourself");
        }
        let team = self.choose_team(team, parent.team_id.as_deref())?;
        if let Some(busy) = self.store.team_busy_in_run(&team.id, &parent.root_id)?.first() {
            bail!("team {:?} is busy in this run (thread {busy}); give it more work with send_message, or pick another team", team.name);
        }
        let short = new_id()[..8].to_string();
        let branch = format!("curia/{short}-{}", slug(&team.name));
        let path = self.data_dir.join("worktrees").join(&parent.root_id[..8]).join(&short);
        let (repo, wt, b) = (PathBuf::from(&parent.cwd), path.clone(), branch.clone());
        let dirty = tokio::task::spawn_blocking(move || -> Result<bool> {
            let base = worktree::head(&repo)?;
            worktree::add(&repo, &wt, &b, &base)?;
            Ok(!crate::git::run(&repo, &["status", "--porcelain"])?.is_empty())
        })
        .await??;
        let child = self.store.thread_create(&NewThread {
            title: title.to_string(),
            cli: team.cli.clone(),
            config_dir: team.config_dir.clone(),
            cwd: path.to_string_lossy().into_owned(),
            model: team.model.clone().or(parent.model.clone()),
            effort: team.effort.clone().or(parent.effort.clone()),
            permission_mode: parent.permission_mode.clone(),
            team_id: Some(team.id.clone()),
            parent_id: Some(parent.id.clone()),
            depth: parent.depth + 1,
            branch: Some(branch.clone()),
            client_request_id: request_id.map(str::to_string),
        });
        let child = match child {
            Ok(c) => c,
            Err(e) => {
                let (repo, wt) = (PathBuf::from(&parent.cwd), path.clone());
                let _ = tokio::task::spawn_blocking(move || worktree::remove(&repo, &wt)).await;
                let _ = crate::git::run(parent.cwd.as_ref(), &["branch", "-D", &branch]);
                return Err(e);
            }
        };
        self.store.result_expect(&child.id, &parent.id)?;
        self.record(&parent.id, &[Event::HandedOff {
            thread_id: child.id.clone(), team: team.name.clone(), title: title.to_string(), branch: Some(branch.clone()),
        }])?;
        self.start_turn(&child.id, task).await.context("the team thread could not start")?;
        let mut note = "The team is working. Its result will arrive here as a message when it finishes; end your turn instead of waiting.".to_string();
        if dirty {
            note.push_str(" Your uncommitted changes are not in the team's checkout.");
        }
        Ok(json!({"thread_id": child.id, "team": team.name, "branch": branch, "note": note}))
    }

    pub fn report_result(&self, caller: &str, summary: &str) -> Result<Value> {
        self.store.result_report(caller, summary)?;
        Ok(json!({"note": "Result saved. End your turn now; it is handed back when the turn ends."}))
    }

    /// After a team thread's turn: commit what it left, merge its branch into the parent's
    /// folder (a checkpoint first, so it can be undone), and tell the parent. Exactly once.
    pub(super) async fn hand_back(self: &Arc<Self>, child_id: &str, turn_id: &str, outcome: &TurnOutcome) -> Result<()> {
        let Some(claimed) = self.store.result_claim(child_id)? else { return Ok(()) };
        let child = self.store.thread_get(child_id)?;
        let parent = self.store.thread_get(&claimed.parent_id)?;
        let inferred = claimed.summary.is_none();
        let summary = match claimed.summary {
            Some(s) => s,
            None => self.store.last_text_of_turn(turn_id)?.unwrap_or_else(|| "(the team ended without a reply)".into()),
        };
        let ok = matches!(outcome, TurnOutcome::Completed);
        let merge = self.merge_child(&child, &parent, ok).await;
        self.record(&parent.id, &[Event::HandBack { from: child.id.clone(), summary: summary.clone(), inferred, ok, merge: merge.clone() }])?;
        let mut text = format!("Result from team thread \"{}\" ({}){}:\n{summary}\n\n", child.title, child.id,
            if inferred { ", taken from its last message because it never called report_result" } else { "" });
        if !ok {
            text.push_str(&format!("Its turn did not complete ({outcome:?}).\n"));
        }
        text.push_str(&match &merge {
            MergeReport::Merged { files, branch, merge_id } => format!("Its branch {branch} was merged into your folder ({} file(s); merge {merge_id}, which the user can undo).", files.len()),
            MergeReport::NothingToMerge { branch } => format!("Its branch {branch} had no changes."),
            MergeReport::Failed { branch, reason } => format!("Its branch {branch} was NOT merged: {reason}. The work is still on that branch."),
            MergeReport::NoBranch => String::new(),
        });
        self.deliver_as_written(&parent.id, Some(&child.id), &summary, &text).await.map(|_| ())
    }

    async fn merge_child(&self, child: &ThreadRow, parent: &ThreadRow, ok: bool) -> MergeReport {
        let Some(branch) = child.branch.clone() else { return MergeReport::NoBranch };
        if !ok {
            return MergeReport::Failed { branch, reason: "the team's turn did not complete, so nothing was merged".into() };
        }
        let (wt, cwd, b) = (PathBuf::from(&child.cwd), PathBuf::from(&parent.cwd), branch.clone());
        let message = format!("Merge team thread \"{}\"", child.title);
        let done = tokio::task::spawn_blocking(move || -> Result<merge::Outcome> {
            merge::commit_leftovers(&wt, "curia: work the team left uncommitted")?;
            merge::merge(&cwd, &b, &message)
        })
        .await;
        match done {
            Ok(Ok(merge::Outcome::Merged(m))) => {
                let id = new_id();
                let row = MergeRow {
                    id: id.clone(), thread_id: parent.id.clone(), from_thread: child.id.clone(), cwd: parent.cwd.clone(),
                    branch: branch.clone(), pre_head: m.pre_head, merge_commit: m.merge_commit, checkpoint: m.checkpoint, state: "merged".into(),
                };
                match self.store.merge_add(&row) {
                    Ok(()) => MergeReport::Merged { merge_id: id, branch, files: m.files },
                    Err(e) => MergeReport::Failed { branch, reason: format!("merged, but the record of it failed: {e:#}") },
                }
            }
            Ok(Ok(merge::Outcome::NothingNew)) => MergeReport::NothingToMerge { branch },
            Ok(Ok(merge::Outcome::Refused(reason))) => MergeReport::Failed { branch, reason },
            Ok(Err(e)) => MergeReport::Failed { branch, reason: format!("{e:#}") },
            Err(e) => MergeReport::Failed { branch, reason: format!("{e}") },
        }
    }

    /// Undoes a hand-back merge: the parent's folder goes back to just before it.
    pub async fn undo_merge(&self, merge_id: &str) -> Result<()> {
        let m = self.store.merge_get(merge_id)?.context("no such merge")?;
        if m.state != "merged" {
            bail!("this merge was already undone");
        }
        let merged = merge::Merged { pre_head: m.pre_head.clone(), merge_commit: m.merge_commit.clone(), checkpoint: m.checkpoint.clone(), files: vec![] };
        let cwd = PathBuf::from(&m.cwd);
        tokio::task::spawn_blocking(move || merge::undo(&cwd, &merged)).await??;
        self.store.merge_set_state(merge_id, "undone")?;
        self.record(&m.thread_id, &[Event::MergeUndone { merge_id: merge_id.to_string() }])
    }

    pub fn list_teams(&self, caller: &str) -> Result<Value> {
        let root = self.store.thread_get(caller)?.root_id;
        Ok(json!({"teams": self.teams_with_quota(Some(&root))?, "note": "hand_off accepts team \"auto\": the team with the most quota left"}))
    }

    /// Messages queued for the caller (they would otherwise arrive when its turn ends).
    pub fn read_messages(&self, caller: &str) -> Result<Value> {
        let msgs = self.store.mailbox_take(caller)?;
        Ok(json!({"messages": msgs.into_iter().map(|(from, text)| json!({"from": from, "text": text})).collect::<Vec<_>>()}))
    }

    /// `send_message` from an agent: only within its run, and never to a thread with more freedom.
    pub async fn send_message(self: &Arc<Self>, caller: &str, to: &str, text: &str) -> Result<Value> {
        let (from, target) = (self.store.thread_get(caller)?, self.store.thread_get(to)?);
        if from.root_id != target.root_id {
            bail!("you can only message threads in your own run");
        }
        if crate::freedom::rank(&target.permission_mode) > crate::freedom::rank(&from.permission_mode) {
            bail!("that thread runs with more freedom than yours");
        }
        let how = self.deliver(to, Some(caller), text).await?;
        Ok(json!({"delivered": how}))
    }
}

fn slug(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-")
}
