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
    /// Signs in a client on the host itself, with the token from `link.json`.
    Hello { token: String },
    /// A fresh challenge for a paired device to sign (the first step of `hello_device`).
    Challenge,
    /// Signs in a paired device: `signature` is its key's signature of `curia-sign-in:<nonce>`
    /// (ECDSA P-256 with SHA-256, the raw 64-byte form, base64).
    HelloDevice { device_id: String, signature: String },
    /// Pairs a new device with a one-time code from `pair_code`. `public_key`: its P-256
    /// public key as the uncompressed point, base64. Answers with the device's id and role.
    Pair { code: String, name: String, public_key: String },
    /// A one-time pairing code for a new device with `role` (`owner`, `approve`, `view`),
    /// good for `minutes` (default 1, at most 10).
    PairCode {
        role: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        minutes: Option<u32>,
    },
    /// The paired devices.
    DevicesList,
    /// Unpairs a device: its key is deleted and its connections are closed.
    DeviceRevoke { device_id: String },
    /// The host's Web Push public key (base64url), to subscribe a browser with.
    PushKey,
    /// Every file in a thread's folder, with `added`/`modified`/`deleted` where git says so.
    ThreadTree { thread_id: String },
    /// A link (`/file/<token>`, ten minutes) to a file's bytes; `rev: "HEAD"` for its last
    /// committed version.
    FileLink {
        thread_id: String,
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rev: Option<String>,
    },
    /// This machine, its resources, and how devices reach it (Settings > Host).
    HostInfo,
    /// Renames a team or changes its default model or effort (an empty value clears them).
    TeamUpdate {
        id: String,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        model: Option<String>,
        #[serde(default)]
        effort: Option<String>,
    },
    /// Removes a team no thread uses.
    TeamRemove { id: String },
    /// Opens a console window on the host where the CLI signs an account in.
    TeamSignIn { cli: String, config_dir: String },
    /// Changes a paired device's role (`owner`, `approve`, `view`).
    DeviceSetRole { device_id: String, role: String },
    /// Starts the host when the person signs in to the host machine (or stops doing so).
    AutostartSet { enabled: bool },
    /// Threads whose title, or something said in them, contains the words.
    Search { query: String },
    /// A run's totals: threads, hand-offs, merges, approvals, cost and time.
    RunSummary { thread_id: String },
    /// The TCP ports the thread's programs listen on (for the browser rail).
    ThreadPorts { thread_id: String },
    /// A thread's terminals (shells the host runs in its folder).
    TerminalList { thread_id: String },
    /// Starts a shell in a thread's folder; open it with `terminal_open`.
    TerminalStart { thread_id: String, cols: u16, rows: u16 },
    /// Shows a terminal: answers with its screen, then sends `terminal` notifications as it changes.
    TerminalOpen { id: String },
    /// Keys typed into a terminal, as the bytes a terminal sends.
    TerminalInput { id: String, data: String },
    /// Resizes a terminal (one size for everyone: the last resize wins).
    TerminalResize { id: String, cols: u16, rows: u16 },
    TerminalClose { id: String },
    /// Sends this device's notifications to a browser push subscription (its endpoint and
    /// the `p256dh` and `auth` keys, base64url). Paired devices only.
    PushSubscribe { endpoint: String, p256dh: String, auth: String },
    ThreadsList,
    ThreadCreate { thread: NewThreadParams },
    /// Starts receiving a thread's events. `after_seq`: the last event this client has.
    ThreadOpen { thread_id: String, after_seq: Option<i64> },
    /// Older history, before `before_seq`, newest last.
    ThreadHistory { thread_id: String, before_seq: i64, limit: u32 },
    ThreadSend { thread_id: String, text: String },
    /// Answers a request. A paired device signs `curia-answer:<thread_id>:<request_id>:<allow|deny|answer>`,
    /// so the log records who answered.
    ThreadAnswer {
        thread_id: String,
        request_id: String,
        decision: Decision,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
    ThreadInterrupt { thread_id: String },
    TeamsList,
    TeamCreate { team: NewTeamParams },
    /// Undoes a hand-back merge (its id is in the `hand_back` event).
    MergeUndo { merge_id: String },
    /// Moves an idle thread to another team of the same CLI (its conversation continues).
    ThreadMove { thread_id: String, team: String },
    /// The host's settings, as a JSON object.
    SettingsGet,
    /// Replaces the host's settings (send back the object from `settings_get`, edited).
    SettingsSet { settings: Value },
    /// Every approval and question waiting for an answer, across all threads.
    InboxList,
    /// Changes a thread's model, effort or freedom (`plan`, `ask`, `sandbox`, `full`).
    ThreadConfigure { thread_id: String, model: Option<String>, effort: Option<String>, freedom: Option<String> },
    /// The changed files in a thread's folder.
    ThreadFiles { thread_id: String },
    /// One file's text, from a thread's folder.
    FileRead { thread_id: String, path: String },
    /// One file's changes against its folder's last commit.
    FileDiff { thread_id: String, path: String },
}

/// A new run's main thread (team threads are made by `hand_off`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewThreadParams {
    pub title: String,
    /// The team (by name) whose account runs this thread.
    pub team: String,
    /// The run's folder: a git repository, so hand-offs can give teams their own branch.
    pub cwd: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub permission_mode: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewTeamParams {
    pub name: String,
    pub cli: String,
    /// The account's own config folder (`CLAUDE_CONFIG_DIR`, …).
    pub config_dir: String,
    pub model: Option<String>,
    pub effort: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TeamInfo {
    pub id: String,
    pub name: String,
    pub cli: String,
    pub model: Option<String>,
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
    /// The thread that handed this one its task (none for a run's main thread).
    #[serde(default)]
    pub parent_id: Option<String>,
    /// The run's main thread (itself for a main thread).
    #[serde(default)]
    pub root_id: String,
    #[serde(default)]
    pub team_id: Option<String>,
    /// Freedom level: `plan`, `ask`, `sandbox` or `full`.
    #[serde(default)]
    pub freedom: String,
    /// A team thread's own branch.
    #[serde(default)]
    pub branch: Option<String>,
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
    /// Something happened in a thread this client doesn't follow (`params`: `thread_id`,
    /// `kind` = the event's type). Lists that span every thread (the inbox, the sidebar)
    /// refresh on it.
    pub const ACTIVITY: &'static str = "activity";
    /// A terminal's screen changed: `{id, title, screen, exited}`.
    pub const TERMINAL: &'static str = "terminal";
}

/// Error codes (JSON-RPC reserves -32768..-32000 for its own).
pub mod code {
    pub const INVALID: i64 = -32600;
    pub const UNAUTHORIZED: i64 = 1;
    pub const FAILED: i64 = 2;
}
