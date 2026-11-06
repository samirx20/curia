//! What a paired device signs with its key, written once for the host that checks and every
//! client that signs (the phone app writes the same strings).
use crate::Decision;

/// Signing in: the nonce from `challenge`.
pub fn sign_in_message(nonce: &str) -> String {
    format!("curia-sign-in:{nonce}")
}

/// Answering a request, so the log can say which device allowed or denied it.
pub fn answer_message(thread_id: &str, request_id: &str, decision: &Decision) -> String {
    let kind = match decision {
        Decision::Allow => "allow",
        Decision::Deny { .. } => "deny",
        Decision::Answer { .. } => "answer",
    };
    format!("curia-answer:{thread_id}:{request_id}:{kind}")
}
