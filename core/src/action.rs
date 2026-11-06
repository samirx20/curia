//! What a client (or the host itself) asks a thread to do.
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The answer to an `ApprovalRequested` event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum Decision {
    Allow,
    /// For a question: the chosen answer per question text.
    Answer { answers: Map<String, Value> },
    Deny { message: Option<String> },
}
