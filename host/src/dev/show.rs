//! How the dev console prints events.
use curia_core::{Event, ItemId, TextKind};
use std::collections::HashMap;
use std::io::Write;

pub fn show(event: &Event) {
    match event {
        Event::TextDelta { text, .. } => {
            print!("{text}");
            let _ = std::io::stdout().flush();
        }
        Event::TextStarted { kind, text, .. } => print!("\n[{kind:?}] {text}"),
        Event::TextDone { .. } => println!(),
        Event::ToolCall { name, input, parent_tool_use_id, .. } => {
            let by = if parent_tool_use_id.is_some() { " (subagent)" } else { "" };
            println!("[tool{by}] {name} {input}");
        }
        Event::ToolResult { output, is_error, .. } => {
            let first = output.lines().next().unwrap_or("");
            println!("  → {}{first}", if *is_error { "error: " } else { "" });
        }
        Event::ApprovalRequested { kind, tool_name, input, .. } => {
            println!("[asks: {kind:?}] {tool_name} {input}\n  answer with /allow, /deny or /answer");
        }
        other => println!("[{}]", serde_json::to_string(other).unwrap_or_default()),
    }
}

/// Prints stored history: each text block as one line, labelled by the kind its start gave.
#[derive(Default)]
pub struct History(HashMap<ItemId, TextKind>);

impl History {
    pub fn show(&mut self, event: &Event) {
        match event {
            Event::TextStarted { item, kind, .. } => {
                self.0.insert(item.clone(), *kind);
            }
            Event::TextDone { item, text } => {
                let kind = self.0.remove(item).unwrap_or(TextKind::Text);
                println!("[{kind:?}] {text}");
            }
            other => show(other),
        }
    }
}
