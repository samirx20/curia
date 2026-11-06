//! The link between clients and the host: JSON-RPC 2.0 over a WebSocket.
//!
//! A client calls methods (`Call`) and gets one `Reply` per call. The host pushes events as
//! `event` notifications. Each stored event has a `seq`: its place in the host's log. A client
//! that reconnects opens a thread with the last `seq` it saw and receives exactly what came after,
//! or a fresh snapshot when the gap is too large.
use crate::{Decision, Event};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A client's request. `id` is echoed in the reply.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(flatten)]
    pub call: Call,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "snake_case")]
pub enum Call {
    /// Must be the first call on a connection.
    Hello { token: String },
    ThreadsList,
    ThreadCreate { thread: NewThreadParams },
    /// Starts receiving a thread's events. `after_seq`: the last event this client has.
    ThreadOpen { thread_id: String, after_seq: Option<i64> },
    /// Older history, before `before_seq`, newest last.
    ThreadHistory { thread_id: String, before_seq: i64, limit: u32 },
    ThreadSend { thread_id: String, text: String },
    ThreadAnswer { thread_id: String, request_id: String, decision: Decision },
    ThreadInterrupt { thread_id: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewThreadParams {
    pub title: String,
    pub cli: String,
    pub config_dir: String,
    pub cwd: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub permission_mode: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThreadInfo {
    pub id: String,
    pub title: String,
    pub cli: String,
    pub cwd: String,
    pub model: Option<String>,
    /// `idle` or `running`.
    pub status: String,
    pub updated_at: i64,
}

/// One event as a client receives it. Live-only events (streamed text) have no `seq`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireEvent {
    pub thread_id: String,
    pub seq: Option<i64>,
    pub event: Event,
}

/// The answer to `thread_open`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Opened {
    /// Everything after the client's `after_seq`, in order.
    Replay { thread: ThreadInfo, events: Vec<WireEvent> },
    /// The client's copy is too old (or ahead of the host): start over from the newest events.
    /// `has_older` says whether `thread_history` can page further back.
    Snapshot { thread: ThreadInfo, events: Vec<WireEvent>, has_older: bool },
}

/// The host's answer to one request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reply {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

/// A host → client push.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub jsonrpc: String,
    pub method: String,
    pub params: Value,
}

impl Notification {
    pub const EVENT: &'static str = "event";
    /// The host is closing this connection; `params.reason` says why (for example, the
    /// client fell too far behind and should reconnect for a snapshot).
    pub const CLOSING: &'static str = "closing";
}

/// Error codes (JSON-RPC reserves -32768..-32000 for its own).
pub mod code {
    pub const INVALID: i64 = -32600;
    pub const UNAUTHORIZED: i64 = 1;
    pub const FAILED: i64 = 2;
}
