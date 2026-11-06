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

/// The windows in a `get_usage` reply. Unlike `rate_limit_event` (a 0–1 fraction and epoch
/// seconds), it gives a 0–100 percentage and an ISO date (checked on 2.1.318).
pub fn usage_reply(body: &Value) -> Vec<UsageWindow> {
    let Some(limits) = body["rate_limits"].as_object() else { return Vec::new() };
    let mut windows = Vec::new();
    for (name, w) in limits {
        let Some(pct) = w.get("utilization").and_then(Value::as_f64) else { continue };
        if !(name == "five_hour" || name.starts_with("seven_day")) {
            continue;
        }
        let resets_at_ms = w["resets_at"]
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.timestamp_millis());
        windows.push(UsageWindow { name: name.clone(), utilization: Some(pct / 100.0), resets_at_ms });
    }
    windows
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
    fn a_get_usage_reply_becomes_windows() {
        let body = json!({"rate_limits": {
            "five_hour": {"utilization": 8, "resets_at": "2026-11-06T16:49:59.562421+00:00"},
            "seven_day": {"utilization": 10, "resets_at": "2026-11-10T08:59:59+00:00"},
            "seven_day_opus": null,
            "extra_usage": {"utilization": null}
        }});
        let mut w = usage_reply(&body);
        w.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(w.len(), 2);
        assert_eq!((w[0].name.as_str(), w[0].utilization), ("five_hour", Some(0.08)));
        assert_eq!(w[0].resets_at_ms, Some(1793983799562));
        assert_eq!((w[1].name.as_str(), w[1].utilization), ("seven_day", Some(0.10)));
    }

    #[test]
    fn aborted_means_interrupted() {
        let v = json!({"type": "result", "is_error": true, "terminal_reason": "aborted_streaming"});
        assert_eq!(outcome(&v), TurnOutcome::Interrupted);
    }
}
