//! Procedural paradigm actor state and event application (ADR-017).

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use wtf_common::{ActivityId, WorkflowEvent};

#[cfg(test)]
mod tests;

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

/// Result of applying a single event to Procedural state.
#[derive(Debug, Clone)]
pub enum ProceduralApplyResult {
    /// Event was already applied (duplicate delivery) — state unchanged.
    AlreadyApplied,
    /// No meaningful change (informational event).
    None,
    /// Activity dispatched — `operation_id` now in `in_flight`.
    ActivityDispatched {
        operation_id: u32,
        activity_id: ActivityId,
    },
    /// Activity completed — checkpoint recorded, workflow code may resume.
    ActivityCompleted { operation_id: u32, result: Bytes },
    /// Activity permanently failed (retries exhausted).
    ActivityFailed { operation_id: u32 },
}

/// Error applying an event to Procedural state.
#[derive(Debug, thiserror::Error)]
pub enum ProceduralApplyError {
    #[error("ActivityCompleted for unknown activity_id {0}; no matching in_flight entry")]
    UnknownActivityId(String),
}

/// Apply a single `WorkflowEvent` to the Procedural actor state.
pub fn apply_event(
    state: &ProceduralActorState,
    event: &WorkflowEvent,
    seq: u64,
) -> Result<(ProceduralActorState, ProceduralApplyResult), ProceduralApplyError> {
    if state.applied_seq.contains(&seq) {
        return Ok((state.clone(), ProceduralApplyResult::AlreadyApplied));
    }

    let result = match event {
        WorkflowEvent::ActivityDispatched { activity_id, .. } => {
            let mut next = state.clone();
            let op_id = next.operation_counter;
            next.operation_counter += 1;
            next.in_flight.insert(op_id, ActivityId::new(activity_id));
            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;

            (
                next,
                ProceduralApplyResult::ActivityDispatched {
                    operation_id: op_id,
                    activity_id: ActivityId::new(activity_id),
                },
            )
        }

        WorkflowEvent::ActivityCompleted {
            activity_id,
            result,
            ..
        } => {
            let aid = ActivityId::new(activity_id);

            let op_id = state
                .in_flight
                .iter()
                .find(|(_, v)| *v == &aid)
                .map(|(k, _)| *k)
                .ok_or_else(|| ProceduralApplyError::UnknownActivityId(activity_id.clone()))?;

            let mut next = state.clone();
            next.in_flight.remove(&op_id);
            next.checkpoint_map.insert(
                op_id,
                Checkpoint {
                    result: result.clone(),
                    completed_seq: seq,
                },
            );
            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;

            (
                next,
                ProceduralApplyResult::ActivityCompleted {
                    operation_id: op_id,
                    result: result.clone(),
                },
            )
        }

        WorkflowEvent::ActivityFailed {
            activity_id,
            retries_exhausted,
            ..
        } => {
            let aid = ActivityId::new(activity_id);
            let mut next = state.clone();

            if *retries_exhausted {
                let op_id = state
                    .in_flight
                    .iter()
                    .find(|(_, v)| *v == &aid)
                    .map(|(k, _)| *k);

                if let Some(id) = op_id {
                    next.in_flight.remove(&id);
                    next.applied_seq.insert(seq);
                    next.events_since_snapshot += 1;
                    return Ok((
                        next,
                        ProceduralApplyResult::ActivityFailed { operation_id: id },
                    ));
                }
            }

            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;
            (next, ProceduralApplyResult::None)
        }

        WorkflowEvent::NowSampled { operation_id, ts } => {
            let mut next = state.clone();
            let result_bytes = Bytes::copy_from_slice(&ts.timestamp_millis().to_le_bytes());
            next.checkpoint_map.insert(
                *operation_id,
                Checkpoint { result: result_bytes, completed_seq: seq },
            );
            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;
            (next, ProceduralApplyResult::None)
        }

        WorkflowEvent::RandomSampled { operation_id, value } => {
            let mut next = state.clone();
            let result_bytes = Bytes::copy_from_slice(&value.to_le_bytes());
            next.checkpoint_map.insert(
                *operation_id,
                Checkpoint { result: result_bytes, completed_seq: seq },
            );
            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;
            (next, ProceduralApplyResult::None)
        }

        WorkflowEvent::SnapshotTaken { .. } => {
            let mut next = state.clone();
            next.applied_seq.insert(seq);
            next.events_since_snapshot = 0;
            (next, ProceduralApplyResult::None)
        }

        _ => {
            let mut next = state.clone();
            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;
            (next, ProceduralApplyResult::None)
        }
    };

    Ok(result)
}
