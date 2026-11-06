//! The Claude Code adapter.
mod launch;
mod mapper;
mod session;
mod wire;

pub use launch::{CONTROL_ROOM, Launch, find_claude};

/// Whether a message written mid-turn joins the running turn. Claude 2.1.318 in stream-json
/// mode does not: it finishes the turn, then answers the message in a turn of its own
/// (checked live 2026-11-06). So the host queues such messages itself, durably.
pub const STEERS_MID_TURN: bool = false;
pub use mapper::Mapper;
pub use session::ClaudeSession;
