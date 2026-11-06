use super::*;
use curia_core::{Event, ItemId, RequestKind, Resolution, TextKind, TurnOutcome};
use serde_json::json;

fn thread(store: &Store) -> ThreadRow {
    store
        .thread_create(&NewThread {
            title: "t".into(),
            cli: "claude".into(),
            config_dir: "cfg".into(),
            cwd: ".".into(),
            model: None,
            effort: None,
            permission_mode: "manual".into(),
            ..Default::default()
        })
        .unwrap()
}

fn asks(id: &str) -> Event {
    Event::ApprovalRequested { request_id: id.into(), kind: RequestKind::Permission, tool_name: "Write".into(), input: json!({}) }
}

#[test]
fn events_are_numbered_and_keep_the_tables_in_step() {
    let store = Store::in_memory();
    let t = thread(&store);
    let item = ItemId("item-1".into());
    let stored = store
        .record(&t.id, &[
            Event::TurnStarted { prompt: Some("hi".into()) },
            Event::SessionStarted { native_session_id: "s-1".into(), model: None },
            Event::TextStarted { item: item.clone(), kind: TextKind::Text, text: "".into() },
            Event::TextDelta { item: item.clone(), text: "Hel".into() },
            Event::TextDone { item, text: "Hello".into() },
            asks("r-1"),
        ])
        .unwrap();
    assert_eq!(stored.len(), 5, "text deltas are live-only");
    assert!(stored.windows(2).all(|w| w[1].seq > w[0].seq));
    let turn = stored[0].turn_id.clone().unwrap();
    assert!(stored.iter().all(|s| s.turn_id.as_deref() == Some(turn.as_str())));

    let row = store.thread_get(&t.id).unwrap();
    assert_eq!((row.status.as_str(), row.native_session_id.as_deref()), ("running", Some("s-1")));

    store
        .record(&t.id, &[
            Event::ApprovalResolved { request_id: "r-1".into(), resolution: Resolution::Allowed },
            Event::TurnFinished { outcome: TurnOutcome::Completed, cost_usd: Some(0.01) },
        ])
        .unwrap();
    assert_eq!(store.thread_get(&t.id).unwrap().status, "idle");
    assert_eq!(store.turns(&t.id).unwrap()[0].status, "completed");
    let after = store.events_after(&t.id, stored[4].seq).unwrap();
    assert_eq!(after.len(), 2);
    assert_eq!(after[1].event, Event::TurnFinished { outcome: TurnOutcome::Completed, cost_usd: Some(0.01) });
}

#[test]
fn recovery_expires_requests_ends_subagents_and_cuts_turns() {
    let store = Store::in_memory();
    let (a, b) = (thread(&store), thread(&store));
    store
        .record(&a.id, &[
            Event::TurnStarted { prompt: Some("go".into()) },
            asks("r-1"),
            Event::SubagentStarted { task_id: "k".into(), tool_use_id: None, description: "d".into() },
        ])
        .unwrap();
    store
        .record(&b.id, &[
            Event::TurnStarted { prompt: Some("done".into()) },
            Event::TurnFinished { outcome: TurnOutcome::Completed, cost_usd: None },
        ])
        .unwrap();

    let done = store.recover().unwrap();
    assert_eq!(done, Recovered { threads: vec![a.id.clone()], turns: 1, requests: 1, subagents: 1, cut: vec![a.id.clone()], waiting: vec![a.id.clone()] });
    let tail: Vec<Event> = store.events_after(&a.id, 0).unwrap().into_iter().map(|s| s.event).skip(3).collect();
    assert_eq!(tail, vec![
        Event::ApprovalResolved { request_id: "r-1".into(), resolution: Resolution::Expired },
        Event::SubagentEnded { task_id: "k".into(), status: "host_restarted".into() },
        Event::Notice { text: "The host restarted during this turn, so it was cut off.".into() },
        Event::TurnFinished { outcome: TurnOutcome::Interrupted, cost_usd: None },
    ]);
    assert_eq!(store.turns(&a.id).unwrap()[0].status, "interrupted");
    assert_eq!(store.thread_get(&a.id).unwrap().status, "idle");
    assert_eq!(store.turns(&b.id).unwrap()[0].status, "completed", "finished turns are untouched");
    assert_eq!(store.recover().unwrap(), Recovered::default(), "a second recovery finds nothing");
}

#[test]
fn item_counts_never_go_back() {
    let store = Store::in_memory();
    let t = thread(&store);
    store.thread_set_items_used(&t.id, 7).unwrap();
    store.thread_set_items_used(&t.id, 3).unwrap();
    assert_eq!(store.thread_get(&t.id).unwrap().items_used, 7);
}

#[test]
fn reopening_a_database_keeps_it() {
    let path = std::env::temp_dir().join(format!("curia-store-test-{}.sqlite", new_id()));
    let id = thread(&Store::open(&path).unwrap()).id;
    assert_eq!(Store::open(&path).unwrap().thread_get(&id).unwrap().id, id);
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
}
