//! Actor message types for workflow instance actors.
//!
//! This module was moved from vo-actor/src/lib.rs as part of the
//! ADR-016/v2 module split refactoring.

pub mod instance_actor_message;
pub mod control_actor_message;

pub use instance_actor_message::InstanceActorMessage;
pub use control_actor_message::ControlActorMessage;
