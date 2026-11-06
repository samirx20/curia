//! Curia's host. For now it has two developer commands:
//!   curia-host dev --config-dir <dir> --cwd <dir> [--model M] [--effort E] [--mode M] [--resume ID] [--record FILE]
//!   curia-host replay <recording.jsonl>
//!   curia-host scrub <live recording> <fixture>
mod claude;
mod dev;
mod process;
mod record;
mod scrub;

use anyhow::{Context, Result, bail};
use std::path::PathBuf;

const USAGE: &str = "usage:\n  curia-host dev --config-dir <dir> --cwd <dir> [--model M] [--effort E] [--mode M] [--resume ID] [--record FILE]\n  curia-host replay <recording.jsonl>\n  curia-host scrub <live recording> <fixture>";

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("dev") => {
            let (launch, record) = dev_args(&args[1..])?;
            dev::run(launch, record).await
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

fn dev_args(args: &[String]) -> Result<(claude::Launch, Option<PathBuf>)> {
    let mut launch = claude::Launch {
        config_dir: PathBuf::new(),
        cwd: std::env::current_dir()?,
        model: None,
        effort: None,
        permission_mode: "manual".into(),
        resume: None,
    };
    let mut record = None;
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it.next().with_context(|| format!("{flag} needs a value"))?.clone();
        match flag.as_str() {
            "--config-dir" => launch.config_dir = value.into(),
            "--cwd" => launch.cwd = value.into(),
            "--model" => launch.model = Some(value),
            "--effort" => launch.effort = Some(value),
            "--mode" => launch.permission_mode = value,
            "--resume" => launch.resume = Some(value),
            "--record" => record = Some(PathBuf::from(value)),
            _ => bail!("unknown flag {flag}\n{USAGE}"),
        }
    }
    if launch.config_dir.as_os_str().is_empty() {
        bail!("--config-dir is required: the team's own Claude config folder (never your everyday one)");
    }
    Ok((launch, record))
}

/// Replays a recording and checks it produces the events recorded live.
fn replay(path: &str) -> Result<()> {
    let entries = record::read(path.as_ref())?;
    let replayed = record::replay_claude(&entries)?;
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
