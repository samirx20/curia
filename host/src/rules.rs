//! Always-allow and never-allow lists (Scope §3.4, §5). A rule is a tool name (`Write`), or a
//! tool and a prefix of its command (`Bash:git status`). Some actions always ask, whatever the
//! lists say: pushing, and deleting with a path outside the thread's folder.
use curia_core::Decision;
use serde_json::Value;

/// Actions that always ask (Scope §5 "Freedom and approvals").
const ALWAYS_ASK: &[&str] = &["Bash:git push"];

/// What the lists decide for an approval request: allow, deny, or `None` (ask the user).
pub fn decide(tool: &str, input: &Value, always_allow: &[String], never_allow: &[String]) -> Option<Decision> {
    if never_allow.iter().any(|r| matches(r, tool, input)) {
        return Some(Decision::Deny { message: Some(format!("Not allowed on this host ({tool} is on the never-allow list).")) });
    }
    if ALWAYS_ASK.iter().any(|r| matches(r, tool, input)) || deletes_outside(tool, input) {
        return None;
    }
    if always_allow.iter().any(|r| matches(r, tool, input)) {
        return Some(Decision::Allow);
    }
    None
}

fn matches(rule: &str, tool: &str, input: &Value) -> bool {
    match rule.split_once(':') {
        None => rule == tool,
        Some((t, prefix)) => t == tool && command(input).is_some_and(|c| c.trim_start().starts_with(prefix.trim())),
    }
}

fn command(input: &Value) -> Option<&str> {
    input["command"].as_str()
}

/// A delete command naming an absolute path or `..`: it may reach outside the folder.
fn deletes_outside(tool: &str, input: &Value) -> bool {
    let Some(c) = command(input).filter(|_| tool == "Bash" || tool == "PowerShell") else { return false };
    let deletes = ["rm ", "rmdir ", "del ", "Remove-Item", "rd "].iter().any(|d| c.contains(d));
    deletes && (c.contains("..") || c.contains(":\\") || c.contains(":/") || c.split_whitespace().any(|w| w.starts_with('/') || w.starts_with('~')))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn bash(c: &str) -> Value {
        json!({"command": c})
    }

    #[test]
    fn lists_allow_and_deny_by_tool_or_command_prefix() {
        let allow = vec!["Read".to_string(), "Bash:git status".to_string()];
        let deny = vec!["Bash:curl".to_string()];
        assert_eq!(decide("Read", &json!({}), &allow, &deny), Some(Decision::Allow));
        assert_eq!(decide("Bash", &bash("git status --short"), &allow, &deny), Some(Decision::Allow));
        assert_eq!(decide("Bash", &bash("git commit -m x"), &allow, &deny), None);
        assert!(matches!(decide("Bash", &bash("curl https://x"), &allow, &deny), Some(Decision::Deny { .. })));
    }

    #[test]
    fn some_actions_always_ask() {
        let everything = vec!["Bash:".to_string()];
        assert_eq!(decide("Bash", &bash("git push origin main"), &everything, &[]), None);
        assert_eq!(decide("Bash", &bash("rm -rf ../other"), &everything, &[]), None);
        assert_eq!(decide("Bash", &bash("rm -rf C:\\Users\\x"), &everything, &[]), None);
        assert_eq!(decide("Bash", &bash("rm build/out.txt"), &everything, &[]), Some(Decision::Allow));
    }

    #[test]
    fn never_allow_wins_over_always_allow() {
        let both = vec!["Bash:npm".to_string()];
        assert!(matches!(decide("Bash", &bash("npm install"), &both, &both), Some(Decision::Deny { .. })));
    }
}
