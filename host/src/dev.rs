//! `curia-host dev`: one Claude Code thread driven from the console, for trying the adapter
//! against the real CLI. Type a message to send it; commands start with `/`.
use crate::claude::{ClaudeSession, Launch, find_claude};
use crate::record::Recorder;
use anyhow::Result;
use curia_core::{Decision, Event};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};

const HELP: &str = "Type a message to send it. Commands: /allow, /deny [reason], /answer <json>, /stop, /effort <level>, /quit";

pub async fn run(launch: Launch, record: Option<PathBuf>) -> Result<()> {
    let exe = find_claude()?;
    let recorder = record.as_deref().map(Recorder::create).transpose()?;
    println!("Claude: {}\nConfig folder: {}\nWorking folder: {}\n{HELP}", exe.display(), launch.config_dir.display(), launch.cwd.display());
    let (session, mut events) = ClaudeSession::start(&exe, &launch, 0, recorder).await?;
    let pending: Arc<Mutex<Option<String>>> = Arc::default();

    let waiting = pending.clone();
    let printer = tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            if let Event::ApprovalRequested { request_id, .. } = &event {
                *waiting.lock().unwrap() = Some(request_id.clone());
            }
            let exited = matches!(event, Event::Exited { .. });
            show(&event);
            if exited {
                break;
            }
        }
    });

    let mut input = BufReader::new(tokio::io::stdin()).lines();
    while let Some(line) = input.next_line().await? {
        let line = line.trim();
        let (cmd, rest) = line.split_once(' ').unwrap_or((line, ""));
        let result = match cmd {
            "" => Ok(()),
            "/quit" => break,
            "/stop" => session.interrupt().await,
            "/effort" => session.set_effort(rest).await,
            "/allow" => answer(&session, &pending, Decision::Allow).await,
            "/deny" => answer(&session, &pending, Decision::Deny { message: Some(rest.to_string()).filter(|m| !m.is_empty()) }).await,
            "/answer" => match serde_json::from_str(rest) {
                Ok(answers) => answer(&session, &pending, Decision::Answer { answers }).await,
                Err(e) => Err(anyhow::anyhow!("/answer takes a JSON object of question → answer: {e}")),
            },
            c if c.starts_with('/') => Err(anyhow::anyhow!("unknown command. {HELP}")),
            _ => session.send(line).await,
        };
        if let Err(e) = result {
            println!("! {e:#}");
        }
    }
    session.close().await;
    printer.await?;
    if let Some(id) = session.session_id() {
        println!("Session id (for --resume): {id}");
    }
    println!("Items used: {}", session.items_used());
    Ok(())
}

async fn answer(session: &ClaudeSession, pending: &Mutex<Option<String>>, decision: Decision) -> Result<()> {
    let id = pending.lock().unwrap().take().ok_or_else(|| anyhow::anyhow!("nothing is waiting for an answer"))?;
    session.answer(&id, &decision).await
}

fn show(event: &Event) {
    match event {
        Event::TextDelta { text, .. } => {
            print!("{text}");
            let _ = std::io::stdout().flush();
        }
        Event::TextDone { .. } => println!(),
        Event::TextStarted { kind, text, .. } => print!("\n[{kind:?}] {text}"),
        Event::ToolCall { name, input, parent_tool_use_id, .. } => {
            let by = if parent_tool_use_id.is_some() { " (subagent)" } else { "" };
            println!("[tool{by}] {name} {input}");
        }
        Event::ToolResult { output, is_error, .. } => {
            let first = output.lines().next().unwrap_or("");
            println!("  → {}{first}", if *is_error { "error: " } else { "" });
        }
        Event::ApprovalRequested { kind, tool_name, input, .. } => {
            println!("[asks: {kind:?}] {tool_name} {input}\n  answer with /allow, /deny or /answer");
        }
        other => println!("[{}]", serde_json::to_string(other).unwrap_or_default()),
    }
}
