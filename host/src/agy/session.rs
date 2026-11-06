//! One live `agy` in stream-json mode on `LineProcess`. A message is one input line and runs
//! one turn. agy has no interrupt and can't change effort mid-process, so both close it; the
//! host then resumes the conversation (`--conversation`) when the next message arrives.
use super::Launch;
use super::mapper::Mapper;
use crate::process::{LineProcess, Output};
use crate::record::Recorder;
use anyhow::{Result, bail};
use curia_core::{Decision, Event, UsageWindow};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// The input line for one message; the first one carries the role's instructions ahead of it.
pub fn user_line(text: &str, instructions: Option<&str>) -> Value {
    let content = match instructions {
        Some(i) => format!("{OPEN}\n{i}\n{CLOSE}\n\n{text}"),
        None => text.to_string(),
    };
    json!({"event": "user", "message": {"content": content}})
}

/// The user's own text in an input line, without the instructions ahead of it.
pub fn user_text(line: &Value) -> &str {
    let content = line["message"]["content"].as_str().unwrap_or("");
    match content.strip_prefix(OPEN).and_then(|rest| rest.split_once(&format!("{CLOSE}\n\n"))) {
        Some((_, text)) => text,
        None => content,
    }
}

const OPEN: &str = "<curia-instructions>";
const CLOSE: &str = "</curia-instructions>";

/// The note recorded when Curia stops a turn by closing agy (agy never sees it).
pub fn stop_note() -> Value {
    json!({"curia": "stop"})
}

pub struct AgySession {
    proc: Arc<LineProcess>,
    mapper: Mutex<Mapper>,
    instructions: Mutex<Option<String>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl AgySession {
    pub async fn start(exe: &Path, launch: &Launch, items_so_far: u64, recorder: Option<Recorder>) -> Result<(Arc<AgySession>, mpsc::UnboundedReceiver<Event>)> {
        let (tx, rx) = mpsc::unbounded_channel();
        let (proc, out) = LineProcess::start(launch.command(exe), "Antigravity", "agy", recorder, items_so_far, tx)?;
        // A resumed conversation already has its instructions.
        let instructions = if launch.resume.is_none() { launch.instructions.clone() } else { None };
        let s = Arc::new(AgySession { proc, mapper: Mutex::new(Mapper::starting_at(items_so_far)), instructions: Mutex::new(instructions) });
        tokio::spawn(s.clone().read(out));
        Ok((s, rx))
    }

    async fn read(self: Arc<Self>, mut out: Output) {
        while let Some(line) = out.lines.recv().await {
            match serde_json::from_str::<Value>(&line) {
                Ok(v) => {
                    let events = lock(&self.mapper).handle(&v);
                    self.proc.emit(events);
                }
                Err(_) => self.proc.notice(format!("Unreadable line from Antigravity: {line}")),
            }
        }
        let code = out.exit.await.ok().flatten();
        let tail = self.proc.exited(code);
        let events = lock(&self.mapper).on_exit(code, &tail);
        self.proc.emit(events);
    }

    /// Runs one turn. agy queues a line sent mid-turn and runs it next, like Claude.
    pub async fn send(&self, text: &str) -> Result<()> {
        let instructions = lock(&self.instructions).take();
        let events = lock(&self.mapper).begin_turn(text);
        self.proc.emit(events);
        self.proc.write(&user_line(text, instructions.as_deref())).await
    }

    pub async fn answer(&self, _request_id: &str, _decision: &Decision) -> Result<()> {
        bail!("Antigravity never asks for approval; there is nothing to answer")
    }

    /// agy has no interrupt: the process is closed and the next message resumes.
    pub async fn interrupt(&self) -> Result<()> {
        lock(&self.mapper).stopping();
        self.proc.note(&stop_note());
        self.proc.close().await;
        Ok(())
    }

    /// Effort is fixed per process: close it between turns, and the next message restarts
    /// with the thread's new effort.
    pub async fn set_effort(&self, _level: &str) -> Result<()> {
        if lock(&self.mapper).in_turn() {
            bail!("Antigravity can change effort only between turns; try again when this turn ends");
        }
        self.proc.close().await;
        Ok(())
    }

    /// Not available headless: agy 1.5.2 treats `-p=/usage` as a prompt (checked 2026-11-07).
    pub async fn usage(&self) -> Result<Vec<UsageWindow>> {
        Ok(Vec::new())
    }

    pub fn pid(&self) -> Option<u32> {
        self.proc.pid()
    }

    pub fn items_used(&self) -> u64 {
        lock(&self.mapper).items_used()
    }

    pub async fn close(&self) {
        self.proc.close().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instructions_ride_ahead_of_the_first_message_and_are_stripped_for_display() {
        let line = user_line("fix the bug", Some("You are a team thread."));
        assert!(line["message"]["content"].as_str().unwrap().contains("You are a team thread."));
        assert_eq!(user_text(&line), "fix the bug");
        assert_eq!(user_text(&user_line("plain", None)), "plain");
    }
}
