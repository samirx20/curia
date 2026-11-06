//! Git, by running the `git` program (the most dependable way to handle worktrees, merges
//! and undo). Checkpoints and diffs are in `checkpoint`.
pub mod checkpoint;

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
