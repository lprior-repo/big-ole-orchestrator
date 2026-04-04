//! DbWriterMessage enum for atomic control-plane transitions.
//!
//! Per ADR-016: DbWriterActor uses fjall::Batch for every control-plane
//! transition. All events are sent to DbWriterActor for batch commit.
//!
//! Per ADR-029: Execution leases with monotonic fence tokens for
//! (instance_id, step_id) pairs. All completion paths carry the fence.

use serde::{Deserialize, Serialize};
use vo_types::{EffectRecord, FenceToken, FireAtMs, IdempotencyKey, InstanceId, SequenceNumber, StepId, TimerId};

/// Messages sent to `DbWriterActor` for atomic batch commits.
///
/// Per ADR-016: every control-plane transition must atomically update
/// all touched partitions in the same batch.
///
/// Per ADR-029: all completion paths carry fence tokens for lease validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum DbWriterMessage {
    /// Append an event to the event log with idempotency protection.
    AppendEvent {
        instance_id: InstanceId,
        sequence_number: SequenceNumber,
        idempotency_key: IdempotencyKey,
    },
    /// Record the current instance status in the instances index partition.
    RecordInstanceStatus {
        instance_id: InstanceId,
        status_byte: u8,
    },
    /// Acquire an execution lease for a (instance_id, step_id) pair (ADR-029).
    AcquireLease {
        instance_id: InstanceId,
        step_id: StepId,
        fence: FenceToken,
    },
    /// Release an execution lease for a (instance_id, step_id) pair (ADR-029).
    ReleaseLease {
        instance_id: InstanceId,
        step_id: StepId,
    },
    /// Upsert a timer in the timers partition.
    UpsertTimer {
        instance_id: InstanceId,
        timer_id: TimerId,
        fire_at: FireAtMs,
    },
    /// Delete a timer from the timers partition.
    DeleteTimer {
        instance_id: InstanceId,
        timer_id: TimerId,
    },
    /// Record an effect in the effect journal.
    RecordEffect {
        effect: EffectRecord,
    },
}

/// Snapshot data for instance state hibernation.
///
/// Invariant: `state_bytes` must be non-empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotData {
    sequence_number: SequenceNumber,
    state_bytes: Vec<u8>,
}

impl SnapshotData {
    /// Create a new `SnapshotData`.
    ///
    /// Returns `None` if `state_bytes` is empty (invariant: state must be non-empty).
    #[must_use]
    pub fn new(sequence_number: SequenceNumber, state_bytes: Vec<u8>) -> Option<Self> {
        if state_bytes.is_empty() {
            return None;
        }
        Some(Self {
            sequence_number,
            state_bytes,
        })
    }

    #[must_use]
    pub fn sequence_number(&self) -> SequenceNumber {
        self.sequence_number
    }

    #[must_use]
    pub fn state_bytes(&self) -> &[u8] {
        &self.state_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vo_types::{EffectIntent, EffectKind, EffectRecord, FenceToken, FireAtMs, StepId, TimerId};

    fn valid_instance_id() -> InstanceId {
        InstanceId::parse("01ARYZ6S410000000000000000").expect("valid instance id")
    }

    fn valid_sequence() -> SequenceNumber {
        SequenceNumber::new_unchecked(1)
    }

    fn valid_idempotency_key() -> IdempotencyKey {
        IdempotencyKey::parse("key-1").expect("valid key")
    }

    fn valid_step_id() -> StepId {
        StepId::parse("step-1").expect("valid step id")
    }

    fn valid_fence_token() -> FenceToken {
        FenceToken::new(1).expect("valid fence token")
    }

    fn valid_timer_id() -> TimerId {
        TimerId::parse("timer-1").expect("valid timer id")
    }

    fn valid_fire_at() -> FireAtMs {
        FireAtMs::try_from(1712200000000u64).expect("valid fire_at")
    }

    #[test]
    fn append_event_serializes_with_snake_case_tag() {
        let msg = DbWriterMessage::AppendEvent {
            instance_id: valid_instance_id(),
            sequence_number: valid_sequence(),
            idempotency_key: valid_idempotency_key(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains("\"append_event\""),
            "expected snake_case tag 'append_event', got: {json}"
        );
    }

    #[test]
    fn record_instance_status_serializes_with_snake_case_tag() {
        let msg = DbWriterMessage::RecordInstanceStatus {
            instance_id: valid_instance_id(),
            status_byte: 0x01,
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains("\"record_instance_status\""),
            "expected snake_case tag 'record_instance_status', got: {json}"
        );
    }

    #[test]
    fn acquire_lease_serializes_with_snake_case_tag() {
        let msg = DbWriterMessage::AcquireLease {
            instance_id: valid_instance_id(),
            step_id: valid_step_id(),
            fence: valid_fence_token(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains("\"acquire_lease\""),
            "expected snake_case tag 'acquire_lease', got: {json}"
        );
    }

    #[test]
    fn release_lease_serializes_with_snake_case_tag() {
        let msg = DbWriterMessage::ReleaseLease {
            instance_id: valid_instance_id(),
            step_id: valid_step_id(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains("\"release_lease\""),
            "expected snake_case tag 'release_lease', got: {json}"
        );
    }

    #[test]
    fn upsert_timer_serializes_with_snake_case_tag() {
        let msg = DbWriterMessage::UpsertTimer {
            instance_id: valid_instance_id(),
            timer_id: valid_timer_id(),
            fire_at: FireAtMs::try_from(1712200000000u64).unwrap(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains("\"upsert_timer\""),
            "expected snake_case tag 'upsert_timer', got: {json}"
        );
    }

    #[test]
    fn delete_timer_serializes_with_snake_case_tag() {
        let msg = DbWriterMessage::DeleteTimer {
            instance_id: valid_instance_id(),
            timer_id: valid_timer_id(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains("\"delete_timer\""),
            "expected snake_case tag 'delete_timer', got: {json}"
        );
    }

    #[test]
    fn record_effect_serializes_with_snake_case_tag() {
        let effect = EffectRecord::new(
            "intent-1".to_string(),
            EffectKind::HttpCall,
            serde_json::json!({}),
            EffectIntent::Prepared,
            None,
        ).expect("valid effect record");
        let msg = DbWriterMessage::RecordEffect { effect };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains("\"record_effect\""),
            "expected snake_case tag 'record_effect', got: {json}"
        );
    }

    #[test]
    fn snapshot_data_new_returns_some_when_state_bytes_non_empty() {
        let snap = SnapshotData::new(valid_sequence(), vec![0x01, 0x02, 0x03]);
        assert!(snap.is_some());
    }

    #[test]
    fn snapshot_data_new_returns_none_when_state_bytes_empty() {
        let snap = SnapshotData::new(valid_sequence(), vec![]);
        assert_eq!(snap, None);
    }
}
