//! Codex app-server messages → Curia's shared events. Pure, like Claude's mapper: recorded
//! streams replay through exactly this code. Responses to Curia's own requests are handled by
//! the session; this sees notifications and Codex's requests (approvals, questions).
mod items;

use super::wire;
use anyhow::{Result, bail};
use curia_core::{Decision, Event, ItemId, RequestKind, Resolution, TurnOutcome, UsageWindow};
use serde_json::{Map, Value, json};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Default)]
pub struct Step {
    pub events: Vec<Event>,
    pub replies: Vec<Value>,
}

struct Pending {
    /// Codex's JSON-RPC id for the request (echoed in the reply).
    id: Value,
    kind: RequestKind,
    input: Value,
    /// An MCP tool approval, sent as an elicitation: answered with `action`, not `decision`.
    elicitation: bool,
}

#[derive(Default)]
pub struct Mapper {
    next_item: u64,
    in_turn: bool,
    thread_id: Option<String>,
    turn_id: Option<String>,
    /// Codex item id → (Curia item id, is a tool, has finished).
    items: HashMap<String, (ItemId, bool, bool)>,
    requests: HashMap<String, Pending>,
    queued_prompts: VecDeque<String>,
}

impl Mapper {
    #[cfg(test)]
    pub fn new() -> Mapper {
        Mapper::default()
    }

    pub fn starting_at(items_so_far: u64) -> Mapper {
        Mapper { next_item: items_so_far, ..Mapper::default() }
    }

    pub fn items_used(&self) -> u64 {
        self.next_item
    }

    /// The running turn's id, while one runs (needed to steer or interrupt it).
    pub fn running_turn(&self) -> Option<&str> {
        self.turn_id.as_deref().filter(|_| self.in_turn)
    }

    fn item(&mut self, native: &str, tool: bool) -> ItemId {
        if let Some((id, _, _)) = self.items.get(native) {
            return id.clone();
        }
        self.next_item += 1;
        let id = ItemId(format!("item-{}", self.next_item));
        self.items.insert(native.to_string(), (id.clone(), tool, false));
        id
    }

    /// A user message was sent: a new turn, or (when one runs) steered into it.
    pub fn begin_turn(&mut self, text: &str, steered: bool) -> Vec<Event> {
        if self.in_turn {
            if !steered {
                self.queued_prompts.push_back(text.to_string());
            }
            return Vec::new();
        }
        self.in_turn = true;
        vec![Event::TurnStarted { prompt: Some(text.to_string()), by: None }]
    }

    pub fn handle(&mut self, v: &Value) -> Step {
        let mut out = Step::default();
        let Some(method) = v["method"].as_str() else { return out };
        let p = &v["params"];
        if v.get("id").is_some() {
            self.request(method, &v["id"], p, &mut out);
            return out;
        }
        match method {
            "thread/started" => {
                if let Some(id) = p["thread"]["id"].as_str().filter(|id| self.thread_id.as_deref() != Some(*id)) {
                    self.thread_id = Some(id.to_string());
                    let model = p["model"].as_str().map(str::to_string);
                    out.events.push(Event::SessionStarted { native_session_id: id.to_string(), model });
                }
            }
            "turn/started" => {
                self.turn_id = p["turn"]["id"].as_str().map(str::to_string);
                if !self.in_turn {
                    self.in_turn = true;
                    out.events.push(Event::TurnStarted { prompt: self.queued_prompts.pop_front(), by: None });
                }
            }
            "item/started" | "item/completed" | "item/agentMessage/delta" | "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => {
                self.item_event(method, p, &mut out.events)
            }
            "account/rateLimits/updated" => out.events.push(rate_limits(&p["rateLimits"])),
            "error" => out.events.push(Event::Notice { text: format!("Codex: {}", p["error"]["message"].as_str().unwrap_or("an error")) }),
            "serverRequest/resolved" => {
                // Codex settled a request itself (its turn ended, say): it can't be answered now.
                let key = p["requestId"].to_string();
                if self.requests.remove(&key).is_some() {
                    out.events.push(Event::ApprovalResolved { request_id: key, resolution: Resolution::Cancelled });
                }
            }
            "turn/completed" => {
                let turn = &p["turn"];
                let outcome = match turn["status"].as_str() {
                    Some("completed") => TurnOutcome::Completed,
                    Some("interrupted") => TurnOutcome::Interrupted,
                    _ => {
                        let message = turn["error"]["message"].as_str().unwrap_or("Codex's turn failed").to_string();
                        if message.to_lowercase().contains("usage limit") || message.to_lowercase().contains("rate limit") {
                            TurnOutcome::UsageLimited { resets_at_ms: None }
                        } else {
                            TurnOutcome::Failed { message }
                        }
                    }
                };
                self.finish(outcome, &mut out.events);
            }
            _ => {}
        }
        out
    }

    fn request(&mut self, method: &str, id: &Value, p: &Value, out: &mut Step) {
        let (kind, tool, input) = match method {
            "item/commandExecution/requestApproval" => {
                (RequestKind::Permission, "Shell", json!({"command": p["command"], "cwd": p["cwd"], "reason": p["reason"]}))
            }
            "item/fileChange/requestApproval" => (RequestKind::Permission, "Edit", json!({"reason": p["reason"], "grant_root": p["grantRoot"]})),
            "item/permissions/requestApproval" => (RequestKind::Permission, "Permissions", p.clone()),
            // Codex asks before an MCP tool runs (unless the server's tools are pre-approved,
            // as Curia's control room is) as a form elicitation with no fields.
            "mcpServer/elicitation/request" if p["_meta"]["codex_approval_kind"] == "mcp_tool_call" => {
                let name = format!("mcp__{}__{}", p["serverName"].as_str().unwrap_or("?"), p["_meta"]["tool_name"].as_str().unwrap_or("?"));
                let request_id = id.to_string();
                let input = p["_meta"]["tool_params"].clone();
                self.requests.insert(request_id.clone(), Pending { id: id.clone(), kind: RequestKind::Permission, input: input.clone(), elicitation: true });
                out.events.push(Event::ApprovalRequested { request_id, kind: RequestKind::Permission, tool_name: name, input });
                return;
            }
            "item/tool/requestUserInput" => {
                let questions: Vec<Value> = p["questions"].as_array().into_iter().flatten().map(|q| json!({
                    "id": q["id"], "question": q["question"],
                    "options": q["options"].as_array().into_iter().flatten().map(|o| json!({"label": o["label"]})).collect::<Vec<_>>(),
                })).collect();
                (RequestKind::Question, "AskUserQuestion", json!({"questions": questions}))
            }
            _ => {
                out.replies.push(wire::refuse(id, "Not supported by Curia"));
                return;
            }
        };
        let request_id = id.to_string();
        self.requests.insert(request_id.clone(), Pending { id: id.clone(), kind, input: input.clone(), elicitation: false });
        out.events.push(Event::ApprovalRequested { request_id, kind, tool_name: tool.into(), input });
    }

    /// Answers a pending request: the line to write and the event to record.
    pub fn resolve(&mut self, request_id: &str, decision: &Decision) -> Result<(Value, Event)> {
        let Some(p) = self.requests.remove(request_id) else { bail!("request {request_id} is no longer pending") };
        let (result, resolution) = match (p.kind, decision) {
            (RequestKind::Question, Decision::Answer { answers }) => (json!({"answers": codex_answers(&p.input, answers)}), Resolution::Answered),
            (RequestKind::Question, Decision::Deny { .. }) => (json!({"answers": {}}), Resolution::Denied),
            (RequestKind::Question, Decision::Allow) => (json!({"answers": {}}), Resolution::Answered),
            (_, Decision::Answer { .. }) => bail!("only a question can be answered with answers"),
            (_, Decision::Allow) if p.elicitation => (json!({"action": "accept", "content": {}}), Resolution::Allowed),
            (_, Decision::Deny { .. }) if p.elicitation => (json!({"action": "decline"}), Resolution::Denied),
            (_, Decision::Allow) => (json!({"decision": "accept"}), Resolution::Allowed),
            (_, Decision::Deny { .. }) => (json!({"decision": "decline"}), Resolution::Denied),
        };
        Ok((wire::reply(&p.id, result), Event::ApprovalResolved { request_id: request_id.to_string(), resolution }))
    }

    /// Ends the turn: unfinished tools and text are settled, pending requests cancelled.
    fn finish(&mut self, outcome: TurnOutcome, events: &mut Vec<Event>) {
        let cut = !matches!(outcome, TurnOutcome::Completed);
        for (id, tool, done) in self.items.values_mut().filter(|(_, _, done)| !*done) {
            *done = true;
            if *tool {
                let output = if cut { "Interrupted" } else { "" }.to_string();
                events.push(Event::ToolResult { item: id.clone(), output, is_error: cut });
            }
        }
        for (request_id, _) in self.requests.drain() {
            events.push(Event::ApprovalResolved { request_id, resolution: Resolution::Cancelled });
        }
        self.items.clear();
        if self.in_turn {
            self.in_turn = false;
            events.push(Event::TurnFinished { outcome, cost_usd: None });
        }
    }

    pub fn on_exit(&mut self, code: Option<i32>, stderr_tail: &str) -> Vec<Event> {
        let mut events = Vec::new();
        if self.in_turn {
            let code = code.map_or("?".into(), |c| c.to_string());
            let message = format!("Codex exited unexpectedly (code {code}). {stderr_tail}").trim().to_string();
            self.finish(TurnOutcome::Failed { message }, &mut events);
        }
        events.push(Event::Exited { code });
        events
    }
}

/// Curia's answers (keyed by question text or id) as Codex wants them: id → {answers: [...]}.
fn codex_answers(input: &Value, answers: &Map<String, Value>) -> Value {
    let mut out = Map::new();
    for q in input["questions"].as_array().into_iter().flatten() {
        let id = q["id"].as_str().unwrap_or("");
        let given = answers.get(id).or_else(|| answers.get(q["question"].as_str().unwrap_or("")));
        if let Some(a) = given {
            let list = match a {
                Value::Array(xs) => xs.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect::<Vec<_>>(),
                other => vec![other.as_str().map(str::to_string).unwrap_or_else(|| other.to_string())],
            };
            out.insert(id.to_string(), json!({"answers": list}));
        }
    }
    Value::Object(out)
}

/// A rate-limit snapshot as usage windows. Codex names windows by length, so they're named
/// `<hours>h`, except the 5-hour window, which the quota picker knows as `five_hour`.
pub fn rate_limits(snapshot: &Value) -> Event {
    let mut windows = Vec::new();
    for slot in ["primary", "secondary"] {
        let w = &snapshot[slot];
        let Some(used) = w["usedPercent"].as_f64() else { continue };
        let mins = w["windowDurationMins"].as_i64().unwrap_or(0);
        let name = if mins == 300 { "five_hour".to_string() } else { format!("{}h", mins / 60) };
        windows.push(UsageWindow { name, utilization: Some(used / 100.0), resets_at_ms: w["resetsAt"].as_i64().map(|s| s * 1000) });
    }
    Event::Usage { windows, limited: !snapshot["rateLimitReachedType"].is_null() }
}

#[cfg(test)]
mod tests;
