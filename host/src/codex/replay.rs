//! Replaying a Codex recording through the mapper, the way `CodexSession` feeds it live.
use super::mapper::Mapper;
use crate::record::Entry;
use anyhow::{Context, Result};
use curia_core::{Decision, Event};
use serde_json::{Value, json};

pub fn replay(entries: &[Entry]) -> Result<Vec<Event>> {
    let mut m = Mapper::starting_at(0);
    let mut events = Vec::new();
    for e in entries {
        match e {
            Entry::Start { items_so_far, .. } => m = Mapper::starting_at(*items_so_far),
            Entry::Out { line, .. } => {
                let v: Value = serde_json::from_str(line).with_context(|| format!("unreadable Codex line: {line}"))?;
                if v.get("method").is_some() {
                    events.extend(m.handle(&v).events);
                } else if let Some(id) = v["result"]["thread"]["id"].as_str() {
                    // The reply to thread/start or thread/resume announces the session, as live.
                    let model = v["result"]["model"].clone();
                    events.extend(m.handle(&json!({"method": "thread/started", "params": {"thread": {"id": id}, "model": model}})).events);
                }
            }
            Entry::In { line, .. } => {
                let v: Value = serde_json::from_str(line)?;
                let text = || v["params"]["input"][0]["text"].as_str().unwrap_or("").to_string();
                match v["method"].as_str() {
                    Some("turn/start") => events.extend(m.begin_turn(&text(), false)),
                    Some("turn/steer") => events.extend(m.begin_turn(&text(), true)),
                    None if v["result"].is_object() => {
                        let r = &v["result"];
                        let decision = if r.get("answers").is_some() {
                            Decision::Answer { answers: Default::default() }
                        } else if r["decision"] == "accept" || r["decision"] == "acceptForSession" || r["action"] == "accept" {
                            Decision::Allow
                        } else {
                            Decision::Deny { message: None }
                        };
                        let (_, event) = m.resolve(&v["id"].to_string(), &decision)?;
                        events.push(event);
                    }
                    _ => {}
                }
            }
            Entry::Exit { code, .. } => events.extend(m.on_exit(*code, "")),
            Entry::Err { .. } | Entry::Event { .. } => {}
        }
    }
    Ok(events)
}
