//! Usage limits and how a turn ended.
use curia_core::{Event, TurnOutcome, UsageWindow};
use serde_json::Value;

/// `rate_limit_event`: utilization and reset time per window (5-hour, weekly, …).
pub(super) fn rate_limit(v: &Value) -> Event {
    let info = &v["rate_limit_info"];
    let mut windows = Vec::new();
    if let Some(map) = info["unifiedWindows"].as_object() {
        for (name, w) in map {
            windows.push(UsageWindow {
                name: name.clone(),
                utilization: w["utilization"].as_f64(),
                resets_at_ms: w["resetsAt"].as_i64().map(|s| s * 1000),
            });
        }
    }
    Event::Usage { windows, limited: info["status"] == "rejected" }
}

/// The turn's outcome from its `result` frame.
pub(super) fn outcome(v: &Value) -> TurnOutcome {
    if v["terminal_reason"].as_str().is_some_and(|r| r.starts_with("aborted")) {
        return TurnOutcome::Interrupted;
    }
    if usage_limited(v) {
        return TurnOutcome::UsageLimited { resets_at_ms: None };
    }
    if v["is_error"].as_bool().unwrap_or(false) {
        let errors = v["errors"].as_array().map(|e| e.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join("; "));
        let message = v["result"].as_str().map(str::to_string).or(errors);
        let message = message.unwrap_or_else(|| format!("Claude failed ({})", v["subtype"]));
        return TurnOutcome::Failed { message };
    }
    TurnOutcome::Completed
}

/// A failed result caused by the account's usage limit, not an ordinary error.
pub(super) fn usage_limited(v: &Value) -> bool {
    if !v["is_error"].as_bool().unwrap_or(false) {
        return false;
    }
    let text = v["result"].as_str().unwrap_or("").to_lowercase();
    v["terminal_reason"] == "blocking_limit"
        || v["api_error_status"].as_i64() == Some(429)
        || (text.contains("limit") && (text.contains("usage") || text.contains("reset") || text.contains("rate")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detects_usage_limit_results() {
        assert!(usage_limited(&json!({"is_error": true, "terminal_reason": "blocking_limit"})));
        assert!(usage_limited(&json!({"is_error": true, "api_error_status": 429})));
        assert!(usage_limited(&json!({"is_error": true, "result": "Claude usage limit reached. Your limit will reset at 7pm"})));
        assert!(!usage_limited(&json!({"is_error": true, "result": "Tool failed"})));
        assert!(!usage_limited(&json!({"is_error": false, "terminal_reason": "blocking_limit"})));
    }

    #[test]
    fn aborted_means_interrupted() {
        let v = json!({"type": "result", "is_error": true, "terminal_reason": "aborted_streaming"});
        assert_eq!(outcome(&v), TurnOutcome::Interrupted);
    }
}
