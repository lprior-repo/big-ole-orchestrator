//! Procedural paradigm actor state and event application (ADR-017).

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use wtf_common::ActivityId;

#[cfg(test)]
mod tests;

mod apply;

pub use apply::{apply_event, ProceduralApplyError, ProceduralApplyResult};

/// The deterministic key for a single workflow operation.
pub type OperationId = ActivityId;

/// A completed operation in the checkpoint map.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Checkpoint {
    /// Result bytes returned to the workflow code.
    pub result: Bytes,
    /// JetStream sequence number of the `ActivityCompleted` event.
    pub completed_seq: u64,
}

/// In-memory state for a Procedural workflow actor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProceduralActorState {
    /// Completed operations: `operation_id` → `Checkpoint`.
    pub checkpoint_map: HashMap<u32, Checkpoint>,

    /// Monotonically incrementing counter for the next operation.
    pub operation_counter: u32,

    /// Currently dispatched operations: `operation_id` → `ActivityId`.
    pub in_flight: HashMap<u32, ActivityId>,

    /// In-flight timers: `timer_id` → `operation_id` (for sleep replay).
    #[serde(default)]
    pub in_flight_timers: HashMap<String, u32>,

    /// Received signals buffered for future `wait_for_signal` calls.
    /// Keyed by signal name; `Vec<Bytes>` preserves FIFO order for
    /// multiple arrivals of the same signal name.
    #[serde(default)]
    pub received_signals: HashMap<String, Vec<Bytes>>,

    /// JetStream sequence numbers already applied (idempotency — ADR-016).
    pub applied_seq: HashSet<u64>,

    /// Events processed since the last snapshot.
    pub events_since_snapshot: u32,
}

impl ProceduralActorState {
    /// Create a new empty `ProceduralActorState`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            checkpoint_map: HashMap::new(),
            operation_counter: 0,
            in_flight: HashMap::new(),
            in_flight_timers: HashMap::new(),
            received_signals: HashMap::new(),
            applied_seq: HashSet::new(),
            events_since_snapshot: 0,
        }
    }

    /// Return `true` if a checkpoint exists for `operation_id`.
    #[must_use]
    pub fn has_checkpoint(&self, operation_id: u32) -> bool {
        self.checkpoint_map.contains_key(&operation_id)
    }

    /// Look up the result of a previously completed operation.
    #[must_use]
    pub fn get_checkpoint(&self, operation_id: u32) -> Option<&Checkpoint> {
        self.checkpoint_map.get(&operation_id)
    }

    /// Return the highest operation ID with a checkpoint, or `None` if empty.
    #[must_use]
    pub fn max_checkpointed_operation_id(&self) -> Option<u32> {
        self.checkpoint_map.keys().copied().max()
    }
}

impl Default for ProceduralActorState {
    fn default() -> Self {
        Self::new()
    }
}
