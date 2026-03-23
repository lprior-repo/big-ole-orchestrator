//! `WorkflowEvent` — the only type written to NATS `JetStream` (ADR-013).

use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;
mod types;

pub use types::*;

/// Every durable state transition is recorded as a `WorkflowEvent` in NATS `JetStream`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkflowEvent {
    /// A new workflow instance was created and its initial input recorded.
    InstanceStarted {
        instance_id: String,
        workflow_type: String,
        input: Bytes,
    },
    /// The workflow completed successfully and its output is recorded.
    InstanceCompleted { output: Bytes },
    /// The workflow failed with an unrecoverable error.
    InstanceFailed { error: String },
    /// The workflow was cancelled by an external request.
    InstanceCancelled { reason: String },
    /// An FSM state transition was applied.
    TransitionApplied {
        from_state: String,
        event_name: String,
        to_state: String,
        effects: Vec<EffectDeclaration>,
    },
    /// An activity was dispatched to the worker queue.
    ActivityDispatched {
        activity_id: String,
        activity_type: String,
        payload: Bytes,
        retry_policy: RetryPolicy,
        attempt: u32,
    },
    /// An activity completed successfully.
    ActivityCompleted {
        activity_id: String,
        result: Bytes,
        duration_ms: u64,
    },
    /// An activity failed (possibly with retries remaining).
    ActivityFailed {
        activity_id: String,
        error: String,
        retries_exhausted: bool,
    },
    /// A heartbeat sent by a long-running activity to report progress.
    ActivityHeartbeat {
        activity_id: String,
        progress: String,
    },
    /// A timer was scheduled to fire at a future time.
    TimerScheduled {
        timer_id: String,
        fire_at: DateTime<Utc>,
    },
    /// A scheduled timer fired.
    TimerFired { timer_id: String },
    /// A timer was cancelled before it fired.
    TimerCancelled { timer_id: String },
    /// An external signal was received by this workflow instance.
    SignalReceived { signal_name: String, payload: Bytes },
    /// A child workflow was started by this instance.
    ChildStarted {
        child_id: String,
        workflow_type: String,
    },
    /// A child workflow completed.
    ChildCompleted { child_id: String, result: Bytes },
    /// A child workflow failed.
    ChildFailed { child_id: String, error: String },
    /// A deterministic timestamp was sampled for a procedural workflow operation.
    NowSampled {
        operation_id: u32,
        ts: DateTime<Utc>,
    },
    /// A deterministic random `u64` was sampled for a procedural workflow operation.
    RandomSampled { operation_id: u32, value: u64 },
    /// Actor state was snapshotted to sled (ADR-019).
    SnapshotTaken { seq: u64, checksum: u32 },
}

impl WorkflowEvent {
    pub fn to_msgpack(&self) -> Result<Vec<u8>, rmp_serde::encode::Error> {
        rmp_serde::to_vec_named(self)
    }

    pub fn from_msgpack(bytes: &[u8]) -> Result<Self, rmp_serde::decode::Error> {
        rmp_serde::from_slice(bytes)
    }
}
