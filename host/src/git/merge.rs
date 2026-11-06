//! Bringing a team's branch into its parent's folder, and undoing that.
use super::checkpoint;
use super::{run, run_env};
use anyhow::{Context, Result, bail};
use std::path::Path;

const AUTHOR: &[(&str, &str)] = &[
    ("GIT_AUTHOR_NAME", "Curia"),
    ("GIT_AUTHOR_EMAIL", "curia@localhost"),
    ("GIT_COMMITTER_NAME", "Curia"),
    ("GIT_COMMITTER_EMAIL", "curia@localhost"),
];

/// Commits everything in `cwd` (a team's worktree) that the team left uncommitted.
/// Returns whether there was anything.
pub fn commit_leftovers(cwd: &Path, message: &str) -> Result<bool> {
    run(cwd, &["add", "-A"])?;
    if run(cwd, &["diff", "--cached", "--quiet"]).is_ok() {
        return Ok(false);
    }
    run_env(cwd, &["commit", "-q", "-m", message], AUTHOR)?;
    Ok(true)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Merged {
    pub pre_head: String,
    pub merge_commit: String,
    /// The folder as it was just before, uncommitted changes included (for undo).
    pub checkpoint: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Merged(Merged),
    NothingNew,
    /// Not merged (conflict, or local changes in the way); the folder is as it was.
    Refused(String),
}

/// Merges `branch` into the folder's current branch with a merge commit.
pub fn merge(cwd: &Path, branch: &str, message: &str) -> Result<Outcome> {
    let pre_head = run(cwd, &["rev-parse", "HEAD"])?;
    let behind = run(cwd, &["rev-list", "--count", &format!("HEAD..{branch}")])?;
    if behind.trim() == "0" {
        return Ok(Outcome::NothingNew);
    }
    let checkpoint = checkpoint::snapshot(cwd, "curia: before merge")?.context("not a git repository")?;
    if let Err(e) = run_env(cwd, &["merge", "--no-ff", "--no-edit", "-m", message, branch], AUTHOR) {
        let conflicted = run(cwd, &["diff", "--name-only", "--diff-filter=U"]).unwrap_or_default();
        if run(cwd, &["rev-parse", "-q", "--verify", "MERGE_HEAD"]).is_ok() {
            run(cwd, &["merge", "--abort"]).context("a failed merge could not be aborted")?;
        }
        let reason = if conflicted.is_empty() {
            format!("{e:#}")
        } else {
            format!("conflicts in {}", conflicted.lines().collect::<Vec<_>>().join(", "))
        };
        return Ok(Outcome::Refused(reason));
    }
    let merge_commit = run(cwd, &["rev-parse", "HEAD"])?;
    let files = run(cwd, &["diff", "--name-only", &pre_head, &merge_commit])?.lines().map(str::to_string).collect();
    Ok(Outcome::Merged(Merged { pre_head, merge_commit, checkpoint, files }))
}

/// Puts the folder back as it was before the merge: the branch pointer and the files
/// (uncommitted changes the user had then included). Refused once the folder has moved on.
pub fn undo(cwd: &Path, m: &Merged) -> Result<()> {
    let head = run(cwd, &["rev-parse", "HEAD"])?;
    if head != m.merge_commit {
        bail!("the folder has new commits since this merge, so it can't be undone automatically");
    }
    run(cwd, &["reset", "-q", "--hard", &m.pre_head])?;
    checkpoint::restore(cwd, &m.checkpoint)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::tests_support::repo;
    use super::super::worktree;
    use super::*;

    fn team_branch(r: &Path, name: &str, file: &str, text: &str) -> std::path::PathBuf {
        let wt = r.with_extension(name);
        worktree::add(r, &wt, &format!("curia/{name}"), &worktree::head(r).unwrap()).unwrap();
        std::fs::write(wt.join(file), text).unwrap();
        assert!(commit_leftovers(&wt, "team work").unwrap());
        wt
    }

    #[test]
    fn a_team_branch_merges_and_the_merge_can_be_undone() {
        let r = repo();
        let wt = team_branch(&r, "team-m", "b.txt", "from the team\n");
        std::fs::write(r.join("mine.txt"), "uncommitted, mine\n").unwrap();
        let Outcome::Merged(m) = merge(&r, "curia/team-m", "merge team").unwrap() else { panic!("expected a merge") };
        assert_eq!(m.files, vec!["b.txt"]);
        assert!(r.join("b.txt").exists() && r.join("mine.txt").exists());
        undo(&r, &m).unwrap();
        assert!(!r.join("b.txt").exists(), "the team's file is gone again");
        assert!(r.join("mine.txt").exists(), "the user's uncommitted file came back");
        assert_eq!(run(&r, &["rev-parse", "HEAD"]).unwrap(), m.pre_head);
        assert_eq!(merge(&r, "curia/team-m", "again").map(|o| matches!(o, Outcome::Merged(_))).unwrap(), true);
        let _ = worktree::remove(&r, &wt);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn a_conflict_is_refused_and_leaves_the_folder_as_it_was() {
        let r = repo();
        let wt = team_branch(&r, "team-c", "a.txt", "team's version\n");
        std::fs::write(r.join("a.txt"), "main's version\n").unwrap();
        commit_leftovers(&r, "main's change").unwrap();
        let before = run(&r, &["rev-parse", "HEAD"]).unwrap();
        let Outcome::Refused(reason) = merge(&r, "curia/team-c", "merge").unwrap() else { panic!("expected a refusal") };
        assert!(reason.contains("a.txt"), "{reason}");
        assert_eq!(run(&r, &["rev-parse", "HEAD"]).unwrap(), before);
        assert!(run(&r, &["rev-parse", "-q", "--verify", "MERGE_HEAD"]).is_err(), "no merge left half-done");
        let _ = worktree::remove(&r, &wt);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn a_branch_with_nothing_new_is_not_merged() {
        let r = repo();
        let wt = r.with_extension("same");
        worktree::add(&r, &wt, "curia/same", &worktree::head(&r).unwrap()).unwrap();
        assert_eq!(merge(&r, "curia/same", "m").unwrap(), Outcome::NothingNew);
        let _ = worktree::remove(&r, &wt);
        let _ = std::fs::remove_dir_all(&r);
    }
}
