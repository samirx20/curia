//! Frames shaped like the ones codex 0.167.0 sent in the live probe (2026-11-07).
use super::*;
use curia_core::TextKind;
use serde_json::json;

fn note(method: &str, params: Value) -> Value {
    json!({"method": method, "params": params})
}

fn feed(m: &mut Mapper, frames: &[Value]) -> Step {
    let mut all = Step::default();
    for f in frames {
        let s = m.handle(f);
        all.events.extend(s.events);
        all.replies.extend(s.replies);
    }
    all
}

#[test]
fn a_turn_with_a_command_approval_and_a_reply() {
    let mut m = Mapper::new();
    let mut events = m.begin_turn("make a folder", false);
    let step = feed(&mut m, &[
        note("thread/started", json!({"thread": {"id": "th-1"}})),
        note("turn/started", json!({"threadId": "th-1", "turn": {"id": "tu-1", "status": "inProgress"}})),
        note("item/started", json!({"item": {"type": "commandExecution", "id": "exec-1", "command": "mkdir x", "cwd": "C:\\w", "status": "inProgress"}})),
        json!({"id": 0, "method": "item/commandExecution/requestApproval", "params": {"itemId": "exec-1", "command": "mkdir x", "cwd": "C:\\w", "threadId": "th-1", "turnId": "tu-1", "startedAtMs": 1}}),
    ]);
    events.extend(step.events);
    assert_eq!(m.running_turn(), Some("tu-1"));
    assert!(matches!(&events[1], Event::SessionStarted { native_session_id, .. } if native_session_id == "th-1"));
    assert!(matches!(&events[2], Event::ToolCall { name, .. } if name == "Shell"));
    let Event::ApprovalRequested { request_id, tool_name, .. } = &events[3] else { panic!("{events:?}") };
    assert_eq!(tool_name, "Shell");

    let (reply, resolved) = m.resolve(request_id, &Decision::Allow).unwrap();
    assert_eq!(reply, json!({"id": 0, "result": {"decision": "accept"}}));
    assert_eq!(resolved, Event::ApprovalResolved { request_id: request_id.clone(), resolution: Resolution::Allowed });

    let step = feed(&mut m, &[
        note("item/completed", json!({"item": {"type": "commandExecution", "id": "exec-1", "status": "completed", "exitCode": 0, "aggregatedOutput": "ok"}})),
        note("item/started", json!({"item": {"type": "agentMessage", "id": "msg-1", "text": ""}})),
        note("item/agentMessage/delta", json!({"itemId": "msg-1", "delta": "Do"})),
        note("item/agentMessage/delta", json!({"itemId": "msg-1", "delta": "ne."})),
        note("item/completed", json!({"item": {"type": "agentMessage", "id": "msg-1", "text": "Done."}})),
        note("turn/completed", json!({"threadId": "th-1", "turn": {"id": "tu-1", "status": "completed"}})),
    ]);
    let tool = ItemId("item-1".into());
    let msg = ItemId("item-2".into());
    assert_eq!(step.events, vec![
        Event::ToolResult { item: tool, output: "ok".into(), is_error: false },
        Event::TextStarted { item: msg.clone(), kind: TextKind::Text, text: "".into() },
        Event::TextDelta { item: msg.clone(), text: "Do".into() },
        Event::TextDelta { item: msg.clone(), text: "ne.".into() },
        Event::TextDone { item: msg, text: "Done.".into() },
        Event::TurnFinished { outcome: TurnOutcome::Completed, cost_usd: None },
    ]);
    assert_eq!(m.running_turn(), None);
}

#[test]
fn unknown_requests_are_refused_not_left_waiting() {
    let mut m = Mapper::new();
    let step = m.handle(&json!({"id": 7, "method": "account/chatgptAuthTokens/refresh", "params": {}}));
    assert!(step.events.is_empty());
    assert_eq!(step.replies[0]["id"], 7);
    assert!(step.replies[0]["error"].is_object());
}

#[test]
fn an_mcp_tool_approval_is_an_approval_answered_with_an_action() {
    let mut m = Mapper::new();
    let ask = json!({"id": 3, "method": "mcpServer/elicitation/request", "params": {"threadId": "t", "turnId": "u", "serverName": "docs", "mode": "form",
        "_meta": {"codex_approval_kind": "mcp_tool_call", "tool_name": "search", "tool_params": {"q": "x"}},
        "message": "Allow the docs MCP server to run tool \"search\"?", "requestedSchema": {"type": "object", "properties": {}}}});
    let step = m.handle(&ask);
    let Event::ApprovalRequested { request_id, tool_name, input, .. } = &step.events[0] else { panic!("{:?}", step.events) };
    assert_eq!((tool_name.as_str(), input), ("mcp__docs__search", &json!({"q": "x"})));
    let (reply, _) = m.resolve(request_id, &Decision::Allow).unwrap();
    assert_eq!(reply, json!({"id": 3, "result": {"action": "accept", "content": {}}}));
}

#[test]
fn a_question_is_answered_by_question_id() {
    let mut m = Mapper::new();
    let q = json!({"id": 3, "method": "item/tool/requestUserInput", "params": {"itemId": "i", "threadId": "t", "turnId": "u", "isBlocking": true,
        "questions": [{"id": "colour", "header": "Colour", "question": "Which colour?", "options": [{"label": "Blue"}]}]}});
    let step = m.handle(&q);
    let Event::ApprovalRequested { request_id, kind, .. } = &step.events[0] else { panic!() };
    assert_eq!(*kind, RequestKind::Question);
    let mut answers = Map::new();
    answers.insert("Which colour?".into(), "Blue".into());
    let (reply, _) = m.resolve(request_id, &Decision::Answer { answers }).unwrap();
    assert_eq!(reply, json!({"id": 3, "result": {"answers": {"colour": {"answers": ["Blue"]}}}}));
}

#[test]
fn rate_limits_become_windows_and_an_interrupt_settles_the_turn() {
    // The free plan's single 30-day window, as the live probe reported it.
    let Event::Usage { windows, limited } = rate_limits(&json!({"primary": {"usedPercent": 12, "windowDurationMins": 43200, "resetsAt": 1796569870}, "secondary": null, "rateLimitReachedType": null})) else { panic!() };
    assert_eq!((windows[0].name.as_str(), windows[0].utilization, limited), ("720h", Some(0.12), false));
    let mut m = Mapper::new();
    m.begin_turn("go", false);
    feed(&mut m, &[note("item/started", json!({"item": {"type": "commandExecution", "id": "e", "command": "sleep"}}))]);
    let step = feed(&mut m, &[note("turn/completed", json!({"turn": {"id": "u", "status": "interrupted"}}))]);
    assert_eq!(step.events.last(), Some(&Event::TurnFinished { outcome: TurnOutcome::Interrupted, cost_usd: None }));
    assert!(matches!(&step.events[0], Event::ToolResult { is_error: true, .. }));
}
