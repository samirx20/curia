//! Start-up recovery. No CLI survives a host restart, so whatever was running is settled as
//! events (CARRY_OVER §19): waiting approvals expire, open subagents end, running turns are
//! interrupted. Written through `record`, so the tables stay in step with the log.
use super::Store;
use anyhow::Result;
use curia_core::{Event, Resolution, TurnOutcome};
use std::collections::BTreeMap;

/// What recovery settled, per thread.
#[derive(Debug, Default, PartialEq)]
pub struct Recovered {
    pub threads: Vec<String>,
    pub turns: usize,
    pub requests: usize,
    pub subagents: usize,
    /// Threads whose turn was cut off.
    pub cut: Vec<String>,
    /// Threads that had an approval or question waiting when the host stopped.
    pub waiting: Vec<String>,
}

impl Store {
    pub fn recover(&self) -> Result<Recovered> {
        let mut per_thread: BTreeMap<String, Vec<Event>> = BTreeMap::new();
        let mut done = Recovered::default();
        {
            let conn = self.conn();
            let mut q = conn.prepare("SELECT thread_id, request_id FROM requests WHERE status = 'pending' ORDER BY created_at")?;
            for row in q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
                let (thread, request_id) = row?;
                if !done.waiting.contains(&thread) {
                    done.waiting.push(thread.clone());
                }
                per_thread.entry(thread).or_default().push(Event::ApprovalResolved { request_id, resolution: Resolution::Expired });
                done.requests += 1;
            }
            let mut q = conn.prepare("SELECT thread_id, task_id FROM subagents WHERE status = 'running' ORDER BY started_at")?;
            for row in q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
                let (thread, task_id) = row?;
                per_thread.entry(thread).or_default().push(Event::SubagentEnded { task_id, status: "host_restarted".into() });
                done.subagents += 1;
            }
            let mut q = conn.prepare("SELECT thread_id FROM turns WHERE status = 'running'")?;
            for row in q.query_map([], |r| r.get::<_, String>(0))? {
                let thread = row?;
                done.cut.push(thread.clone());
                let events = per_thread.entry(thread).or_default();
                events.push(Event::Notice { text: "The host restarted during this turn, so it was cut off.".into() });
                events.push(Event::TurnFinished { outcome: TurnOutcome::Interrupted, cost_usd: None });
                done.turns += 1;
            }
        }
        for (thread, events) in &per_thread {
            self.record(thread, events)?;
        }
        // A thread marked running with no running turn (a crash between writes) is idle too.
        self.conn().execute("UPDATE threads SET status = 'idle' WHERE status = 'running'", [])?;
        done.threads = per_thread.into_keys().collect();
        Ok(done)
    }
}
