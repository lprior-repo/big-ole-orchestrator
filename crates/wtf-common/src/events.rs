//! `WorkflowEvent` — the only type written to NATS `JetStream` (ADR-013).
//!
//! This is a closed enum. No other type is appended to the event log.
//! Serialized with msgpack (rmp-serde) for `JetStream`; JSON-serializable for debugging.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A declared side effect attached to an FSM transition.
///
/// Effect declarations are embedded in `TransitionApplied` events so they can
/// be skipped on replay (the effect already happened — ADR-015/ADR-016).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectDeclaration {
    /// Name of the effect (e.g. `"CallAuthorizationService"`).
    pub effect_type: String,
    /// Msgpack-encoded effect payload.
    pub payload: Bytes,
}

/// How many times to retry a failed activity before giving up.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of attempts (including the first). 0 = no retries.
    pub max_attempts: u32,
    /// Initial delay in milliseconds before the first retry.
    pub initial_interval_ms: u64,
    /// Exponential backoff multiplier (e.g. 2.0 = double delay each retry).
    pub backoff_coefficient: f64,
    /// Maximum delay cap in milliseconds.
    pub max_interval_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_interval_ms: 1_000,
            backoff_coefficient: 2.0,
            max_interval_ms: 60_000,
        }
    }
}

/// Every durable state transition is recorded as a `WorkflowEvent` in NATS `JetStream`.
///
/// **Invariant:** This enum is exhaustive. Only these variants are ever appended
/// to the `wtf-events` stream. New variants require an ADR amendment.
///
/// Serialization note: msgpack via `rmp_serde::to_vec_named` for `JetStream`.
/// The `#[serde(tag = "type", rename_all = "snake_case")]` ensures forward-compatible
/// JSON representations for debugging and the Monitor UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkflowEvent {
    /// A new workflow instance was created and its initial input recorded.
    InstanceStarted {
        /// ULID of this instance.
        instance_id: String,
        /// Name of the workflow type (e.g. `"checkout"`).
        workflow_type: String,
        /// Msgpack-encoded input payload.
        input: Bytes,
    },

    /// The workflow completed successfully and its output is recorded.
    InstanceCompleted {
        /// Msgpack-encoded output payload.
        output: Bytes,
    },

    /// The workflow failed with an unrecoverable error.
    InstanceFailed {
        /// Human-readable error description.
        error: String,
    },

    /// The workflow was cancelled by an external request.
    InstanceCancelled {
        /// Reason for cancellation.
        reason: String,
    },

    // ── FSM paradigm (ADR-017) ────────────────────────────────────────────────
    /// An FSM state transition was applied.
    ///
    /// The `effects` list is embedded here so that replay can skip re-executing
    /// effects that have already occurred (ADR-015 write-ahead guarantee).
    TransitionApplied {
        /// The FSM state before this event.
        from_state: String,
        /// The event name that triggered this transition.
        event_name: String,
        /// The FSM state after this event.
        to_state: String,
        /// Side effects triggered by this transition (skipped on replay).
        effects: Vec<EffectDeclaration>,
    },

    // ── Activity lifecycle (shared by all paradigms) ──────────────────────────
    /// An activity was dispatched to the worker queue.
    ActivityDispatched {
        /// Unique ID for this activity invocation (`operation_id` for Procedural).
        activity_id: String,
        /// Name of the activity type (matches the worker registration).
        activity_type: String,
        /// Msgpack-encoded input to the activity.
        payload: Bytes,
        /// Retry configuration for this activity.
        retry_policy: RetryPolicy,
        /// Which attempt this dispatch represents (1 = first attempt).
        attempt: u32,
    },

    /// An activity completed successfully.
    ActivityCompleted {
        /// Matches the `activity_id` from `ActivityDispatched`.
        activity_id: String,
        /// Msgpack-encoded activity output.
        result: Bytes,
        /// Wall-clock duration of the activity in milliseconds.
        duration_ms: u64,
    },

    /// An activity failed (possibly with retries remaining).
    ActivityFailed {
        /// Matches the `activity_id` from `ActivityDispatched`.
        activity_id: String,
        /// Human-readable error.
        error: String,
        /// If true, all retries were exhausted and the workflow should fail.
        retries_exhausted: bool,
    },

    /// A heartbeat sent by a long-running activity to report progress.
    ActivityHeartbeat {
        /// Matches the `activity_id` from `ActivityDispatched`.
        activity_id: String,
        /// Human-readable progress update (max 1KB).
        progress: String,
    },

    // ── Timer lifecycle ───────────────────────────────────────────────────────
    /// A timer was scheduled to fire at a future time.
    TimerScheduled {
        /// Unique timer ID (ULID).
        timer_id: String,
        /// When the timer should fire.
        fire_at: DateTime<Utc>,
    },

    /// A scheduled timer fired.
    TimerFired {
        /// Matches the `timer_id` from `TimerScheduled`.
        timer_id: String,
    },

    /// A timer was cancelled before it fired.
    TimerCancelled {
        /// Matches the `timer_id` from `TimerScheduled`.
        timer_id: String,
    },

    // ── Signals ───────────────────────────────────────────────────────────────
    /// An external signal was received by this workflow instance.
    SignalReceived {
        /// Signal name (e.g. `"approve"`, `"cancel"`).
        signal_name: String,
        /// Msgpack-encoded signal payload.
        payload: Bytes,
    },

    // ── Child workflows ───────────────────────────────────────────────────────
    /// A child workflow was started by this instance.
    ChildStarted {
        /// ULID of the child instance.
        child_id: String,
        /// Workflow type of the child.
        workflow_type: String,
    },

    /// A child workflow completed.
    ChildCompleted {
        /// Matches `child_id` from `ChildStarted`.
        child_id: String,
        /// Msgpack-encoded child output.
        result: Bytes,
    },

    /// A child workflow failed.
    ChildFailed {
        /// Matches `child_id` from `ChildStarted`.
        child_id: String,
        /// Human-readable error from the child.
        error: String,
    },

    // ── Procedural deterministic sampling (ADR-020) ──────────────────────────
    /// A deterministic timestamp was sampled for a procedural workflow operation.
    ///
    /// Appended in place of `chrono::Utc::now()` so replay returns the same value.
    NowSampled {
        /// Operation ID that requested the timestamp.
        operation_id: u32,
        /// The wall-clock time recorded at first execution.
        ts: DateTime<Utc>,
    },

    /// A deterministic random `u64` was sampled for a procedural workflow operation.
    ///
    /// Appended in place of `rand::random::<u64>()` so replay returns the same value.
    RandomSampled {
        /// Operation ID that requested the value.
        operation_id: u32,
        /// The random value recorded at first execution.
        value: u64,
    },

    // ── Snapshot ─────────────────────────────────────────────────────────────
    /// Actor state was snapshotted to sled (ADR-019).
    ///
    /// Recovery uses this as the cursor: load the sled snapshot for `seq`,
    /// then replay `JetStream` from `seq + 1` to the tail.
    SnapshotTaken {
        /// `JetStream` sequence number of the last event applied before this snapshot.
        seq: u64,
        /// CRC32 checksum of the serialized state bytes (for corruption detection).
        checksum: u32,
    },
}

impl WorkflowEvent {
    /// Serialize to msgpack bytes for appending to NATS `JetStream`.
    ///
    /// # Errors
    /// Returns an error if serialization fails (should never happen for well-formed events).
    pub fn to_msgpack(&self) -> Result<Vec<u8>, rmp_serde::encode::Error> {
        rmp_serde::to_vec_named(self)
    }

    /// Deserialize from msgpack bytes read from NATS `JetStream`.
    ///
    /// # Errors
    /// Returns an error if the bytes are not a valid msgpack-encoded `WorkflowEvent`.
    pub fn from_msgpack(bytes: &[u8]) -> Result<Self, rmp_serde::decode::Error> {
        rmp_serde::from_slice(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_msgpack_now_sampled() {
        let event = WorkflowEvent::NowSampled {
            operation_id: 3,
            ts: DateTime::parse_from_rfc3339("2026-01-01T12:00:00Z")
                .expect("parse")
                .with_timezone(&Utc),
        };
        let bytes = event.to_msgpack().expect("encode");
        let decoded = WorkflowEvent::from_msgpack(&bytes).expect("decode");
        assert_eq!(event, decoded);
    }

    fn activity_completed() -> WorkflowEvent {
        WorkflowEvent::ActivityCompleted {
            activity_id: "act-001".into(),
            result: Bytes::from_static(b"ok"),
            duration_ms: 42,
        }
    }

    fn snapshot_taken() -> WorkflowEvent {
        WorkflowEvent::SnapshotTaken {
            seq: 99,
            checksum: 0xDEAD_BEEF,
        }
    }

    #[test]
    fn roundtrip_msgpack_activity_completed() {
        let event = activity_completed();
        let bytes = event.to_msgpack().expect("encode");
        let decoded = WorkflowEvent::from_msgpack(&bytes).expect("decode");
        assert_eq!(event, decoded);
    }

    #[test]
    fn roundtrip_msgpack_snapshot_taken() {
        let event = snapshot_taken();
        let bytes = event.to_msgpack().expect("encode");
        let decoded = WorkflowEvent::from_msgpack(&bytes).expect("decode");
        assert_eq!(event, decoded);
    }

    #[test]
    fn roundtrip_msgpack_transition_applied() {
        let event = WorkflowEvent::TransitionApplied {
            from_state: "Pending".into(),
            event_name: "Authorize".into(),
            to_state: "Authorized".into(),
            effects: vec![EffectDeclaration {
                effect_type: "CallAuthorizationService".into(),
                payload: Bytes::from_static(b"{}"),
            }],
        };
        let bytes = event.to_msgpack().expect("encode");
        let decoded = WorkflowEvent::from_msgpack(&bytes).expect("decode");
        assert_eq!(event, decoded);
    }

    #[test]
    fn roundtrip_msgpack_timer_scheduled() {
        let event = WorkflowEvent::TimerScheduled {
            timer_id: "tmr-001".into(),
            fire_at: DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
                .expect("parse")
                .with_timezone(&Utc),
        };
        let bytes = event.to_msgpack().expect("encode");
        let decoded = WorkflowEvent::from_msgpack(&bytes).expect("decode");
        assert_eq!(event, decoded);
    }

    #[test]
    fn serde_json_tag_is_snake_case() {
        let event = WorkflowEvent::SnapshotTaken {
            seq: 1,
            checksum: 0,
        };
        let json = serde_json::to_string(&event).expect("json");
        assert!(json.contains("\"type\":\"snapshot_taken\""), "got: {json}");
    }

    #[test]
    fn serde_json_tag_activity_dispatched() {
        let event = WorkflowEvent::ActivityDispatched {
            activity_id: "a".into(),
            activity_type: "fetch".into(),
            payload: Bytes::new(),
            retry_policy: RetryPolicy::default(),
            attempt: 1,
        };
        let json = serde_json::to_string(&event).expect("json");
        assert!(
            json.contains("\"type\":\"activity_dispatched\""),
            "got: {json}"
        );
    }

    #[test]
    fn retry_policy_default_has_three_attempts() {
        assert_eq!(RetryPolicy::default().max_attempts, 3);
    }

    #[test]
    fn retry_policy_default_backoff_is_two() {
        #[allow(clippy::float_cmp)]
        let is_two = RetryPolicy::default().backoff_coefficient == 2.0_f64;
        assert!(is_two);
    }

    #[test]
    fn effect_declaration_roundtrips_msgpack() {
        let decl = EffectDeclaration {
            effect_type: "SendEmail".into(),
            payload: Bytes::from_static(b"payload"),
        };
        let bytes = rmp_serde::to_vec_named(&decl).expect("encode");
        let decoded: EffectDeclaration = rmp_serde::from_slice(&bytes).expect("decode");
        assert_eq!(decl, decoded);
    }
}
