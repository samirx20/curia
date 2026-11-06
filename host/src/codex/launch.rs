//! How a Codex thread is started: `codex app-server` (JSON-RPC over stdio) on the team's own
//! `CODEX_HOME`, with Curia's control room added for this launch only (`-c` overrides, never
//! the user's config file). Checked against codex-cli 0.167.0 on 2026-11-07.
use anyhow::{Result, bail};
use std::path::{Path, PathBuf};
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct Launch {
    /// The team's own `CODEX_HOME`: what keeps accounts apart.
    pub codex_home: PathBuf,
    pub cwd: PathBuf,
    pub model: Option<String>,
    pub effort: Option<String>,
    /// Curia's freedom level (`plan`, `ask`, `sandbox`, `full`).
    pub freedom: String,
    /// The Codex thread id, to continue an earlier conversation.
    pub resume: Option<String>,
    /// The control room's URL and this thread's token.
    pub control_room: Option<(String, String)>,
    /// What this thread's role adds to Codex's instructions.
    pub instructions: Option<String>,
}

/// The environment variable that carries the control-room token to Codex.
const TOKEN_VAR: &str = "CURIA_MCP_TOKEN";

impl Launch {
    pub fn command(&self, exe: &Path) -> Command {
        let mut cmd = Command::new(exe);
        cmd.arg("app-server").current_dir(&self.cwd).env("CODEX_HOME", &self.codex_home);
        if let Some((url, token)) = &self.control_room {
            cmd.arg("-c").arg(format!("mcp_servers.{}.url=\"{url}\"", crate::claude::CONTROL_ROOM));
            cmd.arg("-c").arg(format!("mcp_servers.{}.bearer_token_env_var=\"{TOKEN_VAR}\"", crate::claude::CONTROL_ROOM));
            // Curia's own tools need no approval (the host checks every call), as with Claude.
            cmd.arg("-c").arg(format!("mcp_servers.{}.default_tools_approval_mode=\"approve\"", crate::claude::CONTROL_ROOM));
            cmd.env(TOKEN_VAR, token);
        }
        cmd
    }

    /// Codex's approval policy and sandbox for the freedom level. A level Codex can't match
    /// exactly takes the more cautious side.
    pub fn policy(&self) -> (&'static str, &'static str) {
        match crate::freedom::level(&self.freedom) {
            "plan" => ("untrusted", "read-only"),
            "sandbox" => ("on-request", "workspace-write"),
            "full" => ("never", "danger-full-access"),
            _ => ("untrusted", "workspace-write"),
        }
    }
}

/// The real Codex executable: `CURIA_CODEX` if set, `codex.exe` on PATH, or the binary inside
/// the npm package that the `codex` script on PATH starts (scripts can't be driven over stdio).
pub fn find_codex() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("CURIA_CODEX") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Ok(p);
        }
        bail!("CURIA_CODEX points at {}, which is not a file", p.display());
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path) {
        let exe = dir.join(if cfg!(windows) { "codex.exe" } else { "codex" });
        if exe.is_file() {
            return Ok(exe);
        }
        let npm = dir.join("node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe");
        if npm.is_file() {
            return Ok(npm);
        }
    }
    bail!("could not find Codex; install it (npm i -g @openai/codex) or set CURIA_CODEX to codex.exe")
}

/// Whether this `CODEX_HOME` holds the Codex thread: `sessions/YYYY/MM/DD/rollout-…-<id>.jsonl`.
pub fn session_visible(codex_home: &Path, thread: &str) -> bool {
    fn walk(dir: &Path, end: &str, depth: u8) -> bool {
        let Ok(entries) = std::fs::read_dir(dir) else { return false };
        entries.flatten().any(|e| {
            let p = e.path();
            if p.is_dir() { depth > 0 && walk(&p, end, depth - 1) } else { p.file_name().is_some_and(|n| n.to_string_lossy().ends_with(end)) }
        })
    }
    walk(&codex_home.join("sessions"), &format!("-{thread}.jsonl"), 3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freedom_levels_map_to_codex_policies() {
        let mut l = Launch {
            codex_home: "h".into(), cwd: ".".into(), model: None, effort: None, freedom: "ask".into(),
            resume: None, control_room: None, instructions: None,
        };
        assert_eq!(l.policy(), ("untrusted", "workspace-write"));
        l.freedom = "full".into();
        assert_eq!(l.policy(), ("never", "danger-full-access"));
        l.freedom = "nonsense".into();
        assert_eq!(l.policy(), ("untrusted", "workspace-write"), "unknown falls back to asking");
    }
}
