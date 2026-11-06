//! Team worktrees: each team thread works in its own checkout and branch, outside the
//! user's folder (in the host's data folder).
use super::run;
use anyhow::{Context, Result, bail};
use std::path::Path;

/// The commit the folder is on. A hand-off needs one: the team starts from it.
pub fn head(cwd: &Path) -> Result<String> {
    run(cwd, &["rev-parse", "--verify", "HEAD"]).context("the folder needs a git repository with at least one commit for a hand-off")
}

/// Creates `branch` at `base` and checks it out at `path`. On failure nothing is left behind.
pub fn add(repo: &Path, path: &Path, branch: &str, base: &str) -> Result<()> {
    if path.exists() {
        bail!("{} already exists", path.display());
    }
    if run(repo, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")]).is_ok() {
        bail!("branch {branch} already exists");
    }
    let p = path.to_string_lossy();
    if let Err(e) = run(repo, &["worktree", "add", "-b", branch, &p, base]) {
        let _ = remove(repo, path);
        let _ = run(repo, &["branch", "-D", branch]);
        return Err(e.context("could not create the team's worktree"));
    }
    Ok(())
}

/// Removes a worktree without `--force`. Links inside it (junctions, symlinks such as a
/// linked `node_modules`) are unlinked first: removing through them once deleted the files
/// they pointed to in Agent Hub's main checkout.
pub fn remove(repo: &Path, path: &Path) -> Result<()> {
    if path.exists() {
        crate::links::unlink_all(path, &[".git"])?;
        run(repo, &["worktree", "remove", &path.to_string_lossy()])?;
    }
    let _ = run(repo, &["worktree", "prune"]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::tests_support::repo;
    use super::*;

    #[test]
    fn a_worktree_is_added_on_its_own_branch_and_removed_cleanly() {
        let r = repo();
        let wt = r.with_extension("wt");
        add(&r, &wt, "curia/team-a", &head(&r).unwrap()).unwrap();
        assert!(wt.join("a.txt").exists());
        assert_eq!(run(&wt, &["branch", "--show-current"]).unwrap(), "curia/team-a");
        assert!(add(&r, &r.with_extension("wt2"), "curia/team-a", "HEAD").is_err(), "a branch is used once");
        remove(&r, &wt).unwrap();
        assert!(!wt.exists());
        let _ = std::fs::remove_dir_all(&r);
    }

    #[cfg(windows)]
    #[test]
    fn removing_a_worktree_never_follows_a_junction() {
        let r = repo();
        let outside = r.with_extension("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("precious.txt"), "keep me").unwrap();
        let wt = r.with_extension("wt");
        add(&r, &wt, "curia/team-j", &head(&r).unwrap()).unwrap();
        let link = wt.join("linked");
        let made = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J", &link.to_string_lossy(), &outside.to_string_lossy()])
            .output()
            .unwrap();
        assert!(made.status.success(), "could not make a junction");
        let _ = run(&wt, &["add", "-A"]); // even if git tracked it
        let _ = remove(&r, &wt);
        assert!(outside.join("precious.txt").exists(), "the junction's target must survive");
        let _ = std::fs::remove_dir_all(&wt);
        let _ = std::fs::remove_dir_all(&outside);
        let _ = std::fs::remove_dir_all(&r);
    }
}
