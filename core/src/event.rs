//! The shared events. Every CLI adapter turns its CLI's output into these, and nothing
//! downstream knows which CLI produced them.
//!
//! Ids: `ItemId` is Curia's own id for a piece of a turn (identity). The CLI's own ids
//! (`native_session_id`, `tool_use_id`, `request_id`) are kept as references, never as identity.
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Curia's id for one item in a thread (a message, a thinking block, a tool call).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ItemId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextKind {
    Text,
    Thinking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestKind {
    /// Run a tool (a command, an edit, a fetch).
    Permission,
    /// The agent asks the user a question with options.
    Question,
    /// The agent presents a plan and asks to leave plan mode.
    Plan,
}

/// One usage window, such as the 5-hour or weekly limit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageWindow {
    pub name: String,
    /// 0.0 to 1.0, when the CLI reports it.
    pub utilization: Option<f64>,
    /// Unix time in milliseconds.
    pub resets_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum TurnOutcome {
    Completed,
    Interrupted,
    /// The account hit its usage limit.
    UsageLimited { resets_at_ms: Option<i64> },
    Failed { message: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// The CLI reported its own session id (used to resume) and model.
    SessionStarted { native_session_id: String, model: Option<String> },
    /// A turn began. `prompt` is the message that started it, or `None` when the CLI started
    /// it by itself (for example to report a background subagent that finished).
    TurnStarted { prompt: Option<String> },
    /// A text or thinking block began. `text` is what arrived with it (often empty).
    TextStarted { item: ItemId, kind: TextKind, text: String },
    TextDelta { item: ItemId, text: String },
    /// The block is complete; `text` is the whole of it.
    TextDone { item: ItemId, text: String },
    /// A tool call with its complete input. `parent_tool_use_id` is set when a subagent made it.
    ToolCall {
        item: ItemId,
        tool_use_id: String,
        name: String,
        input: Value,
        parent_tool_use_id: Option<String>,
    },
    ToolResult { item: ItemId, output: String, is_error: bool },
    /// A subagent started by a tool call (`tool_use_id`) of this thread. Background shell
    /// commands are not subagents; they show as their tool call.
    SubagentStarted { task_id: String, tool_use_id: Option<String>, description: String },
    SubagentEnded { task_id: String, status: String },
    /// The CLI asks before acting. Answered with a `Decision`.
    ApprovalRequested { request_id: String, kind: RequestKind, tool_name: String, input: Value },
    /// The request was answered, or cancelled because its turn ended.
    ApprovalResolved { request_id: String, resolution: Resolution },
    Usage { windows: Vec<UsageWindow>, limited: bool },
    /// Something worth showing that isn't a message (context compacted, a CLI error).
    Notice { text: String },
    TurnFinished { outcome: TurnOutcome, cost_usd: Option<f64> },
    /// The CLI process ended.
    Exited { code: Option<i32> },

    // Control room: recorded by the host, not by a CLI adapter.
    /// This thread handed a task to a team; `thread_id` is the new team thread.
    HandedOff { thread_id: String, team: String, title: String, branch: Option<String> },
    /// A message from another thread (or the user, `from` = None) reached this thread.
    /// `how`: `steered` into the running turn, `queued` until it ends, or `started` a turn.
    Message { from: Option<String>, text: String, how: String },
    /// A team thread handed back. `inferred`: it never called report_result, so its last
    /// message is used. `ok`: its turn completed (false: it failed or was cut off).
    HandBack { from: String, summary: String, inferred: bool, ok: bool, merge: MergeReport },
    /// A merge was undone (`merge_id` from a `HandBack`).
    MergeUndone { merge_id: String },
}

/// What happened to a team's branch when it handed back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum MergeReport {
    Merged { merge_id: String, branch: String, files: Vec<String> },
    /// The branch had nothing new.
    NothingToMerge { branch: String },
    /// Not merged; the work stays on `branch`.
    Failed { branch: String, reason: String },
    /// The thread has no branch (it shares its parent's folder).
    NoBranch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    Allowed,
    Answered,
    Denied,
    /// The turn ended (or the CLI process closed) before anyone answered.
    Cancelled,
    /// The host restarted while it was waiting; no CLI can take the answer any more.
    Expired,
}
