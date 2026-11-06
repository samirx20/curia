//! Claude stream-json frames → Curia's shared events. Pure: no I/O, so recorded streams
//! replay through exactly the code that handles live ones.
//! Rules from Agent Hub's parked Rust app (`chat/mapper.rs`, `chat/turn.rs`, `chat/session.rs`).
mod stream;
mod usage;

use anyhow::{Result, bail};
use curia_core::{Decision, Event, ItemId, RequestKind, Resolution, TurnOutcome};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet, VecDeque};

use super::wire;

/// What one frame produced: events for the thread, and lines to write back to the CLI.
#[derive(Debug, Default)]
pub struct Step {
    pub events: Vec<Event>,
    pub replies: Vec<Value>,
}

/// A content block still streaming, keyed by message id and block index.
enum Block {
    Text { item: ItemId, text: String },
    Tool { item: ItemId, tool_use_id: String, name: String, json: String },
}

struct Pending {
    kind: RequestKind,
    input: Value,
}

#[derive(Default)]
pub struct Mapper {
    next_item: u64,
    in_turn: bool,
    session_id: Option<String>,
    cur_msg: String,
    blocks: HashMap<String, Block>,
    /// Messages whose text arrived as stream events (their `assistant` copy is skipped).
    streamed: HashSet<String>,
    /// tool_use_id → (item, has a result).
    tools: HashMap<String, (ItemId, bool)>,
    requests: HashMap<String, Pending>,
    /// Running subagents by task id (background shell tasks are not tracked).
    subagents: HashSet<String>,
    /// Messages written while a turn was running. Claude doesn't fold them into that turn;
    /// it answers each with a turn of its own afterwards (checked on 2.1.318).
    queued_prompts: VecDeque<String>,
}

impl Mapper {
    pub fn new() -> Mapper {
        Mapper::default()
    }

    /// A mapper whose item ids continue after `items_so_far`, so a thread resumed in a new
    /// process never reuses an id.
    pub fn starting_at(items_so_far: u64) -> Mapper {
        Mapper { next_item: items_so_far, ..Mapper::default() }
    }

    /// How many item ids this mapper has handed out, counting from its start.
    pub fn items_used(&self) -> u64 {
        self.next_item
    }

    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    #[cfg(test)]
    pub fn in_turn(&self) -> bool {
        self.in_turn
    }

    fn item(&mut self) -> ItemId {
        self.next_item += 1;
        ItemId(format!("item-{}", self.next_item))
    }

    /// A user message was written to the CLI.
    pub fn begin_turn(&mut self, text: &str) -> Vec<Event> {
        if self.in_turn {
            self.queued_prompts.push_back(text.to_string()); // its turn starts after this one
            return Vec::new();
        }
        self.in_turn = true;
        vec![Event::TurnStarted { prompt: Some(text.to_string()) }]
    }

    pub fn handle(&mut self, v: &Value) -> Step {
        let mut out = Step::default();
        match v["type"].as_str().unwrap_or("") {
            "system" => self.system(v, &mut out),
            "stream_event" if v["parent_tool_use_id"].is_null() => self.stream_event(&v["event"], &mut out),
            "assistant" => self.assistant(v, &mut out),
            "user" => self.tool_results(v, &mut out),
            "control_request" => self.control_request(v, &mut out),
            "control_response" if v["response"]["subtype"] == "error" => {
                let error = v["response"]["error"].as_str().unwrap_or("a control request failed");
                out.events.push(Event::Notice { text: format!("Claude refused a request: {error}") });
            }
            "rate_limit_event" => out.events.push(usage::rate_limit(v)),
            "result" => {
                let outcome = usage::outcome(v);
                self.finish(outcome, v["total_cost_usd"].as_f64(), &mut out.events);
            }
            _ => {}
        }
        out
    }

    fn system(&mut self, v: &Value, out: &mut Step) {
        match v["subtype"].as_str() {
            // Claude sends `init` at the start of every turn (checked on 2.1.318).
            Some("init") => {
                if let Some(id) = v["session_id"].as_str().filter(|id| self.session_id.as_deref() != Some(*id)) {
                    self.session_id = Some(id.to_string());
                    let model = v["model"].as_str().map(str::to_string);
                    out.events.push(Event::SessionStarted { native_session_id: id.to_string(), model });
                }
                if !self.in_turn {
                    // A turn we didn't start: a message written mid-turn, or Claude waking
                    // itself (a background task finished).
                    self.in_turn = true;
                    out.events.push(Event::TurnStarted { prompt: self.queued_prompts.pop_front() });
                }
            }
            Some("compact_boundary") => out.events.push(Event::Notice { text: "Context compacted".into() }),
            // `local_agent` is a subagent; `local_bash` is a background shell command.
            Some("task_started") if v["task_type"] == "local_agent" => {
                let task_id = v["task_id"].as_str().unwrap_or("").to_string();
                self.subagents.insert(task_id.clone());
                out.events.push(Event::SubagentStarted {
                    task_id,
                    tool_use_id: v["tool_use_id"].as_str().map(str::to_string),
                    description: v["description"].as_str().unwrap_or("").to_string(),
                });
            }
            Some("task_notification") => {
                let task_id = v["task_id"].as_str().unwrap_or("").to_string();
                if self.subagents.remove(&task_id) {
                    let status = v["status"].as_str().unwrap_or("").to_string();
                    out.events.push(Event::SubagentEnded { task_id, status });
                }
            }
            _ => {}
        }
    }

    fn control_request(&mut self, v: &Value, out: &mut Step) {
        let request_id = v["request_id"].as_str().unwrap_or("").to_string();
        let req = &v["request"];
        if req["subtype"] != "can_use_tool" {
            out.replies.push(wire::response_error(&request_id, "Not supported by Curia"));
            return;
        }
        let tool_name = req["tool_name"].as_str().unwrap_or("?").to_string();
        let kind = match tool_name.as_str() {
            "AskUserQuestion" => RequestKind::Question,
            "ExitPlanMode" => RequestKind::Plan,
            _ => RequestKind::Permission,
        };
        let input = req["input"].clone();
        self.requests.insert(request_id.clone(), Pending { kind, input: input.clone() });
        out.events.push(Event::ApprovalRequested { request_id, kind, tool_name, input });
    }

    /// Answers a pending request: the line to write to the CLI and the event to record.
    pub fn resolve(&mut self, request_id: &str, decision: &Decision) -> Result<(Value, Event)> {
        let Some(p) = self.requests.remove(request_id) else {
            bail!("request {request_id} is no longer pending");
        };
        let (body, resolution) = match decision {
            Decision::Answer { answers } if p.kind == RequestKind::Question => (
                json!({"behavior": "allow", "updatedInput": {"questions": p.input["questions"], "answers": answers}}),
                Resolution::Answered,
            ),
            Decision::Answer { .. } => bail!("only a question can be answered with answers"),
            Decision::Allow => (json!({"behavior": "allow", "updatedInput": p.input}), Resolution::Allowed),
            Decision::Deny { message } => {
                let message = message.clone().filter(|m| !m.trim().is_empty());
                let message = message.unwrap_or_else(|| "The user declined this.".into());
                (json!({"behavior": "deny", "message": message}), Resolution::Denied)
            }
        };
        let event = Event::ApprovalResolved { request_id: request_id.to_string(), resolution };
        Ok((wire::response_success(request_id, body), event))
    }

    /// Ends the turn: open blocks are closed, unfinished tools and pending requests settled.
    fn finish(&mut self, outcome: TurnOutcome, cost_usd: Option<f64>, events: &mut Vec<Event>) {
        for (_, block) in self.blocks.drain() {
            if let Block::Text { item, text, .. } = block {
                events.push(Event::TextDone { item, text });
            }
        }
        let cut = !matches!(outcome, TurnOutcome::Completed);
        for (item, has_result) in self.tools.values_mut().filter(|(_, done)| !*done) {
            *has_result = true;
            let output = if cut { "Interrupted" } else { "" }.to_string();
            events.push(Event::ToolResult { item: item.clone(), output, is_error: cut });
        }
        for (request_id, _) in self.requests.drain() {
            events.push(Event::ApprovalResolved { request_id, resolution: Resolution::Cancelled });
        }
        self.streamed.clear();
        self.tools.clear();
        if self.in_turn {
            self.in_turn = false;
            events.push(Event::TurnFinished { outcome, cost_usd });
        }
    }

    /// The process ended. A turn still running fails with the tail of stderr.
    pub fn on_exit(&mut self, code: Option<i32>, stderr_tail: &str) -> Vec<Event> {
        let mut events = Vec::new();
        if self.in_turn {
            let code = code.map_or("?".into(), |c| c.to_string());
            let message = format!("Claude exited unexpectedly (code {code}). {stderr_tail}").trim().to_string();
            self.finish(TurnOutcome::Failed { message }, None, &mut events);
        }
        events.push(Event::Exited { code });
        events
    }
}

#[cfg(test)]
mod tests;
