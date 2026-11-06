//! The requests Curia sends Codex's app-server (JSON-RPC without the `jsonrpc` field, one per
//! line; shapes from `codex app-server generate-json-schema`, 0.167.0).
use serde_json::{Value, json};

pub fn request(id: u64, method: &str, params: Value) -> Value {
    json!({"id": id, "method": method, "params": params})
}

pub fn initialized() -> Value {
    json!({"method": "initialized"})
}

pub fn initialize_params() -> Value {
    json!({"clientInfo": {"name": "curia", "version": env!("CARGO_PKG_VERSION")}})
}

pub fn text_input(text: &str) -> Value {
    json!([{"type": "text", "text": text}])
}

/// A reply to one of Codex's requests (an approval).
pub fn reply(id: &Value, result: Value) -> Value {
    json!({"id": id, "result": result})
}

/// Refuses a request Curia doesn't handle, so Codex isn't left waiting.
pub fn refuse(id: &Value, message: &str) -> Value {
    json!({"id": id, "error": {"code": -32601, "message": message}})
}
