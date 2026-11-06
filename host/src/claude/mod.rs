//! The Claude Code adapter.
mod launch;
mod mapper;
mod session;
mod wire;

pub use launch::{Launch, find_claude};
pub use mapper::Mapper;
pub use session::ClaudeSession;
