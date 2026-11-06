//! `curia-host dev`: one thread driven from the console through the host, for trying it
//! against the real CLI. Type a message to send it; commands start with `/`.
mod show;

use crate::host::Host;
use crate::store::NewThread;
use anyhow::{Context, Result, bail};
use curia_core::{Decision, Event};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};

const HELP: &str = "Type a message to send it. Commands: /allow, /deny [reason], /answer <json>, /stop, /effort <level>, /turns, /diff <n>, /restore <n>, /quit";

pub enum Pick {
    New(NewThread),
    Thread(String),
    Last,
}

pub async fn run(data: PathBuf, pick: Pick) -> Result<()> {
    let (host, recovered) = Host::open(&data)?;
    if !recovered.threads.is_empty() {
        println!("Recovered after a restart: {} turn(s) cut off, {} approval(s) expired, {} subagent(s) ended.",
            recovered.turns, recovered.requests, recovered.subagents);
    }
    crate::mcp::start(host.clone()).await?;
    let row = match pick {
        Pick::New(t) => host.create_thread(&t)?,
        Pick::Thread(id) => host.store.thread_get(&id)?,
        Pick::Last => host.store.threads()?.into_iter().next().context("there are no threads yet")?,
    };
    println!("Thread {} in {} (team folder {})\n{HELP}", row.id, row.cwd, row.config_dir);
    let mut history = show::History::default();
    for s in host.store.events_after(&row.id, 0)? {
        history.show(&s.event);
    }
    let id = row.id.clone();
    let pending: Arc<Mutex<Option<String>>> = Arc::default();

    let (waiting, mut events, thread) = (pending.clone(), host.subscribe(), id.clone());
    let printer = tokio::spawn(async move {
        let mut last_seq = None;
        while let Ok(e) = events.recv().await {
            if e.thread_id != thread {
                continue;
            }
            last_seq = e.seq.or(last_seq);
            if let Event::ApprovalRequested { request_id, .. } = &e.event {
                *waiting.lock().unwrap() = Some(request_id.clone());
            }
            show::show(&e.event);
            if matches!(e.event, Event::Exited { .. }) {
                break;
            }
        }
        last_seq
    });

    let mut input = BufReader::new(tokio::io::stdin()).lines();
    while let Some(line) = input.next_line().await? {
        let line = line.trim();
        if line == "/quit" {
            break;
        }
        if let Err(e) = command(&host, &id, &pending, line).await {
            println!("! {e:#}");
        }
    }
    let was_live = host.is_live(&id);
    host.close_all().await;
    if was_live {
        if let Ok(Ok(Some(seq))) = tokio::time::timeout(std::time::Duration::from_secs(6), printer).await {
            println!("Last stored event: #{seq}");
        }
    }
    let row = host.store.thread_get(&id)?;
    println!("Thread {} is {}; items used {}.", row.id, row.status, row.items_used);
    Ok(())
}

async fn command(host: &Arc<Host>, id: &str, pending: &Mutex<Option<String>>, line: &str) -> Result<()> {
    let (cmd, rest) = line.split_once(' ').unwrap_or((line, ""));
    let take = || pending.lock().unwrap().take().context("nothing is waiting for an answer");
    let number = || rest.trim().parse::<usize>().context("give a turn number (see /turns)");
    match cmd {
        "" => Ok(()),
        "/stop" => host.interrupt(id).await,
        "/effort" => host.set_effort(id, rest).await,
        "/allow" => host.answer(id, &take()?, &Decision::Allow).await,
        "/deny" => {
            let message = Some(rest.to_string()).filter(|m| !m.is_empty());
            host.answer(id, &take()?, &Decision::Deny { message }).await
        }
        "/answer" => {
            let answers = serde_json::from_str(rest).context("/answer takes a JSON object of question → answer")?;
            host.answer(id, &take()?, &Decision::Answer { answers }).await
        }
        "/turns" => {
            for (n, t) in host.store.turns(id)?.iter().enumerate() {
                let prompt = t.prompt.as_deref().unwrap_or("(started by the CLI)");
                let checkpoints = if t.checkpoint_before.is_some() { "checkpointed" } else { "no checkpoint" };
                println!("{n}: {} — {prompt} ({checkpoints})", t.status);
            }
            Ok(())
        }
        "/diff" => {
            let d = host.turn_diff(id, number()?).await?;
            for f in &d.files {
                println!("{} +{} -{}{}", f.path, f.added, f.removed, if f.binary { " (binary)" } else { "" });
            }
            println!("{}{}", d.patch, if d.truncated { "\n… (cut)" } else { "" });
            Ok(())
        }
        "/restore" => {
            let safety = host.restore_before(id, number()?).await?;
            println!("Restored. The state before the restore is kept as checkpoint {safety}.");
            Ok(())
        }
        c if c.starts_with('/') => bail!("unknown command. {HELP}"),
        _ => host.deliver(id, None, line).await.map(|_| ()),
    }
}
