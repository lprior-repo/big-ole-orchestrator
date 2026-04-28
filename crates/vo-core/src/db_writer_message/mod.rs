//! DbWriterMessage enum for atomic control-plane transitions.
//!
//! Per ADR-016: DbWriterActor uses fjall::OwnedWriteBatch for every control-plane
//! transition. All events are sent to DbWriterActor for batch commit.
//!
//! Per ADR-029: Execution leases with monotonic fence tokens for
//! (instance_id, step_id) pairs. All completion paths carry the fence.

mod event_commands;
mod instance_commands;
mod message;
mod signal_commands;
mod snapshot_commands;
mod timer_commands;

pub use message::DbWriterMessage;
pub use snapshot_commands::SnapshotData;
pub use timer_commands::TimerOp;

use thiserror::Error;

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

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_instance_id() -> vo_types::InstanceId {
        vo_types::InstanceId::parse("01ARYZ6S410000000000000000").expect("valid instance id")
    }

    fn valid_sequence() -> vo_types::SequenceNumber {
        vo_types::SequenceNumber::new_unchecked(1)
    }

    fn valid_idempotency_key() -> vo_types::IdempotencyKey {
        vo_types::IdempotencyKey::parse("key-1").expect("valid key")
    }

    fn valid_step_id() -> vo_types::StepId {
        vo_types::StepId::parse("step-1").expect("valid step id")
    }

    fn valid_fence_token() -> vo_types::FenceToken {
        vo_types::FenceToken::new(1).expect("valid fence token")
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
        let result = vo_types::FenceToken::new(0);
        assert!(result.is_err());
    }

    #[test]
    fn fence_token_rejects_zero_value_when_deserialized_from_json() {
        let result: Result<vo_types::FenceToken, _> = serde_json::from_str("0");
        assert!(result.is_err());
    }

    #[test]
    fn sequence_number_rejects_zero_value_when_constructed() {
        let result = vo_types::SequenceNumber::try_from(0u64);
        assert!(result.is_err());
    }

    #[test]
    fn sequence_number_rejects_zero_value_when_deserialized_from_json() {
        let result: Result<vo_types::SequenceNumber, _> = serde_json::from_str("0");
        assert!(result.is_err());
    }

    // ========================================================================
    // B30-B33: Non-empty string guarantees
    // ========================================================================

    #[test]
    fn instance_id_rejects_empty_string_when_parsed() {
        let result = vo_types::InstanceId::parse("");
        assert!(result.is_err());
    }

    #[test]
    fn step_id_rejects_empty_string_when_parsed() {
        let result = vo_types::StepId::parse("");
        assert!(result.is_err());
    }

    #[test]
    fn timer_id_rejects_empty_string_when_parsed() {
        let result = vo_types::TimerId::parse("");
        assert!(result.is_err());
    }

    #[test]
    fn idempotency_key_rejects_empty_string_when_parsed() {
        let result = vo_types::IdempotencyKey::parse("");
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
}
