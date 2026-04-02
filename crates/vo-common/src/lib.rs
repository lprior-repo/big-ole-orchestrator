//! Common utilities and types for vo-engine.
//!
//! Shared functionality used across multiple crates including
//! type aliases and common event definitions.

pub type InstanceId = String;
pub type NamespaceId = String;
pub type TimerId = String;

pub enum WorkflowEvent {
    TimerFired { timer_id: String, timestamp_ms: u64 },
}

pub type VoError = String;
