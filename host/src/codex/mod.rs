//! The Codex adapter: `codex app-server` over JSON-RPC.
mod launch;
pub(crate) mod mapper;
pub(crate) mod replay;
mod session;
mod wire;

pub use launch::{Launch, find_codex, session_visible};
pub use session::CodexSession;
