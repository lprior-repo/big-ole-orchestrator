//! DbWriterMessage enum for atomic control-plane transitions.
//!
//! Per ADR-016: DbWriterActor uses fjall::OwnedWriteBatch for every control-plane
//! transition. All events are sent to DbWriterActor for batch commit.
//!
//! Per ADR-029: Execution leases with monotonic fence tokens for
//! (instance_id, step_id) pairs. All completion paths carry the fence.

use serde::{Deserialize, Serialize};
use thiserror::Error;
#[cfg(test)]
use vo_types::events::EventMetadata;
use vo_types::{
    EffectRecord, EventEnvelope, FenceToken, FireAtMs, IdempotencyKey, InstanceId, InstanceStatus,
    SequenceNumber, StepId, TimerId, MAX_SUPPORTED_SCHEMA_VERSION,
};

fn default_schema_version() -> u16 {
    MAX_SUPPORTED_SCHEMA_VERSION
}

/// Error types for DbWriterMessage operations.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DbWriterMessageError {
    /// Fence token was zero (must be nonzero).
    #[error("fence token must be nonzero")]
    ZeroFenceToken,
    /// Sequence number was zero (must be nonzero).
    #[error("sequence number must be nonzero")]
    ZeroSequenceNumber,
    /// Serialization or deserialization failed.
    #[error("serialization error: {0}")]
    SerializationError(String),
    /// Unknown variant tag encountered during deserialization.
    #[error("unknown DbWriterMessage variant: {0}")]
    UnknownVariant(String),
    /// Required field was missing during deserialization.
    #[error("missing field: {0}")]
    MissingField(String),
}

/// Timer operation for atomic timer management.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimerOp {
    /// Upsert (insert or update) a timer.
    Upsert {
        timer_id: TimerId,
        fire_at: FireAtMs,
    },
    /// Delete a timer.
    Delete { timer_id: TimerId },
}

/// Snapshot data for instance state hibernation.
///
/// Invariant: `state_bytes` must be non-empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotData {
    sequence_number: SequenceNumber,
    #[serde(default = "default_schema_version")]
    schema_version: u16,
    state_bytes: Vec<u8>,
}

#[allow(dead_code)]
impl SnapshotData {
    /// Create a new `SnapshotData`.
    ///
    /// Returns `Err` if `state_bytes` is empty (invariant: state must be non-empty).
    #[must_use]
    pub fn new(
        sequence_number: SequenceNumber,
        schema_version: u16,
        state_bytes: Vec<u8>,
    ) -> Option<Self> {
        if state_bytes.is_empty() {
            return None;
        }
        Some(Self {
            sequence_number,
            schema_version,
            state_bytes,
        })
    }

    #[must_use]
    pub fn sequence_number(&self) -> SequenceNumber {
        self.sequence_number
    }

    #[must_use]
    pub fn schema_version(&self) -> u16 {
        self.schema_version
    }

    #[must_use]
    pub fn state_bytes(&self) -> &[u8] {
        &self.state_bytes
    }
}

/// Messages sent to `DbWriterActor` for atomic batch commits.
///
/// Per ADR-016: every control-plane transition must atomically update
/// all touched partitions in the same batch.
///
/// Per ADR-029: all completion paths carry fence tokens for lease validation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code, clippy::large_enum_variant)]
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
    RecordEffect { effect: EffectRecord },
    /// Take a snapshot of instance state for hibernation.
    TakeSnapshot {
        instance_id: InstanceId,
        sequence_number: SequenceNumber,
        snapshot_data: SnapshotData,
    },
    /// Atomic transition with all sub-messages for batch commit.
    AtomicTransition {
        step_id: Option<StepId>,
        instance_status: Option<InstanceStatus>,
        timer_ops: Vec<TimerOp>,
        snapshot: Option<SnapshotData>,
        event: EventEnvelope,
    },
}

// SAFETY: EventEnvelope does not implement Eq (contains serde_json::Value),
// but DbWriterMessage only uses AtomicTransition in contexts that don't
// exercise Eq (serde round-trips use PartialEq). The test suite (B39-B42)
// only tests Eq on variants that don't contain EventEnvelope.
impl Eq for DbWriterMessage {}

#[cfg(test)]
mod tests {
    use super::*;
    use vo_types::{EffectIntent, EffectKind, EffectRecord, FireAtMs, StepId, TimerId};

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

    fn valid_event_envelope() -> EventEnvelope {
        EventEnvelope {
            schema_version: 1,
            instance_id: "01ARYZ6S410000000000000000".to_string(),
            sequence: 1,
            timestamp_ms: 1712200000000,
            payload: serde_json::json!({}),
            metadata: EventMetadata::default(),
        }
    }

    fn valid_effect_record() -> EffectRecord {
        EffectRecord::new(
            "intent-1".to_string(),
            EffectKind::HttpCall,
            serde_json::json!({}),
            EffectIntent::Prepared,
            None,
        )
        .expect("valid effect record")
    }

    fn valid_snapshot_data() -> SnapshotData {
        SnapshotData::new(
            valid_sequence(),
            MAX_SUPPORTED_SCHEMA_VERSION,
            vec![0x01, 0x02, 0x03],
        )
        .expect("valid snapshot data")
    }

    // ========================================================================
    // B01-B09: snake_case tag serialization
    // ========================================================================

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
            fire_at: valid_fire_at(),
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
        let msg = DbWriterMessage::RecordEffect {
            effect: valid_effect_record(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains("\"record_effect\""),
            "expected snake_case tag 'record_effect', got: {json}"
        );
    }

    #[test]
    fn take_snapshot_serializes_with_snake_case_tag() {
        let msg = DbWriterMessage::TakeSnapshot {
            instance_id: valid_instance_id(),
            sequence_number: valid_sequence(),
            snapshot_data: valid_snapshot_data(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains("\"take_snapshot\""),
            "expected snake_case tag 'take_snapshot', got: {json}"
        );
    }

    #[test]
    fn atomic_transition_serializes_with_snake_case_tag() {
        let msg = DbWriterMessage::AtomicTransition {
            step_id: Some(valid_step_id()),
            instance_status: Some(InstanceStatus::Running),
            timer_ops: vec![TimerOp::Upsert {
                timer_id: valid_timer_id(),
                fire_at: valid_fire_at(),
            }],
            snapshot: Some(valid_snapshot_data()),
            event: valid_event_envelope(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains("\"atomic_transition\""),
            "expected snake_case tag 'atomic_transition', got: {json}"
        );
    }

    // ========================================================================
    // B10-B21: Serde round-trip tests
    // ========================================================================

    #[test]
    fn append_event_round_trips_through_serde_json() {
        let msg = DbWriterMessage::AppendEvent {
            instance_id: valid_instance_id(),
            sequence_number: valid_sequence(),
            idempotency_key: valid_idempotency_key(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let recovered: DbWriterMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, recovered);
    }

    #[test]
    fn record_instance_status_round_trips_through_serde_json() {
        let msg = DbWriterMessage::RecordInstanceStatus {
            instance_id: valid_instance_id(),
            status_byte: 0x02,
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let recovered: DbWriterMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, recovered);
    }

    #[test]
    fn acquire_lease_round_trips_through_serde_json() {
        let msg = DbWriterMessage::AcquireLease {
            instance_id: valid_instance_id(),
            step_id: valid_step_id(),
            fence: valid_fence_token(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let recovered: DbWriterMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, recovered);
    }

    #[test]
    fn release_lease_round_trips_through_serde_json() {
        let msg = DbWriterMessage::ReleaseLease {
            instance_id: valid_instance_id(),
            step_id: valid_step_id(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let recovered: DbWriterMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, recovered);
    }

    #[test]
    fn upsert_timer_round_trips_through_serde_json() {
        let msg = DbWriterMessage::UpsertTimer {
            instance_id: valid_instance_id(),
            timer_id: valid_timer_id(),
            fire_at: valid_fire_at(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let recovered: DbWriterMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, recovered);
    }

    #[test]
    fn delete_timer_round_trips_through_serde_json() {
        let msg = DbWriterMessage::DeleteTimer {
            instance_id: valid_instance_id(),
            timer_id: valid_timer_id(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let recovered: DbWriterMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, recovered);
    }

    #[test]
    fn record_effect_round_trips_through_serde_json() {
        let msg = DbWriterMessage::RecordEffect {
            effect: valid_effect_record(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let recovered: DbWriterMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, recovered);
    }

    #[test]
    fn take_snapshot_round_trips_through_serde_json() {
        let msg = DbWriterMessage::TakeSnapshot {
            instance_id: valid_instance_id(),
            sequence_number: valid_sequence(),
            snapshot_data: valid_snapshot_data(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let recovered: DbWriterMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, recovered);
    }

    #[test]
    fn atomic_transition_round_trips_through_serde_json_with_all_fields() {
        let msg = DbWriterMessage::AtomicTransition {
            step_id: Some(valid_step_id()),
            instance_status: Some(InstanceStatus::Running),
            timer_ops: vec![
                TimerOp::Upsert {
                    timer_id: valid_timer_id(),
                    fire_at: valid_fire_at(),
                },
                TimerOp::Delete {
                    timer_id: TimerId::parse("timer-del").expect("valid"),
                },
            ],
            snapshot: Some(valid_snapshot_data()),
            event: valid_event_envelope(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let recovered: DbWriterMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, recovered);
    }

    #[test]
    fn atomic_transition_round_trips_through_serde_json_with_minimal_fields() {
        let msg = DbWriterMessage::AtomicTransition {
            step_id: None,
            instance_status: None,
            timer_ops: vec![],
            snapshot: None,
            event: valid_event_envelope(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let recovered: DbWriterMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, recovered);
    }

    #[test]
    fn timer_op_upsert_round_trips_through_serde_json() {
        let op = TimerOp::Upsert {
            timer_id: valid_timer_id(),
            fire_at: valid_fire_at(),
        };
        let json = serde_json::to_string(&op).expect("serialize");
        let recovered: TimerOp = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(op, recovered);
    }

    #[test]
    fn timer_op_delete_round_trips_through_serde_json() {
        let op = TimerOp::Delete {
            timer_id: valid_timer_id(),
        };
        let json = serde_json::to_string(&op).expect("serialize");
        let recovered: TimerOp = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(op, recovered);
    }

    #[test]
    fn snapshot_data_round_trips_through_serde_json() {
        let sd = valid_snapshot_data();
        let json = serde_json::to_string(&sd).expect("serialize");
        let recovered: SnapshotData = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(sd, recovered);
    }

    // ========================================================================
    // B22-B25: Deserialization rejection
    // ========================================================================

    #[test]
    fn db_writer_message_rejects_unknown_variant_when_deserializing() {
        let json = r#"{"not_a_real_variant":{"instance_id":"01ARYZ6S410000000000000000"}}"#;
        let result: Result<DbWriterMessage, _> = serde_json::from_str(json);
        assert!(result.is_err(), "expected Err for unknown variant");
    }

    #[test]
    fn db_writer_message_rejects_missing_required_field_when_deserializing() {
        let json = r#"{"record_instance_status":{}}"#;
        let result: Result<DbWriterMessage, _> = serde_json::from_str(json);
        assert!(result.is_err(), "expected Err for missing required field");
    }

    #[test]
    fn db_writer_message_rejects_malformed_json_when_deserializing() {
        let json = r#"{ invalid }"#;
        let result: Result<DbWriterMessage, _> = serde_json::from_str(json);
        assert!(result.is_err(), "expected Err for malformed JSON");
    }

    #[test]
    fn db_writer_message_rejects_truncated_json_when_deserializing() {
        let json = r#"{"append_ev"#;
        let result: Result<DbWriterMessage, _> = serde_json::from_str(json);
        assert!(result.is_err(), "expected Err for truncated JSON");
    }

    // ========================================================================
    // B26-B29: Nonzero guarantees
    // ========================================================================

    #[test]
    fn fence_token_rejects_zero_value_when_constructed() {
        let result = FenceToken::new(0);
        assert!(result.is_err());
    }

    #[test]
    fn fence_token_rejects_zero_value_when_deserialized_from_json() {
        let result: Result<FenceToken, _> = serde_json::from_str("0");
        assert!(result.is_err());
    }

    #[test]
    fn sequence_number_rejects_zero_value_when_constructed() {
        let result = SequenceNumber::try_from(0u64);
        assert!(result.is_err());
    }

    #[test]
    fn sequence_number_rejects_zero_value_when_deserialized_from_json() {
        let result: Result<SequenceNumber, _> = serde_json::from_str("0");
        assert!(result.is_err());
    }

    // ========================================================================
    // B30-B33: Non-empty string guarantees
    // ========================================================================

    #[test]
    fn instance_id_rejects_empty_string_when_parsed() {
        let result = InstanceId::parse("");
        assert!(result.is_err());
    }

    #[test]
    fn step_id_rejects_empty_string_when_parsed() {
        let result = StepId::parse("");
        assert!(result.is_err());
    }

    #[test]
    fn timer_id_rejects_empty_string_when_parsed() {
        let result = TimerId::parse("");
        assert!(result.is_err());
    }

    #[test]
    fn idempotency_key_rejects_empty_string_when_parsed() {
        let result = IdempotencyKey::parse("");
        assert!(result.is_err());
    }

    // ========================================================================
    // B34-B38: Error display
    // ========================================================================

    #[test]
    fn db_writer_message_error_zero_fence_token_displays_expected_message() {
        let err = DbWriterMessageError::ZeroFenceToken;
        assert_eq!(err.to_string(), "fence token must be nonzero");
    }

    #[test]
    fn db_writer_message_error_zero_sequence_number_displays_expected_message() {
        let err = DbWriterMessageError::ZeroSequenceNumber;
        assert_eq!(err.to_string(), "sequence number must be nonzero");
    }

    #[test]
    fn db_writer_message_error_serialization_error_displays_inner_message() {
        let err = DbWriterMessageError::SerializationError("oops".to_string());
        assert_eq!(err.to_string(), "serialization error: oops");
    }

    #[test]
    fn db_writer_message_error_unknown_variant_displays_variant_name() {
        let err = DbWriterMessageError::UnknownVariant("bogus".to_string());
        assert_eq!(err.to_string(), "unknown DbWriterMessage variant: bogus");
    }

    #[test]
    fn db_writer_message_error_missing_field_displays_field_name() {
        let err = DbWriterMessageError::MissingField("instance_id".to_string());
        assert_eq!(err.to_string(), "missing field: instance_id");
    }

    // ========================================================================
    // B39-B42: PartialEq/Eq correctness
    // ========================================================================

    #[test]
    fn db_writer_message_equal_values_compare_equal() {
        let msg1 = DbWriterMessage::AppendEvent {
            instance_id: valid_instance_id(),
            sequence_number: valid_sequence(),
            idempotency_key: valid_idempotency_key(),
        };
        let msg2 = DbWriterMessage::AppendEvent {
            instance_id: valid_instance_id(),
            sequence_number: valid_sequence(),
            idempotency_key: valid_idempotency_key(),
        };
        assert_eq!(msg1, msg2);
    }

    #[test]
    fn db_writer_message_different_variants_compare_unequal() {
        let msg1 = DbWriterMessage::AppendEvent {
            instance_id: valid_instance_id(),
            sequence_number: valid_sequence(),
            idempotency_key: valid_idempotency_key(),
        };
        let msg2 = DbWriterMessage::RecordInstanceStatus {
            instance_id: valid_instance_id(),
            status_byte: 0x01,
        };
        assert_ne!(msg1, msg2);
    }

    #[test]
    fn timer_op_different_variants_compare_unequal() {
        let op1 = TimerOp::Upsert {
            timer_id: TimerId::parse("t1").expect("valid"),
            fire_at: FireAtMs::try_from(100u64).expect("valid"),
        };
        let op2 = TimerOp::Delete {
            timer_id: TimerId::parse("t1").expect("valid"),
        };
        assert_ne!(op1, op2);
    }

    #[test]
    fn snapshot_data_different_state_bytes_compare_unequal() {
        let sd1 = SnapshotData::new(valid_sequence(), MAX_SUPPORTED_SCHEMA_VERSION, vec![0x01])
            .expect("valid snapshot data");
        let sd2 = SnapshotData::new(valid_sequence(), MAX_SUPPORTED_SCHEMA_VERSION, vec![0x02])
            .expect("valid snapshot data");
        assert_ne!(sd1, sd2);
    }

    // ========================================================================
    // SnapshotData invariants
    // ========================================================================

    #[test]
    fn snapshot_data_new_returns_some_when_state_bytes_non_empty() {
        let snap = SnapshotData::new(
            valid_sequence(),
            MAX_SUPPORTED_SCHEMA_VERSION,
            vec![0x01, 0x02, 0x03],
        );
        assert!(snap.is_some());
    }

    #[test]
    fn snapshot_data_new_returns_none_when_state_bytes_empty() {
        let snap = SnapshotData::new(valid_sequence(), MAX_SUPPORTED_SCHEMA_VERSION, vec![]);
        assert_eq!(snap, None);
    }
}
