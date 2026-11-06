//! Curia's host.
//!   curia-host serve --data <dir> [--port N] [--tailscale PORT] [--public-url URL]   run the host for clients (see the `curia` client)
//!   curia-host dev --data <dir> (--thread <id> | --last | --config-dir <dir> --cwd <dir> [--model M] [--effort E] [--mode M])
//!   curia-host replay <recording.jsonl>
//!   curia-host scrub <live recording> <fixture>
mod agy;
mod claude;
mod cli;
mod codex;
mod dev;
mod freedom;
mod git;
mod host;
mod link;
mod links;
mod mcp;
mod ports;
mod process;
mod push;
mod quota;
mod redact;
mod rules;
mod record;
mod scrub;
mod store;
mod term;
mod tls;

use anyhow::{Context, Result, bail};
use std::path::PathBuf;

const USAGE: &str = "usage:
  curia-host serve --data <dir> [--port N] [--tailscale PORT] [--public-url URL]
      --tailscale PORT: also serve the phone app over HTTPS on this machine's Tailscale name
      --public-url URL: where phones reach the host when something else serves it (tailscale serve)
  curia-host dev --data <dir> (--thread <id> | --last | --config-dir <dir> --cwd <dir> [--model M] [--effort E] [--mode M])
  curia-host replay <recording.jsonl>
  curia-host scrub <live recording> <fixture>";

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("serve") => serve(&args[1..]).await,
        Some("dev") => {
            let (data, pick) = dev_args(&args[1..])?;
            dev::run(data, pick).await
        }
        Some("replay") => replay(args.get(1).context(USAGE)?),
        Some("scrub") => {
            let (input, output) = (args.get(1).context(USAGE)?, args.get(2).context(USAGE)?);
            let n = scrub::scrub_file(input.as_ref(), output.as_ref())?;
            println!("ok: {n} entries written to {output}; it replays to the same events");
            Ok(())
        }
        _ => bail!("{USAGE}"),
    }
}

fn dev_args(args: &[String]) -> Result<(PathBuf, dev::Pick)> {
    let mut data = None;
    let mut pick = None;
    let mut t = store::NewThread {
        title: "dev".into(),
        cli: "claude".into(),
        config_dir: String::new(),
        cwd: String::new(),
        model: None,
        effort: None,
        permission_mode: "manual".into(),
        ..Default::default()
    };
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        if flag == "--last" {
            pick = Some(dev::Pick::Last);
            continue;
        }
        let value = it.next().with_context(|| format!("{flag} needs a value"))?.clone();
        match flag.as_str() {
            "--data" => data = Some(PathBuf::from(value)),
            "--thread" => pick = Some(dev::Pick::Thread(value)),
            "--config-dir" => t.config_dir = value,
            "--cwd" => t.cwd = value,
            "--model" => t.model = Some(value),
            "--effort" => t.effort = Some(value),
            "--mode" => t.permission_mode = value,
            _ => bail!("unknown flag {flag}\n{USAGE}"),
        }
    }
    let data = data.context("--data is required: the host's data folder")?;
    let pick = match pick {
        Some(p) => p,
        None if t.config_dir.is_empty() || t.cwd.is_empty() => {
            bail!("a new thread needs --config-dir (the team's Claude config folder) and --cwd\n{USAGE}")
        }
        None => dev::Pick::New(t),
    };
    Ok((data, pick))
}

async fn serve(args: &[String]) -> Result<()> {
    let (mut data, mut port, mut tailscale, mut public_url) = (None, 0u16, None::<u16>, None::<String>);
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it.next().with_context(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--data" => data = Some(PathBuf::from(value)),
            "--port" => port = value.parse().context("--port takes a number")?,
            "--tailscale" => tailscale = Some(value.parse().context("--tailscale takes a port number")?),
            "--public-url" => public_url = Some(value.trim_end_matches('/').to_string() + "/"),
            _ => bail!("unknown flag {flag}\n{USAGE}"),
        }
    }
    let (host, recovered) = host::Host::open(&data.context("--data is required")?)?;
    if !recovered.threads.is_empty() {
        println!("Recovered after a restart: {} turn(s) cut off, {} approval(s) expired.", recovered.turns, recovered.requests);
    }
    let control_room = mcp::start(host.clone()).await?;
    host.start_ticker();
    push::start(host.clone());
    host.terms.start_reaper();
    host.continue_after_restart(&recovered).await?;
    println!("Control room (MCP) at {control_room}");
    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    // Phones reach the host over HTTPS on the tailnet only.
    let remote = match tailscale {
        Some(remote_port) => {
            let net = tls::tailnet()?;
            let dir = host.data_dir().join("tls");
            let config = tokio::task::spawn_blocking({ let (n, d) = (net.name.clone(), dir.clone()); move || tls::start(&n, &d) }).await??;
            Some(link::Remote { name: net.name, ip: net.ip, port: remote_port, tls: config })
        }
        None => None,
    };
    let result = link::serve(host.clone(), port, remote, public_url, shutdown).await;
    host.close_all().await;
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await; // let CLIs exit cleanly
    result
}

/// Replays a recording and checks it produces the events recorded live.
fn replay(path: &str) -> Result<()> {
    let entries = record::read(path.as_ref())?;
    let replayed = record::replay(&entries)?;
    let recorded = record::recorded_events(&entries);
    for e in &replayed {
        println!("{}", serde_json::to_string(e)?);
    }
    if replayed != recorded {
        bail!("replay produced {} events, the live run {}; they differ", replayed.len(), recorded.len());
    }
    println!("ok: {} events, same as the live run", replayed.len());
    Ok(())
}
