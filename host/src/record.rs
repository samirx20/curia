//! Recording a CLI conversation and replaying it. A recording is JSONL: every line the CLI
//! wrote (`out`, `err`), every line Curia wrote (`in`), how it exited, and the events the
//! adapter produced live. Replaying feeds the same lines through the mapper; a test then
//! checks it produces the same events. Tests never spend plan usage.
use crate::claude::Mapper;
use anyhow::{Context, Result};
use curia_core::{Decision, Event};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, Write};
use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "dir", rename_all = "snake_case")]
pub enum Entry {
    /// How many item ids the thread had used before this process (older recordings: 0), and
    /// which CLI it was (older recordings: Claude).
    Start {
        ms: u64,
        items_so_far: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cli: Option<String>,
    },
    In { ms: u64, line: String },
    Out { ms: u64, line: String },
    Err { ms: u64, line: String },
    Exit { ms: u64, code: Option<i32> },
    Event { ms: u64, event: Event },
}

pub struct Recorder {
    file: Mutex<std::fs::File>,
    start: Instant,
}

impl Recorder {
    pub fn create(path: &Path) -> Result<Recorder> {
        let file = std::fs::File::create(path).with_context(|| format!("could not create {}", path.display()))?;
        Ok(Recorder { file: Mutex::new(file), start: Instant::now() })
    }

    pub fn ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    /// Writes one entry. A recording that can't be written is an error, not a silent gap.
    pub fn write(&self, entry: &Entry) -> Result<()> {
        let mut f = self.file.lock().unwrap_or_else(|p| p.into_inner());
        serde_json::to_writer(&mut *f, entry)?;
        f.write_all(b"\n")?;
        Ok(())
    }
}

pub fn read(path: &Path) -> Result<Vec<Entry>> {
    let file = std::fs::File::open(path).with_context(|| format!("could not open {}", path.display()))?;
    let mut entries = Vec::new();
    for (n, line) in std::io::BufReader::new(file).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        entries.push(serde_json::from_str(&line).with_context(|| format!("{} line {}", path.display(), n + 1))?);
    }
    Ok(entries)
}

/// Runs a recording's CLI lines through the right adapter's mapper and returns the events.
pub fn replay(entries: &[Entry]) -> Result<Vec<Event>> {
    let cli = entries.iter().find_map(|e| match e {
        Entry::Start { cli, .. } => cli.clone(),
        _ => None,
    });
    match cli.as_deref() {
        Some("codex") => crate::codex::replay::replay(entries),
        Some("agy") => crate::agy::mapper::replay(entries),
        _ => replay_claude(entries),
    }
}

fn replay_claude(entries: &[Entry]) -> Result<Vec<Event>> {
    let mut m = Mapper::new();
    let mut events = Vec::new();
    for e in entries {
        match e {
            Entry::Start { items_so_far, .. } => m = Mapper::starting_at(*items_so_far),
            Entry::Out { line, .. } => {
                let v: Value = serde_json::from_str(line).with_context(|| format!("unreadable CLI line: {line}"))?;
                events.extend(m.handle(&v).events);
            }
            Entry::In { line, .. } => {
                let v: Value = serde_json::from_str(line)?;
                match v["type"].as_str() {
                    Some("user") => events.extend(m.begin_turn(&user_text(&v))),
                    Some("control_response") if v["response"]["subtype"] == "success" => {
                        let r = &v["response"];
                        let request_id = r["request_id"].as_str().unwrap_or("");
                        let (_, event) = m.resolve(request_id, &decision_of(&r["response"]))?;
                        events.push(event);
                    }
                    _ => {}
                }
            }
            Entry::Exit { code, .. } => events.extend(m.on_exit(*code, "")),
            Entry::Err { .. } | Entry::Event { .. } => {}
        }
    }
    Ok(events)
}

/// The events the adapter produced while the recording was made.
pub fn recorded_events(entries: &[Entry]) -> Vec<Event> {
    entries.iter().filter_map(|e| if let Entry::Event { event, .. } = e { Some(event.clone()) } else { None }).collect()
}

pub fn user_text(v: &Value) -> String {
    match &v["message"]["content"] {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    /// Every committed recording of a real CLI run must replay to the events it recorded.
    #[test]
    fn fixtures_replay_to_their_recorded_events() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let mut checked = 0;
        for f in std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()) {
            if f.extension().is_some_and(|x| x == "jsonl") {
                let entries = super::read(&f).unwrap();
                let recorded = super::recorded_events(&entries);
                assert!(!recorded.is_empty(), "{} has no recorded events", f.display());
                assert_eq!(super::replay(&entries).unwrap(), recorded, "{}", f.display());
                checked += 1;
            }
        }
        assert!(checked > 0, "no fixtures in {}", dir.display());
    }
}

fn decision_of(body: &Value) -> Decision {
    if body["behavior"] == "deny" {
        return Decision::Deny { message: body["message"].as_str().map(str::to_string) };
    }
    match body["updatedInput"]["answers"].as_object() {
        Some(answers) => Decision::Answer { answers: answers.clone() },
        None => Decision::Allow,
    }
}
