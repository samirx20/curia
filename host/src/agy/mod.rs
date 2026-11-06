//! The Antigravity adapter: `agy` in stream-json mode, one process per thread.
//!
//! Headless agy can't ask for approval and refuses commands in every mode but Full, so
//! Antigravity teams run with `--dangerously-skip-permissions` (Plan is `--mode plan`), with
//! the run's worktree as the fence (Scope §3.4). It has no interrupt or mid-turn steering:
//! stopping closes the process, and the next message resumes with `--conversation`.
//! The control room reaches it through a per-thread overlay folder (`home`).
pub mod home;
mod launch;
pub(crate) mod mapper;
mod session;

pub use launch::{Launch, find_agy, session_visible};
pub use session::AgySession;
