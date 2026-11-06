//! `curia`: a command-line client that talks to a running host only over the link.
//!   curia --data <dir> threads
//!   curia --data <dir> new --config-dir <dir> --cwd <dir> [--model M] [--effort E] [--mode M]
//!   curia --data <dir> watch (--thread <id> | --last) [--after N]
//!   curia --data <dir> chat (--thread <id> | --last)
use anyhow::{Context, Result, bail};
use curia_client::{Incoming, Link, parse};
use curia_core::protocol::{Call, NewThreadParams, Opened, ThreadInfo, WireEvent};
use curia_core::{Decision, Event};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};

const USAGE: &str = "usage: curia --data <dir> (threads | new --config-dir <dir> --cwd <dir> [--model M] [--effort E] [--mode M] | watch (--thread <id> | --last) [--after N] | chat (--thread <id> | --last))";

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut flags: HashMap<String, String> = HashMap::new();
    let mut command = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--last" => {
                flags.insert("last".into(), String::new());
            }
            f if f.starts_with("--") => {
                flags.insert(f[2..].to_string(), it.next().with_context(|| format!("{f} needs a value"))?.clone());
            }
            c if command.is_none() => command = Some(c.to_string()),
            other => bail!("unexpected {other}\n{USAGE}"),
        }
    }
    let data = PathBuf::from(flags.get("data").context(USAGE)?);
    let (link, events) = Link::connect_local(&data).await?;
    match command.as_deref() {
        Some("threads") => {
            for t in parse::<Vec<ThreadInfo>>(link.call(Call::ThreadsList).await?)? {
                println!("{}  {}  {}  {}", t.id, t.status, t.cwd, t.title);
            }
            Ok(())
        }
        Some("new") => {
            let get = |k: &str| flags.get(k).cloned();
            let thread = NewThreadParams {
                title: get("title").unwrap_or_else(|| "new thread".into()),
                cli: "claude".into(),
                config_dir: get("config-dir").context("--config-dir is required")?,
                cwd: get("cwd").context("--cwd is required")?,
                model: get("model"),
                effort: get("effort"),
                permission_mode: get("mode").unwrap_or_else(|| "manual".into()),
            };
            let t: ThreadInfo = parse(link.call(Call::ThreadCreate { thread }).await?)?;
            println!("{}", t.id);
            Ok(())
        }
        Some("watch") => watch(&link, events, thread_id(&link, &flags).await?, flags.get("after").map(|a| a.parse()).transpose()?).await,
        Some("chat") => chat(&link, events, thread_id(&link, &flags).await?).await,
        _ => bail!("{USAGE}"),
    }
}

async fn thread_id(link: &Link, flags: &HashMap<String, String>) -> Result<String> {
    if let Some(id) = flags.get("thread") {
        return Ok(id.clone());
    }
    let threads: Vec<ThreadInfo> = parse(link.call(Call::ThreadsList).await?)?;
    Ok(threads.into_iter().next().context("the host has no threads yet")?.id)
}

/// Opens the thread and returns the events to show first.
async fn open(link: &Link, thread_id: &str, after_seq: Option<i64>) -> Result<Vec<WireEvent>> {
    let opened: Opened = parse(link.call(Call::ThreadOpen { thread_id: thread_id.into(), after_seq }).await?)?;
    Ok(match opened {
        Opened::Replay { events, .. } => events,
        Opened::Snapshot { events, has_older, .. } => {
            println!("(snapshot{})", if has_older { "; older history not shown" } else { "" });
            events
        }
    })
}

/// Prints every event with its number until the connection ends; then the last number seen.
async fn watch(link: &Link, mut events: tokio::sync::mpsc::UnboundedReceiver<Incoming>, id: String, after: Option<i64>) -> Result<()> {
    let mut last = after;
    for e in open(link, &id, after).await? {
        last = e.seq.or(last);
        println!("#{} {}", e.seq.unwrap_or(0), serde_json::to_string(&e.event)?);
    }
    while let Some(i) = events.recv().await {
        match i {
            Incoming::Event(e) => {
                last = e.seq.or(last);
                let n = e.seq.map_or("live".to_string(), |s| format!("#{s}"));
                println!("{n} {}", serde_json::to_string(&e.event)?);
            }
            Incoming::Closing(reason) => println!("host closed the connection: {reason}"),
        }
    }
    println!("last event #{}", last.unwrap_or(0));
    Ok(())
}

async fn chat(link: &Link, mut events: tokio::sync::mpsc::UnboundedReceiver<Incoming>, id: String) -> Result<()> {
    for e in open(link, &id, None).await? {
        println!("#{} {}", e.seq.unwrap_or(0), serde_json::to_string(&e.event)?);
    }
    let pending: Arc<Mutex<Option<String>>> = Arc::default();
    let waiting = pending.clone();
    tokio::spawn(async move {
        while let Some(Incoming::Event(e)) = events.recv().await {
            if let Event::ApprovalRequested { request_id, .. } = &e.event {
                *waiting.lock().unwrap() = Some(request_id.clone());
            }
            match &e.event {
                Event::TextDelta { text, .. } => print!("{text}"),
                other => println!("\n#{} {}", e.seq.unwrap_or(0), serde_json::to_string(other).unwrap_or_default()),
            }
        }
    });
    let mut input = BufReader::new(tokio::io::stdin()).lines();
    while let Some(line) = input.next_line().await? {
        let line = line.trim().to_string();
        let take = || pending.lock().unwrap().take().context("nothing is waiting for an answer");
        let call = match line.as_str() {
            "" => continue,
            "/quit" => break,
            "/stop" => Call::ThreadInterrupt { thread_id: id.clone() },
            "/allow" => Call::ThreadAnswer { thread_id: id.clone(), request_id: take()?, decision: Decision::Allow },
            "/deny" => Call::ThreadAnswer { thread_id: id.clone(), request_id: take()?, decision: Decision::Deny { message: None } },
            text => Call::ThreadSend { thread_id: id.clone(), text: text.to_string() },
        };
        if let Err(e) = link.call(call).await {
            println!("! {e:#}");
        }
    }
    Ok(())
}
