//! Curia's shared protocol, used by the host and by every client.
pub mod action;
pub mod event;

pub use action::Decision;
pub use event::{Event, ItemId, RequestKind, Resolution, TextKind, TurnOutcome, UsageWindow};
