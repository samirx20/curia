//! Making a live recording safe to commit as a test fixture (the repo is public).
//! Keeps only what the adapter reads, replaces the user's home folder, refuses to write a
//! result that still names it, and checks the scrubbed copy replays to the same events.
use crate::record::{self, Entry};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::path::Path;

/// System frames the Claude mapper reads. Others (`commands_changed`, `status`, …) list the
/// machine's skills, plugins and MCP servers, and are dropped.
const KEEP_SYSTEM: &[&str] = &["init", "task_started", "task_notification", "compact_boundary"];
const HOME_STANDIN: &str = r"C:\Users\user";

pub fn scrub_file(input: &Path, output: &Path) -> Result<usize> {
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).context("no home folder to scrub")?;
    let entries = scrub(record::read(input)?, &home)?;
    let text = entries.iter().map(serde_json::to_string).collect::<Result<Vec<_>, _>>()?.join("\n") + "\n";
    let user = Path::new(&home).file_name().and_then(|n| n.to_str()).unwrap_or("");
    for needle in [home.clone(), home.replace('\\', "/"), home.replace('\\', r"\\"), format!(r"\Users\{user}"), format!("/Users/{user}")] {
        if text.to_lowercase().contains(&needle.to_lowercase()) {
            bail!("the scrubbed recording still contains {needle:?}; not written");
        }
    }
    if let Some(email) = find_email(&text) {
        bail!("the scrubbed recording still contains an email address ({email}); not written");
    }
    let replayed = record::replay_claude(&entries)?;
    if replayed != record::recorded_events(&entries) {
        bail!("the scrubbed recording no longer replays to its recorded events; not written");
    }
    std::fs::write(output, text).with_context(|| format!("could not write {}", output.display()))?;
    Ok(entries.len())
}

fn scrub(entries: Vec<Entry>, home: &str) -> Result<Vec<Entry>> {
    let mut out = Vec::new();
    for e in entries {
        let e = match e {
            Entry::Err { .. } => continue,
            Entry::Out { ms, line } => {
                let v: Value = serde_json::from_str(&line)?;
                let Some(v) = keep(v) else { continue };
                Entry::Out { ms, line: clean(v, home).to_string() }
            }
            Entry::In { ms, line } => Entry::In { ms, line: clean(serde_json::from_str(&line)?, home).to_string() },
            Entry::Event { ms, event } => {
                Entry::Event { ms, event: serde_json::from_value(clean(serde_json::to_value(event)?, home))? }
            }
            kept @ (Entry::Exit { .. } | Entry::Start { .. }) => kept,
        };
        out.push(e);
    }
    Ok(out)
}

fn keep(v: Value) -> Option<Value> {
    // A successful control reply (to `initialize`) describes the signed-in account: email,
    // organization, settings files. The mapper only reads failed ones.
    if v["type"] == "control_response" {
        return (v["response"]["subtype"] == "error").then_some(v);
    }
    if v["type"] != "system" {
        return Some(v);
    }
    match v["subtype"].as_str() {
        Some("init") => Some(json!({"type": "system", "subtype": "init", "session_id": v["session_id"], "model": v["model"]})),
        Some(s) if KEEP_SYSTEM.contains(&s) => Some(v),
        _ => None,
    }
}

/// The first thing shaped like an email address (`name@domain.tld`), if any.
fn find_email(text: &str) -> Option<&str> {
    let is_part = |c: char| c.is_ascii_alphanumeric() || "._%+-".contains(c);
    for (at, _) in text.match_indices('@') {
        let start = text[..at].rfind(|c: char| !is_part(c)).map_or(0, |i| i + 1);
        let end = text[at + 1..].find(|c: char| !is_part(c)).map_or(text.len(), |i| at + 1 + i);
        let (name, domain) = (&text[start..at], text[at + 1..end].trim_end_matches('.'));
        let tld = domain.rsplit('.').next().unwrap_or("");
        if !name.is_empty() && domain.contains('.') && tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic()) {
            return Some(&text[start..end]);
        }
    }
    None
}

/// Replaces the home folder in every string, in both slash styles.
fn clean(v: Value, home: &str) -> Value {
    match v {
        Value::String(s) => Value::String(
            s.replace(home, HOME_STANDIN).replace(&home.replace('\\', "/"), &HOME_STANDIN.replace('\\', "/")),
        ),
        Value::Array(a) => Value::Array(a.into_iter().map(|x| clean(x, home)).collect()),
        Value::Object(o) => Value::Object(o.into_iter().map(|(k, x)| (k, clean(x, home))).collect()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_paths_are_replaced_in_both_slash_styles() {
        let v = json!({"a": r"C:\Users\Sam\x", "b": ["C:/Users/Sam/y"]});
        assert_eq!(clean(v, r"C:\Users\Sam"), json!({"a": r"C:\Users\user\x", "b": ["C:/Users/user/y"]}));
    }

    #[test]
    fn emails_are_found_and_package_names_are_not() {
        assert_eq!(find_email(r#"{"email":"someone@example.com"}"#), Some("someone@example.com"));
        assert_eq!(find_email("npm i @types/node and user@host"), None);
        let reply = json!({"type": "control_response", "response": {"subtype": "success", "response": {"account": {}}}});
        assert!(keep(reply).is_none(), "the initialize reply describes the account");
    }

    #[test]
    fn status_frames_are_dropped_and_init_cut_down() {
        assert!(keep(json!({"type": "system", "subtype": "commands_changed", "commands": []})).is_none());
        let init = keep(json!({"type": "system", "subtype": "init", "session_id": "s", "model": "m", "skills": ["x"]}));
        assert_eq!(init, Some(json!({"type": "system", "subtype": "init", "session_id": "s", "model": "m"})));
    }
}
