//! DbWriterMessage enum for atomic control-plane transitions.
//!
//! Per ADR-016: DbWriterActor uses fjall::OwnedWriteBatch for every control-plane
//! transition. All events are sent to DbWriterActor for batch commit.
//!
//! Per ADR-029: Execution leases with monotonic fence tokens for
//! (instance_id, step_id) pairs. All completion paths carry the fence.

pub mod message;
pub mod tests;
pub mod types;

pub use message::DbWriterMessage;
pub use types::{DbWriterMessageError, SnapshotData, TimerOp};
