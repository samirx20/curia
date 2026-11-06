//! One interface over every CLI adapter. The host talks to a `Session`; nothing above this
//! module knows which CLI a thread runs on, beyond what the session says it can do.
use crate::claude::{self, ClaudeSession};
use crate::agy::{self, AgySession};
use crate::codex::{self, CodexSession};
use crate::record::Recorder;
use crate::store::ThreadRow;
use anyhow::{Result, bail};
use curia_core::{Decision, Event, UsageWindow};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Clone)]
pub enum Session {
    Claude(Arc<ClaudeSession>),
    Codex(Arc<CodexSession>),
    Agy(Arc<AgySession>),
}

/// The control room as one thread sees it.
pub struct ControlRoom {
    pub url: String,
    pub token: String,
    /// Claude reads its MCP servers from a file; Codex takes them on the command line.
    pub config_file: PathBuf,
    /// A folder of the thread's own, for CLIs that read servers only from their account
    /// folder (agy gets an overlay of it here).
    pub home: PathBuf,
}

/// Starts the thread's CLI, resuming its session when it has one.
pub async fn start(row: &ThreadRow, control: Option<&ControlRoom>, instructions: Option<&str>, recorder: Recorder) -> Result<(Session, mpsc::UnboundedReceiver<Event>)> {
    match row.cli.as_str() {
        "claude" => {
            let launch = claude::Launch {
                config_dir: row.config_dir.clone().into(),
                cwd: row.cwd.clone().into(),
                model: row.model.clone(),
                effort: row.effort.clone(),
                permission_mode: crate::freedom::claude_mode(&row.permission_mode).to_string(),
                resume: row.native_session_id.clone(),
                mcp_config: control.map(|c| c.config_file.clone()),
                append_system_prompt: instructions.map(str::to_string),
            };
            let (s, rx) = ClaudeSession::start(&claude::find_claude()?, &launch, row.items_used, Some(recorder)).await?;
            Ok((Session::Claude(s), rx))
        }
        "codex" => {
            let launch = codex::Launch {
                codex_home: row.config_dir.clone().into(),
                cwd: row.cwd.clone().into(),
                model: row.model.clone(),
                effort: row.effort.clone(),
                freedom: row.permission_mode.clone(),
                resume: row.native_session_id.clone(),
                control_room: control.map(|c| (c.url.clone(), c.token.clone())),
                instructions: instructions.map(str::to_string),
            };
            let (s, rx) = CodexSession::start(&codex::find_codex()?, &launch, row.items_used, Some(recorder)).await?;
            Ok((Session::Codex(s), rx))
        }
        "agy" => {
            let gemini_dir = match control {
                Some(c) => {
                    agy::home::prepare(Path::new(&row.config_dir), &c.home, &c.url, &c.token)?;
                    c.home.clone()
                }
                None => row.config_dir.clone().into(),
            };
            let launch = agy::Launch {
                gemini_dir,
                cwd: row.cwd.clone().into(),
                model: row.model.clone(),
                effort: row.effort.clone(),
                freedom: row.permission_mode.clone(),
                resume: row.native_session_id.clone(),
                instructions: instructions.map(str::to_string),
            };
            let (s, rx) = AgySession::start(&agy::find_agy()?, &launch, row.items_used, Some(recorder)).await?;
            Ok((Session::Agy(s), rx))
        }
        other => bail!("Curia can't drive {other:?} yet"),
    }
}

/// The CLIs a team can use.
pub const KNOWN: &[&str] = &["claude", "codex", "agy"];

/// Whether the account in `config_dir` holds the CLI's session, so a thread can move there
/// and continue it.
pub fn session_visible(cli: &str, config_dir: &Path, session: &str) -> bool {
    match cli {
        "claude" => claude::session_visible(config_dir, session),
        "codex" => codex::session_visible(config_dir, session),
        "agy" => agy::session_visible(config_dir, session),
        _ => false,
    }
}

impl Session {
    pub async fn send(&self, text: &str) -> Result<()> {
        match self {
            Session::Claude(s) => s.send(text).await,
            Session::Codex(s) => s.send(text).await,
            Session::Agy(s) => s.send(text).await,
        }
    }

    pub async fn answer(&self, request_id: &str, decision: &Decision) -> Result<()> {
        match self {
            Session::Claude(s) => s.answer(request_id, decision).await,
            Session::Codex(s) => s.answer(request_id, decision).await,
            Session::Agy(s) => s.answer(request_id, decision).await,
        }
    }

    pub async fn interrupt(&self) -> Result<()> {
        match self {
            Session::Claude(s) => s.interrupt().await,
            Session::Codex(s) => s.interrupt().await,
            Session::Agy(s) => s.interrupt().await,
        }
    }

    pub async fn set_effort(&self, level: &str) -> Result<()> {
        match self {
            Session::Claude(s) => s.set_effort(level).await,
            Session::Codex(s) => s.set_effort(level).await,
            Session::Agy(s) => s.set_effort(level).await,
        }
    }

    pub async fn usage(&self) -> Result<Vec<UsageWindow>> {
        match self {
            Session::Claude(s) => s.usage().await,
            Session::Codex(s) => s.usage().await,
            Session::Agy(s) => s.usage().await,
        }
    }

    pub fn pid(&self) -> Option<u32> {
        match self {
            Session::Claude(s) => s.pid(),
            Session::Codex(s) => s.pid(),
            Session::Agy(s) => s.pid(),
        }
    }

    pub fn items_used(&self) -> u64 {
        match self {
            Session::Claude(s) => s.items_used(),
            Session::Codex(s) => s.items_used(),
            Session::Agy(s) => s.items_used(),
        }
    }

    pub async fn close(&self) {
        match self {
            Session::Claude(s) => s.close().await,
            Session::Codex(s) => s.close().await,
            Session::Agy(s) => s.close().await,
        }
    }

    /// Whether a message written mid-turn joins the running turn. Claude in stream-json mode
    /// doesn't (it answers it afterwards, checked 2026-11-06); Codex has `turn/steer`;
    /// agy runs each input line as a turn of its own.
    pub fn steers_mid_turn(&self) -> bool {
        match self {
            Session::Claude(_) => claude::STEERS_MID_TURN,
            Session::Codex(_) => true,
            Session::Agy(_) => false,
        }
    }
}
