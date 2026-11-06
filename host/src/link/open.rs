//! What a client gets when it opens a thread: exactly the events after its cursor, or a fresh
//! snapshot when that gap is too big or the cursor is ahead of the host (CARRY_OVER §8, §17).
use crate::store::{Store, Stored, ThreadRow};
use anyhow::Result;
use curia_core::protocol::{Opened, ThreadInfo, WireEvent};

/// A replay larger than this becomes a snapshot instead.
const REPLAY_MAX_EVENTS: u32 = 128;
const REPLAY_MAX_BYTES: usize = 1024 * 1024;
/// A snapshot holds the newest events up to these limits; older ones are paged on request.
const SNAPSHOT_ROWS: u32 = 75;
const SNAPSHOT_BYTES: usize = 1024 * 1024;

pub fn info(row: &ThreadRow) -> ThreadInfo {
    ThreadInfo {
        id: row.id.clone(),
        title: row.title.clone(),
        cli: row.cli.clone(),
        cwd: row.cwd.clone(),
        model: row.model.clone(),
        status: row.status.clone(),
        updated_at: row.updated_at,
    }
}

pub fn wire(thread_id: &str, s: Stored) -> WireEvent {
    WireEvent { thread_id: thread_id.to_string(), seq: Some(s.seq), event: s.event }
}

fn size(s: &Stored) -> usize {
    serde_json::to_string(&s.event).map_or(0, |j| j.len())
}

pub fn open(store: &Store, thread_id: &str, after_seq: Option<i64>) -> Result<Opened> {
    let thread = info(&store.thread_get(thread_id)?);
    if let Some(after) = after_seq.filter(|a| *a <= store.max_seq().unwrap_or(0)) {
        let page = store.events_page(thread_id, after, REPLAY_MAX_EVENTS + 1)?;
        if page.len() <= REPLAY_MAX_EVENTS as usize && page.iter().map(size).sum::<usize>() <= REPLAY_MAX_BYTES {
            let events = page.into_iter().map(|s| wire(thread_id, s)).collect();
            return Ok(Opened::Replay { thread, events });
        }
    }
    let mut tail = store.events_before(thread_id, i64::MAX, SNAPSHOT_ROWS + 1)?;
    let mut has_older = tail.len() > SNAPSHOT_ROWS as usize;
    if has_older {
        tail.remove(0);
    }
    let mut bytes: usize = tail.iter().map(size).sum();
    while bytes > SNAPSHOT_BYTES && tail.len() > 1 {
        bytes -= size(&tail.remove(0));
        has_older = true;
    }
    let events = tail.into_iter().map(|s| wire(thread_id, s)).collect();
    Ok(Opened::Snapshot { thread, events, has_older })
}

/// The newest event number a client holds after receiving `opened`.
pub fn last_seq(opened: &Opened) -> Option<i64> {
    let events = match opened {
        Opened::Replay { events, .. } | Opened::Snapshot { events, .. } => events,
    };
    events.iter().filter_map(|e| e.seq).max()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::NewThread;
    use curia_core::Event;

    fn store_with(n: usize) -> (Store, String) {
        let store = Store::in_memory();
        let t = store
            .thread_create(&NewThread {
                title: "t".into(), cli: "claude".into(), config_dir: "c".into(), cwd: ".".into(),
                model: None, effort: None, permission_mode: "manual".into(), ..Default::default()
            })
            .unwrap();
        let events: Vec<Event> = (0..n).map(|i| Event::Notice { text: format!("n{i}") }).collect();
        store.record(&t.id, &events).unwrap();
        (store, t.id)
    }

    #[test]
    fn a_small_gap_replays_exactly() {
        let (store, id) = store_with(10);
        match open(&store, &id, Some(4)).unwrap() {
            Opened::Replay { events, .. } => {
                assert_eq!(events.iter().map(|e| e.seq.unwrap()).collect::<Vec<_>>(), (5..=10).collect::<Vec<_>>());
            }
            other => panic!("expected a replay, got {other:?}"),
        }
    }

    #[test]
    fn a_big_gap_a_new_client_or_a_cursor_ahead_gets_a_snapshot() {
        let (store, id) = store_with(200);
        for after in [Some(1), None, Some(999)] {
            match open(&store, &id, after).unwrap() {
                Opened::Snapshot { events, has_older, .. } => {
                    assert_eq!(events.len(), 75);
                    assert_eq!(events.last().unwrap().seq, Some(200));
                    assert!(has_older);
                }
                other => panic!("expected a snapshot for {after:?}, got {other:?}"),
            }
        }
    }
}
