//! The control-room tools. Descriptions are what agents read, so they state the rules.
use crate::host::Host;
use axum::http::request::Parts;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ServerCapabilities, ServerConfig};
use rmcp::{ErrorData, ServerHandler, schemars, tool, tool_handler, tool_router};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone)]
pub struct ControlRoom {
    host: Arc<Host>,

}

#[derive(Deserialize, schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct HandOff {
    /// The team to give the task to: a name from list_teams, or "auto" for the team with the
    /// most quota left.
    team: String,
    /// A short title for the task, shown to the user.
    title: String,
    /// The whole task. The team sees only this text, not your conversation, so include
    /// everything it needs: goal, files, constraints, and what "done" means.
    task: String,
    /// Optional. Reuse the same value when retrying a call that may have gone through, so a
    /// second team thread isn't started.
    client_request_id: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct ReportResult {
    /// What you did, what changed, and anything the thread that gave you the task must know.
    summary: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct SendMessage {
    /// The thread to message (a thread id from hand_off or from a message you received).
    thread_id: String,
    text: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct AskHuman {
    /// The question for the user.
    question: String,
    /// Suggested answers (the user may also type their own).
    #[serde(default)]
    options: Vec<String>,
}

#[tool_router]
impl ControlRoom {
    pub fn new(host: Arc<Host>) -> ControlRoom {
        ControlRoom { host }
    }

    #[tool(description = "List the teams (signed-in CLI accounts) you can hand work to, and whether each is busy in this run. A busy team takes more work through send_message to its thread.")]
    async fn list_teams(&self, Extension(parts): Extension<Parts>) -> Result<CallToolResult, ErrorData> {
        let caller = self.caller(&parts)?;
        Ok(reply(self.host.list_teams(&caller)))
    }

    #[tool(description = "Give one self-contained task to a team. It gets only the task text, works in its own git worktree and branch made from your latest commit (commit first if it needs your changes), and can't have more freedom than you. Returns at once with the team thread's id. Its result comes back to you exactly once, as a message, when its turn ends, with its branch merged into your folder; so end your turn instead of waiting or polling.")]
    async fn hand_off(&self, Extension(parts): Extension<Parts>, Parameters(p): Parameters<HandOff>) -> Result<CallToolResult, ErrorData> {
        let caller = self.caller(&parts)?;
        Ok(reply(self.host.hand_off(&caller, &p.team, &p.title, &p.task, p.client_request_id.as_deref()).await))
    }

    #[tool(description = "For a team thread: hand your result back to the thread that gave you the task. Call it once when the task is done, then end your turn; the result is delivered when the turn ends, with your branch merged. Calling it again before then replaces the summary.")]
    async fn report_result(&self, Extension(parts): Extension<Parts>, Parameters(p): Parameters<ReportResult>) -> Result<CallToolResult, ErrorData> {
        let caller = self.caller(&parts)?;
        Ok(reply(self.host.report_result(&caller, &p.summary)))
    }

    #[tool(description = "Send a message to another thread in this run. If it is idle the message starts its turn; if it is working, the message is slipped into its turn when no tool is running, otherwise queued until its turn ends. It never interrupts.")]
    async fn send_message(&self, Extension(parts): Extension<Parts>, Parameters(p): Parameters<SendMessage>) -> Result<CallToolResult, ErrorData> {
        let caller = self.caller(&parts)?;
        Ok(reply(self.host.send_message(&caller, &p.thread_id, &p.text).await))
    }

    #[tool(description = "Read the messages queued for you while you were working (results and messages from other threads). Reading them removes them from the queue, so they won't also arrive as a new turn.")]
    async fn read_messages(&self, Extension(parts): Extension<Parts>) -> Result<CallToolResult, ErrorData> {
        let caller = self.caller(&parts)?;
        Ok(reply(self.host.read_messages(&caller)))
    }

    #[tool(description = "Ask the user a question and wait for the answer. It appears in the user's inbox on every device. Use it for decisions only the user can make, not for things you can find out yourself.")]
    async fn ask_human(&self, Extension(parts): Extension<Parts>, Parameters(p): Parameters<AskHuman>) -> Result<CallToolResult, ErrorData> {
        let caller = self.caller(&parts)?;
        Ok(reply(self.host.ask_human(&caller, &p.question, &p.options).await))
    }
}

impl ControlRoom {
    /// The thread behind this call, from its token. A token stops working when the thread's
    /// CLI stops, so a call from a stopped thread is refused.
    fn caller(&self, parts: &Parts) -> Result<String, ErrorData> {
        let token = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .unwrap_or("");
        self.host
            .thread_for_token(token)
            .ok_or_else(|| ErrorData::invalid_request("not a current Curia thread token (the thread's CLI may have stopped)", None))
    }
}

#[tool_handler]
impl ServerHandler for ControlRoom {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions("Curia's control room: hand work to other teams, get results back, message threads, and ask the user.")
    }
}

/// A host result as the agent sees it: the JSON, or the error as a readable tool error.
fn reply(r: anyhow::Result<Value>) -> CallToolResult {
    match r {
        Ok(v) => CallToolResult::success(vec![ContentBlock::text(v.to_string())]),
        Err(e) => CallToolResult::error(vec![ContentBlock::text(format!("{e:#}"))]),
    }
}
