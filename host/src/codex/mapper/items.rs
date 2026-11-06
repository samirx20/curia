//! Codex items (messages, reasoning, commands, file changes, tools) as shared events.
use super::Mapper;
use curia_core::{Event, TextKind};
use serde_json::{Value, json};

const MAX_TOOL_OUTPUT: usize = 64 * 1024;

impl Mapper {
    pub(super) fn item_event(&mut self, method: &str, p: &Value, events: &mut Vec<Event>) {
        match method {
            "item/agentMessage/delta" | "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => {
                let native = p["itemId"].as_str().unwrap_or("");
                if let Some((id, _, _)) = self.items.get(native) {
                    events.push(Event::TextDelta { item: id.clone(), text: p["delta"].as_str().unwrap_or("").to_string() });
                }
            }
            "item/started" => self.started(&p["item"], events),
            "item/completed" => self.completed(&p["item"], events),
            _ => {}
        }
    }

    fn started(&mut self, item: &Value, events: &mut Vec<Event>) {
        let native = item["id"].as_str().unwrap_or("").to_string();
        let tool = |name: &str, input: Value| Some((name.to_string(), input));
        let call = match item["type"].as_str() {
            Some("agentMessage") | Some("reasoning") => {
                let kind = if item["type"] == "reasoning" { TextKind::Thinking } else { TextKind::Text };
                let id = self.item(&native, false);
                events.push(Event::TextStarted { item: id, kind, text: String::new() });
                None
            }
            Some("commandExecution") => tool("Shell", json!({"command": item["command"], "cwd": item["cwd"]})),
            Some("fileChange") => tool("Edit", json!({"changes": item["changes"]})),
            Some("mcpToolCall") => tool(
                &format!("mcp__{}__{}", item["server"].as_str().unwrap_or("?"), item["tool"].as_str().unwrap_or("?")),
                item["arguments"].clone(),
            ),
            Some("dynamicToolCall") => tool(item["tool"].as_str().unwrap_or("tool"), item["arguments"].clone()),
            Some("webSearch") => tool("WebSearch", json!({"query": item["query"]})),
            _ => None,
        };
        if let Some((name, input)) = call {
            let id = self.item(&native, true);
            events.push(Event::ToolCall { item: id, tool_use_id: native, name, input, parent_tool_use_id: None });
        }
    }

    fn completed(&mut self, item: &Value, events: &mut Vec<Event>) {
        let native = item["id"].as_str().unwrap_or("");
        let Some((id, tool, done)) = self.items.get_mut(native) else { return };
        if *done {
            return;
        }
        *done = true;
        let id = id.clone();
        if !*tool {
            let text = match item["type"].as_str() {
                Some("reasoning") => join(&item["summary"]).or_else(|| join(&item["content"])).unwrap_or_default(),
                _ => item["text"].as_str().unwrap_or("").to_string(),
            };
            events.push(Event::TextDone { item: id, text });
            return;
        }
        let status = item["status"].as_str().unwrap_or("completed");
        let failed = matches!(status, "failed" | "declined") || item["exitCode"].as_i64().is_some_and(|c| c != 0) || !item["error"].is_null();
        let output = match item["type"].as_str() {
            Some("commandExecution") => item["aggregatedOutput"].as_str().unwrap_or("").to_string(),
            Some("fileChange") => item["changes"].as_array().into_iter().flatten()
                .map(|c| c["path"].as_str().unwrap_or("?").to_string()).collect::<Vec<_>>().join("\n"),
            Some("mcpToolCall") => if item["error"].is_null() { item["result"].to_string() } else { item["error"].to_string() },
            _ => item["result"].to_string(),
        };
        events.push(Event::ToolResult { item: id, output: cap(output), is_error: failed });
    }
}

/// A list of text parts (a reasoning summary) as one string.
fn join(parts: &Value) -> Option<String> {
    let texts: Vec<&str> = parts.as_array()?.iter().filter_map(|p| p.as_str().or(p["text"].as_str())).collect();
    (!texts.is_empty()).then(|| texts.join("\n"))
}

fn cap(text: String) -> String {
    if text.len() <= MAX_TOOL_OUTPUT {
        return text;
    }
    let mut cut = MAX_TOOL_OUTPUT;
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}\n… (cut at 64 KB)", &text[..cut])
}
