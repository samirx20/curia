//! Quota, moving threads, auto-continue and redaction, without a live CLI.
use super::Host;
use crate::store::{NewThread, ThreadRow, now_ms};
use curia_core::{Event, ItemId, TurnOutcome, UsageWindow};
use std::sync::Arc;

fn host() -> Arc<Host> {
    Host::open(&std::env::temp_dir().join(format!("curia-quota-test-{}", crate::store::new_id()))).unwrap().0
}

fn on_team(h: &Host, team_id: &str, config_dir: &str) -> ThreadRow {
    h.store
        .thread_create(&NewThread {
            title: "t".into(), cli: "claude".into(), config_dir: config_dir.into(), cwd: ".".into(),
            permission_mode: "ask".into(), team_id: Some(team_id.into()), ..Default::default()
        })
        .unwrap()
}

fn window(name: &str, used: f64, resets_in_min: i64) -> UsageWindow {
    UsageWindow { name: name.into(), utilization: Some(used), resets_at_ms: Some(now_ms() + resets_in_min * 60_000) }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn usage_reaches_the_team_and_auto_picks_the_most_left() {
    let h = host();
    let a = h.store.team_create("a", "claude", "ca", None, None).unwrap();
    let b = h.store.team_create("b", "claude", "cb", None, None).unwrap();
    let (ta, tb) = (on_team(&h, &a.id, "ca"), on_team(&h, &b.id, "cb"));
    h.usage_seen(&ta.id, &[window("five_hour", 0.70, 60), window("seven_day", 0.20, 3000)], false).unwrap();
    h.usage_seen(&tb.id, &[window("five_hour", 0.10, 60)], false).unwrap();
    assert_eq!(h.choose_team("auto", None).unwrap().name, "b");
    let listed = h.teams_with_quota(None).unwrap();
    let a_row = listed.iter().find(|t| t["name"] == "a").unwrap();
    assert_eq!((a_row["five_hour_percent_left"].as_f64(), a_row["weekly_percent_left"].as_f64()), (Some(30.0), Some(80.0)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_team_that_runs_out_marks_itself_and_the_thread_waits_for_the_reset() {
    let h = host();
    let a = h.store.team_create("a", "claude", "ca", None, None).unwrap();
    let t = on_team(&h, &a.id, "ca");
    h.usage_seen(&t.id, &[window("five_hour", 1.0, 30)], true).unwrap();
    h.inject(&t.id, &[
        Event::TurnStarted { prompt: Some("go".into()) },
        Event::TurnFinished { outcome: TurnOutcome::UsageLimited { resets_at_ms: None }, cost_usd: None },
    ]).unwrap();
    h.out_of_quota(&t.id).await.unwrap();
    let c = h.store.team_candidate(&a.id).unwrap();
    assert!(c.limited_until.unwrap() > now_ms());
    assert!(h.choose_team("auto", None).is_err(), "an out-of-quota team isn't picked");
    // Due after the reset, not before.
    assert!(h.store.threads_due(now_ms()).unwrap().is_empty());
    assert_eq!(h.store.threads_due(now_ms() + 32 * 60_000).unwrap(), vec![t.id.clone()]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_thread_moves_only_to_a_team_that_can_resume_its_session() {
    let h = host();
    let root = std::env::temp_dir().join(format!("curia-cfg-{}", &crate::store::new_id()[..8]));
    let (shared, other) = (root.join("shared"), root.join("other"));
    std::fs::create_dir_all(shared.join("projects").join("C--work")).unwrap();
    std::fs::create_dir_all(other.join("projects")).unwrap();
    std::fs::write(shared.join("projects").join("C--work").join("s-1.jsonl"), "{}").unwrap();
    let a = h.store.team_create("a", "claude", "ca", None, None).unwrap();
    h.store.team_create("shared", "claude", &shared.to_string_lossy(), None, None).unwrap();
    h.store.team_create("other", "claude", &other.to_string_lossy(), None, None).unwrap();
    let t = on_team(&h, &a.id, "ca");
    h.inject(&t.id, &[Event::SessionStarted { native_session_id: "s-1".into(), model: None }]).unwrap();
    assert!(super::quota::session_exists_for_test(&shared, "s-1"));
    let err = h.move_thread(&t.id, "other").await.unwrap_err();
    assert!(format!("{err:#}").contains("can't see"), "{err:#}");
    h.move_thread(&t.id, "shared").await.unwrap();
    let moved = h.store.thread_get(&t.id).unwrap();
    assert_eq!(moved.config_dir, shared.to_string_lossy());
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restarts_are_not_continued_past_the_limit_or_over_a_waiting_question() {
    let h = host();
    let a = h.store.team_create("a", "claude", "ca", None, None).unwrap();
    let (looping, asked) = (on_team(&h, &a.id, "ca"), on_team(&h, &a.id, "ca"));
    for _ in 0..3 {
        h.store.thread_auto_continues(&looping.id, false).unwrap();
    }
    let recovered = crate::store::Recovered { cut: vec![looping.id.clone(), asked.id.clone()], waiting: vec![asked.id.clone()], ..Default::default() };
    h.continue_after_restart(&recovered).await.unwrap();
    let notices = |id: &str| h.store.events_after(id, 0).unwrap().into_iter().filter(|s| matches!(s.event, Event::Notice { .. })).count();
    assert_eq!(notices(&looping.id), 1, "a 4th restart in a row is not continued, and says so");
    assert_eq!(notices(&asked.id), 0, "a thread with a waiting question is left for the user");
    assert!(!h.is_live(&looping.id) && !h.is_live(&asked.id));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn secrets_are_redacted_before_they_are_stored() {
    let h = host();
    let a = h.store.team_create("a", "claude", "ca", None, None).unwrap();
    let t = on_team(&h, &a.id, "ca");
    h.inject(&t.id, &[Event::ToolResult { item: ItemId("item-1".into()), output: "curl -H 'Authorization: Bearer abc123'".into(), is_error: false }]).unwrap();
    let stored = h.store.events_after(&t.id, 0).unwrap();
    let Event::ToolResult { output, .. } = &stored[0].event else { panic!() };
    assert!(!output.contains("abc123") && output.contains("[redacted]"), "{output}");
}
