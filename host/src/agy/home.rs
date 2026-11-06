//! Curia's own `--gemini_dir` for one agy thread. agy reads MCP servers only from
//! `<gemini_dir>\config\mcp_config.json` and expands no variables there (agy 1.5.2, checked
//! 2026-11-07), and `--settings` refuses `mcpServers`. So each thread gets an overlay folder:
//! the account's folders linked in (sign-in, conversations, projects), and a config file of
//! its own with the account's servers plus the control room. Curia never writes in the
//! account's folder; agy itself writes there through the links, as it would anyway.
use crate::links;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::path::Path;

const MCP_FILE: &str = "mcp_config.json";

/// Builds (or rebuilds) the overlay at `home` for the account folder `account`.
pub fn prepare(account: &Path, home: &Path, url: &str, token: &str) -> Result<()> {
    if !account.is_dir() {
        bail!("the team's Antigravity folder {} doesn't exist", account.display());
    }
    links::remove_tree(home)?;
    std::fs::create_dir_all(home.join("config")).with_context(|| format!("could not make {}", home.display()))?;
    mirror(account, home, &["config"])?;
    let config = account.join("config");
    if config.is_dir() {
        mirror(&config, &home.join("config"), &[MCP_FILE])?;
    }
    let mut servers = std::fs::read_to_string(config.join(MCP_FILE)).ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    if !servers["mcpServers"].is_object() {
        servers["mcpServers"] = json!({});
    }
    servers["mcpServers"][crate::claude::CONTROL_ROOM] = json!({"serverUrl": url, "headers": {"Authorization": format!("Bearer {token}")}});
    std::fs::write(home.join("config").join(MCP_FILE), serde_json::to_string_pretty(&servers)?).context("could not write the thread's MCP config")
}

/// Links every entry of `from` into `to`, except `skip`: folders as links, files as hard links.
fn mirror(from: &Path, to: &Path, skip: &[&str]) -> Result<()> {
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        if skip.iter().any(|s| entry.file_name() == *s) {
            continue;
        }
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if src.is_dir() {
            links::link_dir(&src, &dst)?;
        } else {
            std::fs::hard_link(&src, &dst).with_context(|| format!("could not link {}", src.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_overlay_links_the_account_and_adds_the_control_room() {
        let base = std::env::temp_dir().join(format!("curia-agyhome-{}", uuid::Uuid::new_v4().simple()));
        let (account, home) = (base.join("account"), base.join("home"));
        std::fs::create_dir_all(account.join("antigravity-cli")).unwrap();
        std::fs::create_dir_all(account.join("config").join("projects")).unwrap();
        std::fs::write(account.join("antigravity-cli").join("antigravity-oauth-token"), "secret").unwrap();
        std::fs::write(account.join("config").join(MCP_FILE), r#"{"mcpServers": {"mine": {"serverUrl": "http://x"}}}"#).unwrap();
        prepare(&account, &home, "http://127.0.0.1:1/mcp", "tok").unwrap();
        assert!(home.join("antigravity-cli").join("antigravity-oauth-token").exists(), "sign-in is visible");
        assert!(home.join("config").join("projects").is_dir());
        let mcp: Value = serde_json::from_str(&std::fs::read_to_string(home.join("config").join(MCP_FILE)).unwrap()).unwrap();
        assert_eq!(mcp["mcpServers"]["mine"]["serverUrl"], "http://x", "the account's own servers stay");
        assert_eq!(mcp["mcpServers"]["curia"]["headers"]["Authorization"], "Bearer tok");
        let untouched = std::fs::read_to_string(account.join("config").join(MCP_FILE)).unwrap();
        assert!(!untouched.contains("curia"), "the account's file is never written");
        prepare(&account, &home, "http://127.0.0.1:2/mcp", "tok2").unwrap(); // rebuilds cleanly
        links::remove_tree(&home).unwrap();
        assert!(account.join("antigravity-cli").join("antigravity-oauth-token").exists(), "removing the overlay keeps the account");
        let _ = std::fs::remove_dir_all(&base);
    }
}
