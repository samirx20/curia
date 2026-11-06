//! Git, by running the `git` program (the most dependable way to handle worktrees, merges
//! and undo). Checkpoints and diffs are in `checkpoint`.
pub mod checkpoint;
pub mod merge;
pub mod worktree;

#[cfg(test)]
pub(crate) mod tests_support {
    use std::path::PathBuf;

    /// A scratch repository with one commit (`a.txt`).
    pub fn repo() -> PathBuf {
        let d = std::env::temp_dir().join(format!("curia-repo-{}", &crate::store::new_id()[..8]));
        std::fs::create_dir_all(&d).unwrap();
        super::run(&d, &["init", "-q", "-b", "main"]).unwrap();
        std::fs::write(d.join("a.txt"), "one\n").unwrap();
        super::run(&d, &["add", "."]).unwrap();
        let who = [("GIT_AUTHOR_NAME", "t"), ("GIT_AUTHOR_EMAIL", "t@t"), ("GIT_COMMITTER_NAME", "t"), ("GIT_COMMITTER_EMAIL", "t@t")];
        super::run_env(&d, &["commit", "-qm", "init"], &who).unwrap();
        d
    }
}

use anyhow::{Result, bail};
use std::path::Path;
use std::process::Command;

/// Runs git in `cwd` with extra environment; returns trimmed stdout, or git's error text.
pub fn run_env(cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.args(["-c", "core.quotePath=false"]).args(args).current_dir(cwd).env("GIT_TERMINAL_PROMPT", "0");
    for (k, v) in env {
        cmd.env(k, v);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().map_err(|e| anyhow::anyhow!("could not run git: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        bail!("git {} failed: {}", args.first().unwrap_or(&""), if err.is_empty() { "no message" } else { &err });
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

pub fn run(cwd: &Path, args: &[&str]) -> Result<String> {
    run_env(cwd, args, &[])
}

pub fn is_repo(cwd: &Path) -> bool {
    run(cwd, &["rev-parse", "--is-inside-work-tree"]).is_ok_and(|s| s == "true")
}
