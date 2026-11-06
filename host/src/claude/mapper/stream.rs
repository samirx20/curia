//! Streamed content blocks, whole assistant messages, and tool results.
use super::{Block, Mapper, Step};
use curia_core::{Event, TextKind};
use serde_json::{Value, json};

/// Tool output kept per result. Larger output is cut (the CLI's transcript has all of it).
const MAX_TOOL_OUTPUT: usize = 64 * 1024;

impl Mapper {
    /// A top-level `stream_event` (subagent frames are not streamed into the thread).
    pub(super) fn stream_event(&mut self, ev: &Value, out: &mut Step) {
        let key = format!("{}:{}", self.cur_msg, ev["index"]);
        match ev["type"].as_str().unwrap_or("") {
            "message_start" => self.cur_msg = ev["message"]["id"].as_str().unwrap_or("").to_string(),
            "content_block_start" => {
                let cb = &ev["content_block"];
                let item = self.item();
                let block = match cb["type"].as_str() {
                    Some(t @ ("text" | "thinking")) => {
                        let kind = if t == "text" { TextKind::Text } else { TextKind::Thinking };
                        let text = cb[t].as_str().unwrap_or("").to_string();
                        out.events.push(Event::TextStarted { item: item.clone(), kind, text: text.clone() });
                        Block::Text { item, text }
                    }
                    Some("tool_use") => Block::Tool {
                        item,
                        tool_use_id: cb["id"].as_str().unwrap_or("").to_string(),
                        name: cb["name"].as_str().unwrap_or("?").to_string(),
                        json: String::new(),
                    },
                    _ => return,
                };
                self.streamed.insert(self.cur_msg.clone());
                self.blocks.insert(key, block);
            }
            "content_block_delta" => {
                let d = &ev["delta"];
                match (self.blocks.get_mut(&key), d["type"].as_str()) {
                    (Some(Block::Text { item, text, .. }), Some("text_delta" | "thinking_delta")) => {
                        let piece = d["text"].as_str().or(d["thinking"].as_str()).unwrap_or("");
                        text.push_str(piece);
                        out.events.push(Event::TextDelta { item: item.clone(), text: piece.to_string() });
                    }
                    (Some(Block::Tool { json, .. }), Some("input_json_delta")) => {
                        json.push_str(d["partial_json"].as_str().unwrap_or(""));
                    }
                    _ => {}
                }
            }
            "content_block_stop" => match self.blocks.remove(&key) {
                Some(Block::Text { item, text, .. }) => out.events.push(Event::TextDone { item, text }),
                Some(Block::Tool { item, tool_use_id, name, json }) => {
                    let input = serde_json::from_str(&json).unwrap_or(json!({}));
                    self.tools.insert(tool_use_id.clone(), (item.clone(), false));
                    out.events.push(Event::ToolCall { item, tool_use_id, name, input, parent_tool_use_id: None });
                }
                None => {}
            },
            _ => {}
        }
    }

    /// A whole assistant message. Streamed parts were already handled; this adds subagent
    /// tool calls and anything that didn't stream.
    pub(super) fn assistant(&mut self, v: &Value, out: &mut Step) {
        let parent = v["parent_tool_use_id"].as_str().map(str::to_string);
        let msg = v["message"]["id"].as_str().unwrap_or("").to_string();
        for b in v["message"]["content"].as_array().into_iter().flatten() {
            match b["type"].as_str() {
                Some("tool_use") => {
                    let id = b["id"].as_str().unwrap_or("").to_string();
                    let streaming = self.blocks.values().any(|bl| matches!(bl, Block::Tool { tool_use_id, .. } if *tool_use_id == id));
                    if self.tools.contains_key(&id) || streaming {
                        continue;
                    }
                    let item = self.item();
                    self.tools.insert(id.clone(), (item.clone(), false));
                    out.events.push(Event::ToolCall {
                        item,
                        tool_use_id: id,
                        name: b["name"].as_str().unwrap_or("?").to_string(),
                        input: b["input"].clone(),
                        parent_tool_use_id: parent.clone(),
                    });
                }
                Some(t @ ("text" | "thinking")) if parent.is_none() && !self.streamed.contains(&msg) => {
                    let kind = if t == "text" { TextKind::Text } else { TextKind::Thinking };
                    let text = b[t].as_str().unwrap_or("").to_string();
                    let item = self.item();
                    out.events.push(Event::TextStarted { item: item.clone(), kind, text: String::new() });
                    out.events.push(Event::TextDone { item, text });
                }
                _ => {}
            }
        }
    }

    /// `user` frames carry tool results (the CLI echoes them back as the user's turn).
    pub(super) fn tool_results(&mut self, v: &Value, out: &mut Step) {
        let blocks = v["message"]["content"].as_array().into_iter().flatten();
        for b in blocks.filter(|b| b["type"] == "tool_result") {
            let Some((item, done)) = self.tools.get_mut(b["tool_use_id"].as_str().unwrap_or("")) else { continue };
            *done = true;
            out.events.push(Event::ToolResult {
                item: item.clone(),
                output: result_text(&b["content"]),
                is_error: b["is_error"].as_bool().unwrap_or(false),
            });
        }
    }
}

fn result_text(content: &Value) -> String {
    let text = match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    };
    if text.len() <= MAX_TOOL_OUTPUT {
        return text;
    }
    let mut cut = MAX_TOOL_OUTPUT;
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}\n… (cut at 64 KB)", &text[..cut])
}
