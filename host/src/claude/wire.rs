//! The lines Curia writes to Claude's stdin (stream-json). Shapes from Agent Hub's parked
//! Rust app, checked live there against claude 2.1.300.
use serde_json::{Value, json};

/// A control request (`initialize`, `interrupt`, `apply_flag_settings`, `set_model`, …).
pub fn control(request_id: &str, request: Value) -> Value {
    json!({"type": "control_request", "request_id": request_id, "request": request})
}

pub fn initialize() -> Value {
    json!({"subtype": "initialize", "hooks": null})
}

pub fn interrupt() -> Value {
    json!({"subtype": "interrupt"})
}

/// The account's usage windows. `skipBehaviors` stops Claude scanning its whole history.
pub fn get_usage() -> Value {
    json!({"subtype": "get_usage", "skipBehaviors": true})
}

pub fn set_effort(level: &str) -> Value {
    json!({"subtype": "apply_flag_settings", "settings": {"effortLevel": level}})
}

/// A user turn. `content` is a string or Messages API content blocks.
pub fn user(content: Value, session_id: Option<&str>) -> Value {
    json!({
        "type": "user",
        "message": {"role": "user", "content": content},
        "parent_tool_use_id": null,
        "session_id": session_id.unwrap_or(""),
    })
}

/// The answer to one of Claude's control requests (an approval).
pub fn response_success(request_id: &str, body: Value) -> Value {
    json!({"type": "control_response",
        "response": {"subtype": "success", "request_id": request_id, "response": body}})
}

/// Refuses a control request Curia doesn't handle, so the CLI isn't left waiting.
pub fn response_error(request_id: &str, error: &str) -> Value {
    json!({"type": "control_response",
        "response": {"subtype": "error", "request_id": request_id, "error": error}})
}
