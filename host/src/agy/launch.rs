//! How an agy thread is started: stream-json in and out, one process per thread, on the
//! team's `--gemini_dir` with `SSH_CONNECTION` set (agy 1.5.2, checked 2026-11-07).
use anyhow::{Result, bail};
use std::path::{Path, PathBuf};
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct Launch {
    /// The `--gemini_dir`: the team's folder, or the thread's overlay of it (`home`). With
    /// `SSH_CONNECTION` set, this is what keeps accounts apart.
    pub gemini_dir: PathBuf,
    pub cwd: PathBuf,
    pub model: Option<String>,
    pub effort: Option<String>,
    /// Curia's freedom level. Headless agy refuses every command unless permissions are
    /// skipped, so anything but Plan runs as Full, fenced by the thread's worktree.
    pub freedom: String,
    pub resume: Option<String>,
    /// Sent ahead of the first message (agy has no flag for added instructions).
    pub instructions: Option<String>,
}

impl Launch {
    pub fn command(&self, exe: &Path) -> Command {
        let mut cmd = Command::new(exe);
        cmd.arg(format!("--gemini_dir={}", self.gemini_dir.display()))
            .args(["--input-format", "stream-json", "--output-format", "stream-json"])
            .current_dir(&self.cwd)
            .env("SSH_CONNECTION", "1");
        if crate::freedom::level(&self.freedom) == "plan" {
            cmd.args(["--mode", "plan"]);
        } else {
            cmd.arg("--dangerously-skip-permissions");
        }
        if let Some(m) = &self.model {
            cmd.args(["--model", m]);
        }
        if let Some(e) = &self.effort {
            cmd.args(["--effort", e]);
        }
        if let Some(id) = &self.resume {
            cmd.arg(format!("--conversation={id}"));
        }
        cmd.arg("-p="); // the `=` matters: `-p` alone swallows the next flag
        cmd
    }
}

/// The agy executable: `CURIA_AGY` if set, then `%LOCALAPPDATA%\agy\bin`, then PATH.
pub fn find_agy() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("CURIA_AGY") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Ok(p);
        }
        bail!("CURIA_AGY points at {}, which is not a file", p.display());
    }
    let name = if cfg!(windows) { "agy.exe" } else { "agy" };
    let local = std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("agy").join("bin").join(name));
    let path = std::env::var_os("PATH").unwrap_or_default();
    local.into_iter().chain(std::env::split_paths(&path).map(|d| d.join(name))).find(|p| p.is_file())
        .ok_or_else(|| anyhow::anyhow!("could not find Antigravity (agy); install it or set CURIA_AGY to agy.exe"))
}

/// Whether this `--gemini_dir` holds the agy conversation.
pub fn session_visible(gemini_dir: &Path, conversation: &str) -> bool {
    gemini_dir.join("antigravity-cli").join("conversations").join(format!("{conversation}.db")).is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_is_plan_and_everything_else_skips_permissions() {
        let mut l = Launch {
            gemini_dir: "g".into(), cwd: ".".into(), model: None, effort: None, freedom: "plan".into(),
            resume: Some("c-1".into()), instructions: None,
        };
        let args = |l: &Launch| l.command(Path::new("agy")).as_std().get_args().map(|a| a.to_string_lossy().into_owned()).collect::<Vec<_>>();
        let a = args(&l);
        assert!(a.contains(&"plan".to_string()) && !a.contains(&"--dangerously-skip-permissions".to_string()));
        assert!(a.contains(&"--conversation=c-1".to_string()));
        assert_eq!(a.last().unwrap(), "-p=");
        l.freedom = "ask".into();
        assert!(args(&l).contains(&"--dangerously-skip-permissions".to_string()));
    }
}
