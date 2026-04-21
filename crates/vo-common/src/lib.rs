//! Common utilities and types for vo-engine.
//!
//! Shared functionality used across multiple crates including
//! type aliases and common event definitions.

pub mod error;
pub mod events;
pub mod structures;
pub mod types;

pub use error::VoError;
pub use events::{DuplicateResult, EventDedup, WorkflowEvent};
pub use types::{EventId, InstanceId, NamespaceId, TimerId};
