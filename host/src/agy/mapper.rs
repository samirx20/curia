//! `agy --output-format stream-json` → Curia's shared events. Pure. Lines (agy 1.5.2, checked
//! 2026-11-07): `init` (conversation id, model, tools), `step_update` (by step index: user_input,
//! agent_response with `text_delta` while ACTIVE, tool ACTIVE → DONE/ERROR), and `result`.
//! agy updates itself, so shapes are read tolerantly.
use crate::record::Entry;
use anyhow::{Context, Result};
use curia_core::{Event, ItemId, TextKind, TurnOutcome};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Default)]
pub struct Mapper {
    next_item: u64,
    in_turn: bool,
    /// Curia asked the turn to stop (agy has no interrupt: the process is closed).
    stopping: bool,
    conversation: Option<String>,
    /// Step index → (item, is a tool, text so far).
    steps: HashMap<i64, (ItemId, bool, String)>,
}

impl Mapper {
    pub fn starting_at(items_so_far: u64) -> Mapper {
        Mapper { next_item: items_so_far, ..Mapper::default() }
    }

    pub fn items_used(&self) -> u64 {
        self.next_item
    }

    pub fn in_turn(&self) -> bool {
        self.in_turn
    }

    pub fn stopping(&mut self) {
        self.stopping = true;
    }

    pub fn begin_turn(&mut self, text: &str) -> Vec<Event> {
        if self.in_turn {
            return Vec::new();
        }
        self.in_turn = true;
        vec![Event::TurnStarted { prompt: Some(text.to_string()), by: None }]
    }

    pub fn handle(&mut self, v: &Value) -> Vec<Event> {
        let mut events = Vec::new();
        match v["event"].as_str() {
            Some("init") => {
                if let Some(id) = v["conversation_id"].as_str().filter(|id| self.conversation.as_deref() != Some(*id)) {
                    self.conversation = Some(id.to_string());
                    let model = v["init"]["model"].as_str().map(str::to_string);
                    events.push(Event::SessionStarted { native_session_id: id.to_string(), model });
                }
            }
            Some("step_update") => self.step(&v["step_update"], &mut events),
            Some("result") => {
                let r = &v["result"];
                let outcome = if self.stopping || r["finished_reason"].as_str().is_some_and(|f| f.contains("cancel")) {
                    TurnOutcome::Interrupted
                } else if r["status"] == "SUCCESS" {
                    TurnOutcome::Completed
                } else {
                    let message = r["error"].as_str().unwrap_or("Antigravity's turn failed").to_string();
                    if message.to_lowercase().contains("quota") || message.to_lowercase().contains("rate limit") {
                        TurnOutcome::UsageLimited { resets_at_ms: None }
                    } else {
                        TurnOutcome::Failed { message }
                    }
                };
                self.finish(outcome, &mut events);
            }
            _ => {}
        }
        events
    }

    fn step(&mut self, s: &Value, events: &mut Vec<Event>) {
        let Some(index) = s["step_index"].as_i64() else { return };
        let state = s["state"].as_str().unwrap_or("");
        let done = state == "DONE" || state == "ERROR";
        match s["step_type"].as_str() {
            Some("agent_response") => {
                let delta = s["text_delta"].as_str().unwrap_or("");
                if !self.steps.contains_key(&index) && !delta.is_empty() {
                    let item = self.item();
                    events.push(Event::TextStarted { item: item.clone(), kind: TextKind::Text, text: String::new() });
                    self.steps.insert(index, (item, false, String::new()));
                }
                if let Some((item, _, text)) = self.steps.get_mut(&index) {
                    if !delta.is_empty() {
                        text.push_str(delta);
                        events.push(Event::TextDelta { item: item.clone(), text: delta.to_string() });
                    }
                    if done {
                        events.push(Event::TextDone { item: item.clone(), text: text.clone() });
                        self.steps.remove(&index);
                    }
                }
            }
            Some("tool") => {
                let info = &s["tool_info"];
                let name = info["name"].as_str().or(s["tool_name"].as_str()).unwrap_or("tool").to_string();
                if !self.steps.contains_key(&index) {
                    let item = self.item();
                    events.push(Event::ToolCall {
                        item: item.clone(), tool_use_id: format!("step-{index}"), name, input: info["parameters"].clone(), parent_tool_use_id: None,
                    });
                    self.steps.insert(index, (item, true, String::new()));
                }
                if done {
                    if let Some((item, _, _)) = self.steps.remove(&index) {
                        let error = info["error"]["message"].as_str().or(info["error"].as_str());
                        let output = error.or(info["output"].as_str()).unwrap_or("").to_string();
                        events.push(Event::ToolResult { item, output, is_error: state == "ERROR" || error.is_some() });
                    }
                }
            }
            Some("system_message") if done => {
                if let Some(t) = s["text"].as_str().or(s["text_delta"].as_str()).filter(|t| !t.is_empty()) {
                    events.push(Event::Notice { text: t.to_string() });
                }
            }
            _ => {}
        }
    }

    fn item(&mut self) -> ItemId {
        self.next_item += 1;
        ItemId(format!("item-{}", self.next_item))
    }

    fn finish(&mut self, outcome: TurnOutcome, events: &mut Vec<Event>) {
        let cut = !matches!(outcome, TurnOutcome::Completed);
        for (_, (item, tool, text)) in self.steps.drain() {
            if tool {
                events.push(Event::ToolResult { item, output: if cut { "Interrupted".into() } else { String::new() }, is_error: cut });
            } else {
                events.push(Event::TextDone { item, text });
            }
        }
        self.stopping = false;
        if self.in_turn {
            self.in_turn = false;
            events.push(Event::TurnFinished { outcome, cost_usd: None });
        }
    }

    pub fn on_exit(&mut self, code: Option<i32>, stderr_tail: &str) -> Vec<Event> {
        let mut events = Vec::new();
        if self.in_turn {
            let outcome = if self.stopping {
                TurnOutcome::Interrupted
            } else {
                let code = code.map_or("?".into(), |c| c.to_string());
                TurnOutcome::Failed { message: format!("Antigravity exited unexpectedly (code {code}). {stderr_tail}").trim().to_string() }
            };
            self.finish(outcome, &mut events);
        }
        events.push(Event::Exited { code });
        events
    }
}

/// Replays an agy recording as the session feeds it live.
pub fn replay(entries: &[Entry]) -> Result<Vec<Event>> {
    let mut m = Mapper::default();
    let mut events = Vec::new();
    for e in entries {
        match e {
            Entry::Start { items_so_far, .. } => m = Mapper::starting_at(*items_so_far),
            Entry::Out { line, .. } => {
                let v: Value = serde_json::from_str(line).with_context(|| format!("unreadable agy line: {line}"))?;
                events.extend(m.handle(&v));
            }
            Entry::In { line, .. } => {
                let v: Value = serde_json::from_str(line)?;
                if v["event"] == "user" {
                    events.extend(m.begin_turn(super::session::user_text(&v)));
                } else if v == super::session::stop_note() {
                    m.stopping();
                }
            }
            Entry::Exit { code, .. } => events.extend(m.on_exit(*code, "")),
            Entry::Err { .. } | Entry::Event { .. } => {}
        }
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn step(index: i64, kind: &str, state: &str, extra: Value) -> Value {
        let mut s = json!({"conversation_id": "c-1", "step_index": index, "step_type": kind, "state": state});
        for (k, v) in extra.as_object().unwrap() {
            s[k] = v.clone();
        }
        json!({"event": "step_update", "step_update": s})
    }

    #[test]
    fn a_turn_like_the_live_probe() {
        let mut m = Mapper::default();
        let mut events = m.begin_turn("make agy-hello.txt");
        for f in [
            json!({"event": "init", "conversation_id": "c-1", "init": {"model": "gemini-3.8-flash-low"}}),
            step(0, "user_input", "DONE", json!({})),
            step(1, "agent_response", "DONE", json!({})),
            step(2, "tool", "ACTIVE", json!({"tool_name": "write_file", "tool_info": {"name": "write_file", "parameters": {"Path": "C:/w/agy-hello.txt"}}})),
            step(2, "tool", "DONE", json!({"tool_info": {"name": "write_file", "parameters": {"Path": "C:/w/agy-hello.txt"}}})),
            step(3, "agent_response", "ACTIVE", json!({"text_delta": "I have"})),
            step(3, "agent_response", "ACTIVE", json!({"text_delta": " done it."})),
            step(3, "agent_response", "DONE", json!({})),
            json!({"event": "result", "result": {"conversation_id": "c-1", "status": "SUCCESS", "response": "I have done it."}}),
        ] {
            events.extend(m.handle(&f));
        }
        let (tool, text) = (ItemId("item-1".into()), ItemId("item-2".into()));
        assert_eq!(events, vec![
            Event::TurnStarted { prompt: Some("make agy-hello.txt".into()), by: None },
            Event::SessionStarted { native_session_id: "c-1".into(), model: Some("gemini-3.8-flash-low".into()) },
            Event::ToolCall { item: tool.clone(), tool_use_id: "step-2".into(), name: "write_file".into(), input: json!({"Path": "C:/w/agy-hello.txt"}), parent_tool_use_id: None },
            Event::ToolResult { item: tool, output: String::new(), is_error: false },
            Event::TextStarted { item: text.clone(), kind: TextKind::Text, text: String::new() },
            Event::TextDelta { item: text.clone(), text: "I have".into() },
            Event::TextDelta { item: text.clone(), text: " done it.".into() },
            Event::TextDone { item: text, text: "I have done it.".into() },
            Event::TurnFinished { outcome: TurnOutcome::Completed, cost_usd: None },
        ]);
    }

    #[test]
    fn a_stop_closes_the_process_and_the_turn_counts_as_interrupted() {
        let mut m = Mapper::default();
        m.begin_turn("long job");
        m.handle(&step(1, "tool", "ACTIVE", json!({"tool_info": {"name": "run_command", "parameters": {}}})));
        m.stopping();
        let events = m.on_exit(Some(1), "");
        assert!(matches!(&events[0], Event::ToolResult { is_error: true, .. }));
        assert_eq!(events[1], Event::TurnFinished { outcome: TurnOutcome::Interrupted, cost_usd: None });
    }
}
