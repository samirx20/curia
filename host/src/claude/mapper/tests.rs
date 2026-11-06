use super::*;
use curia_core::TextKind;
use serde_json::json;

fn feed(m: &mut Mapper, frames: &[Value]) -> Vec<Event> {
    frames.iter().flat_map(|f| m.handle(f).events).collect()
}

fn stream(index: u32, event: Value) -> Value {
    let mut e = event;
    e["index"] = json!(index);
    json!({"type": "stream_event", "parent_tool_use_id": null, "event": e})
}

#[test]
fn a_streamed_reply_becomes_one_text_item() {
    let mut m = Mapper::new();
    let mut events = m.begin_turn("hi");
    events.extend(feed(&mut m, &[
        json!({"type": "system", "subtype": "init", "session_id": "s-1", "model": "claude-haiku-5-5"}),
        stream(0, json!({"type": "message_start", "message": {"id": "msg-1"}})),
        stream(0, json!({"type": "content_block_start", "content_block": {"type": "text", "text": ""}})),
        stream(0, json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": "Hel"}})),
        stream(0, json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": "lo"}})),
        stream(0, json!({"type": "content_block_stop"})),
        // The whole message arrives again; it was streamed, so it adds nothing.
        json!({"type": "assistant", "parent_tool_use_id": null,
            "message": {"id": "msg-1", "content": [{"type": "text", "text": "Hello"}]}}),
        json!({"type": "result", "is_error": false, "total_cost_usd": 0.001}),
    ]));
    let item = ItemId("item-1".into());
    assert_eq!(events, vec![
        Event::TurnStarted { prompt: Some("hi".into()), by: None },
        Event::SessionStarted { native_session_id: "s-1".into(), model: Some("claude-haiku-5-5".into()) },
        Event::TextStarted { item: item.clone(), kind: TextKind::Text, text: "".into() },
        Event::TextDelta { item: item.clone(), text: "Hel".into() },
        Event::TextDelta { item: item.clone(), text: "lo".into() },
        Event::TextDone { item, text: "Hello".into() },
        Event::TurnFinished { outcome: TurnOutcome::Completed, cost_usd: Some(0.001) },
    ]);
    assert_eq!(m.session_id(), Some("s-1"));
    assert!(!m.in_turn());
}

#[test]
fn a_tool_call_asks_and_the_answer_goes_back() {
    let mut m = Mapper::new();
    m.begin_turn("write a file");
    let events = feed(&mut m, &[
        stream(0, json!({"type": "message_start", "message": {"id": "msg-1"}})),
        stream(0, json!({"type": "content_block_start", "content_block": {"type": "tool_use", "id": "tu-1", "name": "Write"}})),
        stream(0, json!({"type": "content_block_delta", "delta": {"type": "input_json_delta", "partial_json": "{\"file_path\":"}})),
        stream(0, json!({"type": "content_block_delta", "delta": {"type": "input_json_delta", "partial_json": "\"a.txt\"}"}})),
        stream(0, json!({"type": "content_block_stop"})),
        json!({"type": "control_request", "request_id": "r-1",
            "request": {"subtype": "can_use_tool", "tool_name": "Write", "input": {"file_path": "a.txt"}}}),
    ]);
    assert_eq!(events[0], Event::ToolCall {
        item: ItemId("item-1".into()), tool_use_id: "tu-1".into(), name: "Write".into(),
        input: json!({"file_path": "a.txt"}), parent_tool_use_id: None,
    });
    assert!(matches!(&events[1], Event::ApprovalRequested { kind: RequestKind::Permission, .. }));

    let (reply, event) = m.resolve("r-1", &Decision::Allow).unwrap();
    assert_eq!(reply, json!({"type": "control_response", "response": {"subtype": "success", "request_id": "r-1",
        "response": {"behavior": "allow", "updatedInput": {"file_path": "a.txt"}}}}));
    assert_eq!(event, Event::ApprovalResolved { request_id: "r-1".into(), resolution: Resolution::Allowed });
    assert!(m.resolve("r-1", &Decision::Allow).is_err(), "a request is answered once");

    let events = feed(&mut m, &[json!({"type": "user", "message": {"content": [
        {"type": "tool_result", "tool_use_id": "tu-1", "content": "File created", "is_error": false}]}})]);
    assert_eq!(events, vec![Event::ToolResult { item: ItemId("item-1".into()), output: "File created".into(), is_error: false }]);
}

#[test]
fn other_control_requests_are_refused_not_left_waiting() {
    let mut m = Mapper::new();
    let step = m.handle(&json!({"type": "control_request", "request_id": "r-9", "request": {"subtype": "mcp_message"}}));
    assert!(step.events.is_empty());
    assert_eq!(step.replies[0]["response"]["subtype"], "error");
    assert_eq!(step.replies[0]["response"]["request_id"], "r-9");
}

#[test]
fn an_interrupt_settles_open_tools_and_requests() {
    let mut m = Mapper::new();
    m.begin_turn("go");
    feed(&mut m, &[
        json!({"type": "assistant", "parent_tool_use_id": null, "message": {"id": "m", "content": [
            {"type": "tool_use", "id": "tu-1", "name": "Bash", "input": {"command": "sleep 60"}}]}}),
        json!({"type": "control_request", "request_id": "r-1",
            "request": {"subtype": "can_use_tool", "tool_name": "Bash", "input": {}}}),
    ]);
    let events = feed(&mut m, &[json!({"type": "result", "is_error": true, "terminal_reason": "aborted_streaming"})]);
    assert_eq!(events, vec![
        Event::ToolResult { item: ItemId("item-1".into()), output: "Interrupted".into(), is_error: true },
        Event::ApprovalResolved { request_id: "r-1".into(), resolution: Resolution::Cancelled },
        Event::TurnFinished { outcome: TurnOutcome::Interrupted, cost_usd: None },
    ]);
}

#[test]
fn claude_can_start_a_turn_by_itself() {
    // Seen live: a background subagent finished after the turn ended, and Claude began a new
    // turn (with a fresh `init`) to report it. The session id is announced once, not per turn.
    let mut m = Mapper::new();
    m.begin_turn("start a background subagent");
    let init = json!({"type": "system", "subtype": "init", "session_id": "s-1", "model": "m"});
    feed(&mut m, &[init.clone(), json!({"type": "result", "is_error": false})]);
    let events = feed(&mut m, &[init, json!({"type": "result", "is_error": false})]);
    assert_eq!(events, vec![
        Event::TurnStarted { prompt: None, by: None },
        Event::TurnFinished { outcome: TurnOutcome::Completed, cost_usd: None },
    ]);
}

#[test]
fn a_message_written_mid_turn_gets_its_own_turn() {
    // Seen live on 2.1.318: the running turn finishes without it, then Claude starts a turn
    // for it (with a fresh `init`).
    let mut m = Mapper::new();
    let init = json!({"type": "system", "subtype": "init", "session_id": "s-1", "model": "m"});
    m.begin_turn("count to 400");
    feed(&mut m, &[init.clone()]);
    assert!(m.begin_turn("then say BANANA").is_empty(), "nothing starts while a turn runs");
    feed(&mut m, &[json!({"type": "result", "is_error": false})]);
    let events = feed(&mut m, &[init]);
    assert_eq!(events, vec![Event::TurnStarted { prompt: Some("then say BANANA".into()), by: None }]);
}

#[test]
fn only_agent_tasks_are_subagents() {
    let mut m = Mapper::new();
    let task = |id: &str, kind: &str| json!({"type": "system", "subtype": "task_started", "task_id": id,
        "tool_use_id": "tu", "description": "d", "task_type": kind});
    let done = |id: &str| json!({"type": "system", "subtype": "task_notification", "task_id": id, "status": "completed"});
    let events = feed(&mut m, &[task("a", "local_agent"), task("b", "local_bash"), done("b"), done("a")]);
    assert_eq!(events, vec![
        Event::SubagentStarted { task_id: "a".into(), tool_use_id: Some("tu".into()), description: "d".into() },
        Event::SubagentEnded { task_id: "a".into(), status: "completed".into() },
    ]);
}

#[test]
fn an_exit_mid_turn_fails_the_turn() {
    let mut m = Mapper::new();
    m.begin_turn("go");
    let events = m.on_exit(Some(1), "boom");
    assert!(matches!(&events[0], Event::TurnFinished { outcome: TurnOutcome::Failed { message }, .. } if message.contains("boom")));
    assert_eq!(events[1], Event::Exited { code: Some(1) });
}
