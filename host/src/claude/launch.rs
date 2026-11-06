//! How a Claude Code thread is started: which executable, which flags, which environment.
//! Flags follow Agent Hub's parked Rust app, checked against `claude` 2.1.318 on 2026-11-06
//! (the hidden `--permission-prompt-tool stdio` and thinking flags are still accepted).
use anyhow::{Result, bail};
use std::path::{Path, PathBuf};
use tokio::process::Command;

/// Everything that decides how one Claude process starts.
#[derive(Debug, Clone)]
pub struct Launch {
    /// The team's own `CLAUDE_CONFIG_DIR`: this is what keeps accounts apart.
    pub config_dir: PathBuf,
    /// The thread's working folder (its sandbox).
    pub cwd: PathBuf,
    pub model: Option<String>,
    pub effort: Option<String>,
    /// A CLI permission mode: `manual`, `acceptEdits`, `auto`, `plan`, `bypassPermissions`.
    pub permission_mode: String,
    /// The CLI's own session id, to continue an earlier conversation.
    pub resume: Option<String>,
    /// The control room: an MCP config file naming Curia's server (with this thread's token).
    pub mcp_config: Option<PathBuf>,
    /// Added to Claude's system prompt (how this thread uses the control room).
    pub append_system_prompt: Option<String>,
}

/// The MCP server name Curia's tools appear under (`mcp__curia__hand_off`, …).
pub const CONTROL_ROOM: &str = "curia";

impl Launch {
    pub fn args(&self) -> Vec<String> {
        let mut a: Vec<String> = [
            "-p",
            "--output-format", "stream-json",
            "--verbose",
            "--input-format", "stream-json",
            "--include-partial-messages",
            // Approvals arrive as `can_use_tool` control requests on stdout, answered on stdin.
            "--permission-prompt-tool", "stdio",
            "--thinking", "adaptive",
            "--thinking-display", "summarized",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        a.extend(["--permission-mode".into(), self.permission_mode.clone()]);
        if let Some(effort) = &self.effort {
            a.extend(["--effort".into(), effort.clone()]);
        }
        if let Some(model) = &self.model {
            a.extend(["--model".into(), model.clone()]);
        }
        if let Some(path) = &self.mcp_config {
            // Curia's own tools run without an approval card; everything else still asks.
            a.extend(["--mcp-config".into(), path.to_string_lossy().into_owned()]);
            a.extend(["--allowedTools".into(), format!("mcp__{CONTROL_ROOM}")]);
        }
        if let Some(prompt) = &self.append_system_prompt {
            a.extend(["--append-system-prompt".into(), prompt.clone()]);
        }
        if let Some(id) = &self.resume {
            a.extend(["--resume".into(), id.clone()]);
        }
        a
    }

    pub fn command(&self, exe: &Path) -> Command {
        let mut cmd = Command::new(exe);
        cmd.args(self.args())
            .current_dir(&self.cwd)
            .env("CLAUDE_CONFIG_DIR", &self.config_dir)
            // Claude drops an MCP call after 60 s by default; ask_human waits for the user.
            .env("MCP_TOOL_TIMEOUT", "86400000")
            // Never look like a nested Claude Code session, even when the host was started from one.
            .env_remove("CLAUDECODE")
            .env_remove("CLAUDE_CODE_ENTRYPOINT");
        cmd
    }
}

/// Finds the real Claude executable: `CURIA_CLAUDE` if set, else the first match on PATH.
/// `.cmd` and `.ps1` launcher shims can't be spawned with piped stdio, so only the binary counts.
pub fn find_claude() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("CURIA_CLAUDE") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Ok(p);
        }
        bail!("CURIA_CLAUDE points at {}, which is not a file", p.display());
    }
    let name = if cfg!(windows) { "claude.exe" } else { "claude" };
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
        .ok_or_else(|| anyhow::anyhow!("could not find {name} on PATH; set CURIA_CLAUDE to its full path"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_carry_mode_effort_model_and_resume() {
        let l = Launch {
            config_dir: "cfg".into(),
            cwd: ".".into(),
            model: Some("claude-haiku-5-5".into()),
            effort: Some("low".into()),
            permission_mode: "manual".into(),
            resume: Some("sess-1".into()),
            mcp_config: Some("mcp.json".into()),
            append_system_prompt: None,
        };
        let a = l.args().join(" ");
        assert!(a.starts_with("-p --output-format stream-json"));
        assert!(a.contains("--permission-prompt-tool stdio"));
        assert!(a.contains("--permission-mode manual"));
        assert!(a.contains("--effort low"));
        assert!(a.contains("--model claude-haiku-5-5"));
        assert!(a.contains("--mcp-config mcp.json --allowedTools mcp__curia"));
        assert!(a.ends_with("--resume sess-1"));
    }
}
