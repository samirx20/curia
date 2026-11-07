//! `curia`: a command-line client that talks to a running host only over the link.
//!   curia --data <dir> threads
//!   curia --data <dir> teams | team-add --name N --config-dir <dir> [--cli claude] [--model M] [--effort E]
//!   curia --data <dir> new --team N --cwd <dir> [--title T] [--model M] [--effort E] [--mode M]
//!   curia --data <dir> move (--thread <id> | --last) --team N | settings | set --key K --value V | inbox
//!   curia --data <dir> autostart (--on | --off)
//!   curia --data <dir> answer --thread <id> --request <id> [--deny]
//!   curia --data <dir> pair [--role owner|approve|view] [--minutes N] | devices | unpair --device <id>
//!   curia --data <dir> terminals (--thread <id> | --last) | terminal --id <id> [--type TEXT]
//!   curia --data <dir> watch (--thread <id> | --last) [--after N]
//!   curia --data <dir> chat (--thread <id> | --last)
use anyhow::{Context, Result, bail};
use curia_client::{Incoming, Link, parse};
use curia_core::protocol::{Call, NewTeamParams, NewThreadParams, Opened, TeamInfo, ThreadInfo, WireEvent};
use curia_core::{Decision, Event};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};

const USAGE: &str = "usage: curia --data <dir> (threads | teams | team-add --name N --config-dir <dir> | new --team N --cwd <dir> [--model M] [--effort E] [--mode M] | move (--thread <id> | --last) --team N | settings | set --key K --value V | autostart (--on | --off) | inbox | answer --thread <id> --request <id> [--deny] | pair [--role R] | devices | unpair --device <id> | terminals (--thread <id> | --last) | terminal --id <id> [--type TEXT] | watch (--thread <id> | --last) [--after N] | chat (--thread <id> | --last))";

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut flags: HashMap<String, String> = HashMap::new();
    let mut command = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--last" | "--deny" | "--on" | "--off" => {
                flags.insert(a[2..].to_string(), String::new());
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
                team: get("team").context("--team is required (see `curia teams`)")?,
                cwd: get("cwd").context("--cwd is required")?,
                model: get("model"),
                effort: get("effort"),
                permission_mode: get("mode").unwrap_or_else(|| "ask".into()),
            };
            let t: ThreadInfo = parse(link.call(Call::ThreadCreate { thread }).await?)?;
            println!("{}", t.id);
            Ok(())
        }
        Some("teams") => {
            for t in parse::<Vec<serde_json::Value>>(link.call(Call::TeamsList).await?)? {
                let pct = |k: &str| t[k].as_f64().map_or("?".to_string(), |v| format!("{v}%"));
                let out = if t["out_of_quota_until_ms"].is_null() { "" } else { "  OUT OF QUOTA" };
                println!("{}  {}  {}  5h left {}  week left {}{out}", t["name"].as_str().unwrap_or(""), t["cli"].as_str().unwrap_or(""),
                    t["model"].as_str().unwrap_or(""), pct("five_hour_percent_left"), pct("weekly_percent_left"));
            }
            Ok(())
        }
        Some("move") => {
            let team = flags.get("team").context("--team is required")?.clone();
            link.call(Call::ThreadMove { thread_id: thread_id(&link, &flags).await?, team }).await?;
            println!("moved");
            Ok(())
        }
        Some("settings") => {
            println!("{}", serde_json::to_string_pretty(&link.call(Call::SettingsGet).await?)?);
            Ok(())
        }
        Some("set") => {
            let key = flags.get("key").context("--key is required")?;
            let raw = flags.get("value").context("--value is required")?;
            let value = serde_json::from_str(raw).unwrap_or(serde_json::Value::String(raw.clone()));
            let mut settings = link.call(Call::SettingsGet).await?;
            settings[key.as_str()] = value;
            println!("{}", serde_json::to_string_pretty(&link.call(Call::SettingsSet { settings }).await?)?);
            Ok(())
        }
        Some("autostart") => {
            let enabled = match (flags.contains_key("on"), flags.contains_key("off")) {
                (true, false) => true,
                (false, true) => false,
                _ => bail!("autostart takes --on or --off"),
            };
            link.call(Call::AutostartSet { enabled }).await?;
            println!("the host {} when you sign in", if enabled { "now starts" } else { "no longer starts" });
            Ok(())
        }
        Some("pair") => {
            let role = flags.get("role").cloned().unwrap_or_else(|| "approve".into());
            let minutes = flags.get("minutes").map(|m| m.parse()).transpose().context("--minutes takes a number")?;
            let r = link.call(Call::PairCode { role, minutes }).await?;
            let code = r["code"].as_str().unwrap_or("");
            // The code rides in the address's fragment, which browsers never send to a server.
            // A phone uses the host's Tailscale address when it has one.
            let page = match remote_url(&data)? {
                Some(remote) => remote,
                None => data_url(&data)?.replace("ws://", "http://").replace("/ws", "/"),
            };
            let left = (r["expires_at_ms"].as_i64().unwrap_or(0) - std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_millis() as i64) / 60_000;
            println!("Pairing code {code} ({} device), good for {} minute(s).", r["role"].as_str().unwrap_or(""), left.max(1));
            println!("Open on the device: {page}#pair={code}");
            Ok(())
        }
        Some("terminals") => {
            let thread_id = thread_id(&link, &flags).await?;
            for t in parse::<Vec<serde_json::Value>>(link.call(Call::TerminalList { thread_id }).await?)? {
                println!("{}  {}{}", t["id"].as_str().unwrap_or(""), t["title"].as_str().unwrap_or(""), if t["exited"] == true { " (ended)" } else { "" });
            }
            Ok(())
        }
        Some("terminal") => {
            // A second client on the same terminal: optionally types a line, then prints the screen.
            let id = flags.get("id").cloned().context("--id is required")?;
            if let Some(text) = flags.get("type") {
                link.call(Call::TerminalInput { id: id.clone(), data: format!("{text}\r") }).await?;
                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
            }
            let v = link.call(Call::TerminalOpen { id }).await?;
            let s = &v["screen"];
            for line in s["lines"].as_array().into_iter().flatten() {
                println!("{}", line.as_array().into_iter().flatten().map(|r| r["t"].as_str().unwrap_or("")).collect::<String>());
            }
            println!("-- {} {}x{}, cursor at row {} column {}", v["title"].as_str().unwrap_or(""), s["cols"], s["rows"], s["cursor"][0], s["cursor"][1]);
            Ok(())
        }
        Some("devices") => {
            for d in parse::<Vec<serde_json::Value>>(link.call(Call::DevicesList).await?)? {
                println!("{}  {:7}  {}", d["id"].as_str().unwrap_or(""), d["role"].as_str().unwrap_or(""), d["name"].as_str().unwrap_or(""));
            }
            Ok(())
        }
        Some("unpair") => {
            let device_id = flags.get("device").cloned().context("--device is required")?;
            link.call(Call::DeviceRevoke { device_id }).await?;
            Ok(())
        }
        Some("inbox") => {
            for r in parse::<Vec<serde_json::Value>>(link.call(Call::InboxList).await?)? {
                println!("{}  {}  {}  in \"{}\" ({})", r["request_id"].as_str().unwrap_or(""), r["kind"].as_str().unwrap_or(""),
                    r["tool_name"].as_str().unwrap_or(""), r["thread_title"].as_str().unwrap_or(""), r["thread_id"].as_str().unwrap_or(""));
            }
            Ok(())
        }
        Some("answer") => {
            let thread_id = flags.get("thread").cloned().context("--thread is required")?;
            let request_id = flags.get("request").cloned().context("--request is required")?;
            let decision = if flags.contains_key("deny") { Decision::Deny { message: None } } else { Decision::Allow };
            link.call(Call::ThreadAnswer { thread_id, request_id, decision, signature: None }).await?;
            Ok(())
        }
        Some("team-add") => {
            let get = |k: &str| flags.get(k).cloned();
            let team = NewTeamParams {
                name: get("name").context("--name is required")?,
                cli: get("cli").unwrap_or_else(|| "claude".into()),
                config_dir: get("config-dir").context("--config-dir is required")?,
                model: get("model"),
                effort: get("effort"),
            };
            let t: TeamInfo = parse(link.call(Call::TeamCreate { team }).await?)?;
            println!("added team {}", t.name);
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
            Incoming::Activity { .. } | Incoming::Terminal(_) => {}
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
        while let Some(incoming) = events.recv().await {
            let Incoming::Event(e) = incoming else { continue }; // closing and activity notices
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
            "/allow" => Call::ThreadAnswer { thread_id: id.clone(), request_id: take()?, decision: Decision::Allow, signature: None },
            "/deny" => Call::ThreadAnswer { thread_id: id.clone(), request_id: take()?, decision: Decision::Deny { message: None }, signature: None },
            undo if undo.starts_with("/undo ") => Call::MergeUndo { merge_id: undo[6..].trim().to_string() },
            text => Call::ThreadSend { thread_id: id.clone(), text: text.to_string() },
        };
        if let Err(e) = link.call(call).await {
            println!("! {e:#}");
        }
    }
    Ok(())
}

/// The link's address, from the host's `link.json`.
fn data_url(data: &std::path::Path) -> Result<String> {
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(data.join("link.json"))?)?;
    Ok(v["url"].as_str().context("link.json has no url")?.to_string())
}

/// The host's HTTPS address on the tailnet, if it serves one.
fn remote_url(data: &std::path::Path) -> Result<Option<String>> {
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(data.join("link.json"))?)?;
    Ok(v["remote_url"].as_str().map(str::to_string))
}
