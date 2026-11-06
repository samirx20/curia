//! The link end to end: a real WebSocket server and real clients, with events injected as if
//! a CLI produced them.
use crate::host::Host;
use crate::store::NewThread;
use curia_client::{Incoming, Link, parse};
use curia_core::protocol::{Call, Opened, WireEvent};
use curia_core::{Event, ItemId};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;

fn notice(n: u32) -> Event {
    Event::Notice { text: format!("n{n}") }
}

async fn next(rx: &mut UnboundedReceiver<Incoming>) -> WireEvent {
    match tokio::time::timeout(Duration::from_secs(5), rx.recv()).await {
        Ok(Some(Incoming::Event(e))) => e,
        other => panic!("expected an event, got {other:?}"),
    }
}

async fn open(link: &Link, thread_id: &str, after_seq: Option<i64>) -> Opened {
    parse(link.call(Call::ThreadOpen { thread_id: thread_id.into(), after_seq }).await.unwrap()).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn clients_catch_up_exactly_after_a_disconnect() {
    let data = std::env::temp_dir().join(format!("curia-link-test-{}", crate::store::new_id()));
    let (host, _) = Host::open(&data).unwrap();
    let t = host
        .create_thread(&NewThread {
            title: "t".into(), cli: "claude".into(), config_dir: "c".into(), cwd: ".".into(),
            model: None, effort: None, permission_mode: "manual".into(),
        })
        .unwrap();
    host.inject(&t.id, &[notice(1), notice(2), notice(3)]).unwrap();
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(super::serve(host.clone(), 0, async move {
        let _ = stopped.await;
    }));
    for _ in 0..50 {
        if data.join("link.json").exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // A new client gets a snapshot; one with a cursor gets exactly what follows it.
    let (a, mut a_rx) = Link::connect_local(&data).await.unwrap();
    let Opened::Snapshot { events, has_older, .. } = open(&a, &t.id, None).await else { panic!("expected a snapshot") };
    assert_eq!((events.len(), has_older), (3, false));
    let first = events[0].seq.unwrap();
    let (b, mut b_rx) = Link::connect_local(&data).await.unwrap();
    let Opened::Replay { events, .. } = open(&b, &t.id, Some(first)).await else { panic!("expected a replay") };
    assert_eq!(events.iter().map(|e| e.seq.unwrap()).collect::<Vec<_>>(), vec![first + 1, first + 2]);

    // New events reach both, once each, in order; streamed text pieces are joined.
    let item = ItemId("item-1".into());
    host.inject(&t.id, &[
        notice(4),
        Event::TextDelta { item: item.clone(), text: "Hel".into() },
        Event::TextDelta { item: item.clone(), text: "lo".into() },
        notice(5),
    ]).unwrap();
    for rx in [&mut a_rx, &mut b_rx] {
        assert_eq!(next(rx).await.seq, Some(first + 3));
        let delta = next(rx).await;
        assert_eq!((delta.seq, delta.event), (None, Event::TextDelta { item: item.clone(), text: "Hello".into() }));
        assert_eq!(next(rx).await.seq, Some(first + 4));
    }

    // B goes away and misses two events; reconnecting with its last number gets exactly those.
    drop((b, b_rx));
    host.inject(&t.id, &[notice(6), notice(7)]).unwrap();
    let (b2, _b2_rx) = Link::connect_local(&data).await.unwrap();
    let Opened::Replay { events, .. } = open(&b2, &t.id, Some(first + 4)).await else { panic!("expected a replay") };
    assert_eq!(events.iter().map(|e| e.seq.unwrap()).collect::<Vec<_>>(), vec![first + 5, first + 6]);
    assert_eq!(next(&mut a_rx).await.seq, Some(first + 5), "A kept receiving all along");

    // A wrong token is refused.
    let url = serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(data.join("link.json")).unwrap()).unwrap()["url"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(Link::connect(&url, "wrong").await.is_err());

    let _ = stop.send(());
}
