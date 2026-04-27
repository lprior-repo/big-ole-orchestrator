//! CommandEnvelope type for command identity, correlation, and causation (ADR-036).
//!
//! Every mutating API or CLI action enters the Engine as a versioned `CommandEnvelope`.
//! This module provides the canonical command envelope with parsing and validation.

mod signing;
mod validation;

pub use signing::{CommandEnvelope, CommandEnvelopeError, MAX_SUPPORTED_COMMAND_VERSION};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CommandMetadata, IdempotencyKey, Issuer, TimestampMs};

    // -------------------------------------------------------------------------
    // from_bytes tests
    // -------------------------------------------------------------------------

    #[test]
    fn command_envelope_from_bytes_returns_ok_when_input_is_valid_json() {
        let json = r#"{
            "version": 1,
            "command_id": "cmd-001",
            "correlation_id": "corr-001",
            "causation_id": "cause-001",
            "issuer": "system",
            "issued_at": 1700000000
        }"#;
        let result = CommandEnvelope::from_bytes(json.as_bytes());
        let envelope = result.expect("should parse successfully");
        assert_eq!(envelope.schema_version, 1, "schema_version should be 1");
        assert_eq!(
            envelope.metadata.command_id.as_str(),
            "cmd-001",
            "command_id should be preserved"
        );
        assert_eq!(
            envelope.metadata.correlation_id.as_str(),
            "corr-001",
            "correlation_id should be preserved"
        );
        assert_eq!(
            envelope.metadata.causation_id.as_str(),
            "cause-001",
            "causation_id should be preserved"
        );
        assert_eq!(envelope.metadata.issuer, Issuer::System);
        assert_eq!(envelope.metadata.issued_at.as_u64(), 1700000000);
    }

    #[test]
    fn command_envelope_from_bytes_returns_invalid_input_when_bytes_are_not_valid_utf8() {
        let bytes = vec![0xFF, 0xFE, 0xFD, 0x00];
        let result = CommandEnvelope::from_bytes(&bytes);
        assert!(
            matches!(result, Err(CommandEnvelopeError::InvalidInput)),
            "expected InvalidInput error, got: {:?}",
            result
        );
    }

    // -------------------------------------------------------------------------
    // from_str tests
    // -------------------------------------------------------------------------

    #[test]
    fn command_envelope_from_str_returns_ok_when_input_is_valid_json() {
        let json = r#"{
            "version": 1,
            "command_id": "cmd-abc",
            "correlation_id": "corr-xyz",
            "causation_id": "cause-123",
            "issuer": "operator",
            "issued_at": 1700000000
        }"#;
        let result = CommandEnvelope::from_str(json);
        assert!(result.is_ok(), "expected Ok, got: {:?}", result);
        let envelope = result.unwrap();
        assert_eq!(envelope.schema_version, 1);
    }

    #[test]
    fn command_envelope_from_str_returns_invalid_envelope_format_when_json_is_malformed() {
        let json = r#"{"version": 1, "command_id": "cmd-001""#;
        let result = CommandEnvelope::from_str(json);
        assert_eq!(
            result,
            Err(CommandEnvelopeError::InvalidEnvelopeFormat),
            "expected InvalidEnvelopeFormat error, got: {:?}",
            result
        );
    }

    #[test]
    fn command_envelope_from_str_returns_invalid_envelope_format_when_json_is_not_object() {
        let json = r#""just a string""#;
        let result = CommandEnvelope::from_str(json);
        assert_eq!(
            result,
            Err(CommandEnvelopeError::InvalidEnvelopeFormat),
            "expected InvalidEnvelopeFormat error, got: {:?}",
            result
        );
    }

    // -------------------------------------------------------------------------
    // Missing field tests
    // -------------------------------------------------------------------------

    #[test]
    fn command_envelope_from_str_returns_missing_envelope_field_when_version_is_absent() {
        let json = r#"{
            "command_id": "cmd-001",
            "correlation_id": "corr-001",
            "causation_id": "cause-001",
            "issuer": "system",
            "issued_at": 1700000000
        }"#;
        let result = CommandEnvelope::from_str(json);
        assert_eq!(
            result,
            Err(CommandEnvelopeError::MissingEnvelopeField(
                "version".to_string()
            )),
            "expected MissingEnvelopeField(\"version\"), got: {:?}",
            result
        );
    }

    #[test]
    fn command_envelope_from_str_returns_missing_envelope_field_when_command_id_is_absent() {
        let json = r#"{
            "version": 1,
            "correlation_id": "corr-001",
            "causation_id": "cause-001",
            "issuer": "system",
            "issued_at": 1700000000
        }"#;
        let result = CommandEnvelope::from_str(json);
        assert_eq!(
            result,
            Err(CommandEnvelopeError::MissingEnvelopeField(
                "command_id".to_string()
            )),
            "expected MissingEnvelopeField(\"command_id\"), got: {:?}",
            result
        );
    }

    #[test]
    fn command_envelope_from_str_returns_missing_envelope_field_when_correlation_id_is_absent() {
        let json = r#"{
            "version": 1,
            "command_id": "cmd-001",
            "causation_id": "cause-001",
            "issuer": "system",
            "issued_at": 1700000000
        }"#;
        let result = CommandEnvelope::from_str(json);
        assert_eq!(
            result,
            Err(CommandEnvelopeError::MissingEnvelopeField(
                "correlation_id".to_string()
            )),
            "expected MissingEnvelopeField(\"correlation_id\"), got: {:?}",
            result
        );
    }

    #[test]
    fn command_envelope_from_str_returns_missing_envelope_field_when_causation_id_is_absent() {
        let json = r#"{
            "version": 1,
            "command_id": "cmd-001",
            "correlation_id": "corr-001",
            "issuer": "system",
            "issued_at": 1700000000
        }"#;
        let result = CommandEnvelope::from_str(json);
        assert_eq!(
            result,
            Err(CommandEnvelopeError::MissingEnvelopeField(
                "causation_id".to_string()
            )),
            "expected MissingEnvelopeField(\"causation_id\"), got: {:?}",
            result
        );
    }

    #[test]
    fn command_envelope_from_str_returns_missing_envelope_field_when_issuer_is_absent() {
        let json = r#"{
            "version": 1,
            "command_id": "cmd-001",
            "correlation_id": "corr-001",
            "causation_id": "cause-001",
            "issued_at": 1700000000
        }"#;
        let result = CommandEnvelope::from_str(json);
        assert_eq!(
            result,
            Err(CommandEnvelopeError::MissingEnvelopeField(
                "issuer".to_string()
            )),
            "expected MissingEnvelopeField(\"issuer\"), got: {:?}",
            result
        );
    }

    #[test]
    fn command_envelope_from_str_returns_missing_envelope_field_when_issued_at_is_absent() {
        let json = r#"{
            "version": 1,
            "command_id": "cmd-001",
            "correlation_id": "corr-001",
            "causation_id": "cause-001",
            "issuer": "system"
        }"#;
        let result = CommandEnvelope::from_str(json);
        assert_eq!(
            result,
            Err(CommandEnvelopeError::MissingEnvelopeField(
                "issued_at".to_string()
            )),
            "expected MissingEnvelopeField(\"issued_at\"), got: {:?}",
            result
        );
    }

    // -------------------------------------------------------------------------
    // Version boundary tests
    // -------------------------------------------------------------------------

    #[test]
    fn command_envelope_from_str_returns_unsupported_envelope_version_when_version_exceeds_max() {
        let json = r#"{
            "version": 2,
            "command_id": "cmd-001",
            "correlation_id": "corr-001",
            "causation_id": "cause-001",
            "issuer": "system",
            "issued_at": 1700000000
        }"#;
        let result = CommandEnvelope::from_str(json);
        assert_eq!(
            result,
            Err(CommandEnvelopeError::UnsupportedEnvelopeVersion(2)),
            "expected UnsupportedEnvelopeVersion(2), got: {:?}",
            result
        );
    }

    #[test]
    fn command_envelope_from_str_returns_unsupported_envelope_version_when_version_is_u8_max() {
        let json = r#"{
            "version": 255,
            "command_id": "cmd-001",
            "correlation_id": "corr-001",
            "causation_id": "cause-001",
            "issuer": "system",
            "issued_at": 1700000000
        }"#;
        let result = CommandEnvelope::from_str(json);
        assert_eq!(
            result,
            Err(CommandEnvelopeError::UnsupportedEnvelopeVersion(255)),
            "expected UnsupportedEnvelopeVersion(255), got: {:?}",
            result
        );
    }

    #[test]
    fn command_envelope_from_str_returns_invalid_envelope_field_when_version_is_not_integer() {
        let json = r#"{
            "version": "1",
            "command_id": "cmd-001",
            "correlation_id": "corr-001",
            "causation_id": "cause-001",
            "issuer": "system",
            "issued_at": 1700000000
        }"#;
        let result = CommandEnvelope::from_str(json);
        assert!(
            matches!(result, Err(CommandEnvelopeError::InvalidEnvelopeField(_))),
            "expected InvalidEnvelopeField error, got: {:?}",
            result
        );
    }

    // -------------------------------------------------------------------------
    // is_supported tests
    // -------------------------------------------------------------------------

    #[test]
    fn command_envelope_is_supported_returns_true_when_version_is_zero() {
        let json = r#"{
            "version": 0,
            "command_id": "cmd-001",
            "correlation_id": "corr-001",
            "causation_id": "cause-001",
            "issuer": "system",
            "issued_at": 1700000000
        }"#;
        let envelope = CommandEnvelope::from_str(json).unwrap();
        assert!(envelope.is_supported(), "version 0 should be supported");
    }

    #[test]
    fn command_envelope_is_supported_returns_true_when_version_is_one() {
        let json = r#"{
            "version": 1,
            "command_id": "cmd-001",
            "correlation_id": "corr-001",
            "causation_id": "cause-001",
            "issuer": "system",
            "issued_at": 1700000000
        }"#;
        let envelope = CommandEnvelope::from_str(json).unwrap();
        assert!(envelope.is_supported(), "version 1 should be supported");
    }

    #[test]
    fn command_envelope_is_supported_returns_false_when_version_is_two() {
        let envelope = CommandEnvelope {
            schema_version: 2,
            metadata: CommandMetadata {
                command_id: IdempotencyKey::parse("cmd-001").unwrap(),
                correlation_id: IdempotencyKey::parse("corr-001").unwrap(),
                causation_id: IdempotencyKey::parse("cause-001").unwrap(),
                issuer: Issuer::System,
                issued_at: TimestampMs::try_from(1700000000u64).unwrap(),
            },
        };
        assert!(
            !envelope.is_supported(),
            "version 2 should not be supported"
        );
    }

    // -------------------------------------------------------------------------
    // Issuer parsing tests
    // -------------------------------------------------------------------------

    #[test]
    fn command_envelope_parses_all_issuer_variants() {
        let variants = [
            ("system", Issuer::System),
            ("api_client", Issuer::ApiClient),
            ("operator", Issuer::Operator),
            ("ai_agent", Issuer::AiAgent),
            ("timer_loop", Issuer::TimerLoop),
            ("recovery_loop", Issuer::RecoveryLoop),
        ];

        for (issuer_str, expected_issuer) in variants {
            let json = format!(
                r#"{{
                    "version": 1,
                    "command_id": "cmd-001",
                    "correlation_id": "corr-001",
                    "causation_id": "cause-001",
                    "issuer": "{}",
                    "issued_at": 1700000000
                }}"#,
                issuer_str
            );
            let envelope = CommandEnvelope::from_str(&json).unwrap();
            assert_eq!(
                envelope.metadata.issuer, expected_issuer,
                "issuer '{}' should parse to {:?}",
                issuer_str, expected_issuer
            );
        }
    }

    #[test]
    fn command_envelope_returns_invalid_envelope_field_for_unknown_issuer() {
        let json = r#"{
            "version": 1,
            "command_id": "cmd-001",
            "correlation_id": "corr-001",
            "causation_id": "cause-001",
            "issuer": "unknown_issuer",
            "issued_at": 1700000000
        }"#;
        let result = CommandEnvelope::from_str(json);
        assert!(
            matches!(result, Err(CommandEnvelopeError::InvalidEnvelopeField(_))),
            "expected InvalidEnvelopeField error, got: {:?}",
            result
        );
    }

    // -------------------------------------------------------------------------
    // JSON round-trip tests
    // -------------------------------------------------------------------------

    #[test]
    fn command_envelope_json_round_trip_preserves_all_fields() {
        let original_json = r#"{
            "version": 1,
            "command_id": "cmd-abc",
            "correlation_id": "corr-xyz",
            "causation_id": "cause-123",
            "issuer": "ai_agent",
            "issued_at": 1700000000
        }"#;

        // Parse
        let envelope = CommandEnvelope::from_str(original_json).unwrap();

        // Re-serialize
        let serialized = serde_json::to_string(&envelope).unwrap();

        // Parse again
        let deserialized: CommandEnvelope =
            CommandEnvelope::from_str(&serialized).expect("should re-parse");

        // Verify
        assert_eq!(
            envelope.schema_version, deserialized.schema_version,
            "schema_version should match"
        );
        assert_eq!(
            envelope.metadata.command_id, deserialized.metadata.command_id,
            "command_id should match"
        );
        assert_eq!(
            envelope.metadata.correlation_id, deserialized.metadata.correlation_id,
            "correlation_id should match"
        );
        assert_eq!(
            envelope.metadata.causation_id, deserialized.metadata.causation_id,
            "causation_id should match"
        );
        assert_eq!(
            envelope.metadata.issuer, deserialized.metadata.issuer,
            "issuer should match"
        );
        assert_eq!(
            envelope.metadata.issued_at, deserialized.metadata.issued_at,
            "issued_at should match"
        );
    }

    // -------------------------------------------------------------------------
    // Field type validation tests
    // -------------------------------------------------------------------------

    #[test]
    fn command_envelope_returns_invalid_envelope_field_when_command_id_is_not_string() {
        let json = r#"{
            "version": 1,
            "command_id": 123,
            "correlation_id": "corr-001",
            "causation_id": "cause-001",
            "issuer": "system",
            "issued_at": 1700000000
        }"#;
        let result = CommandEnvelope::from_str(json);
        assert!(
            matches!(result, Err(CommandEnvelopeError::InvalidEnvelopeField(_))),
            "expected InvalidEnvelopeField error, got: {:?}",
            result
        );
    }

    #[test]
    fn command_envelope_returns_invalid_envelope_field_when_issued_at_is_not_integer() {
        let json = r#"{
            "version": 1,
            "command_id": "cmd-001",
            "correlation_id": "corr-001",
            "causation_id": "cause-001",
            "issuer": "system",
            "issued_at": "not-a-number"
        }"#;
        let result = CommandEnvelope::from_str(json);
        assert!(
            matches!(result, Err(CommandEnvelopeError::InvalidEnvelopeField(_))),
            "expected InvalidEnvelopeField error, got: {:?}",
            result
        );
    }

    // -------------------------------------------------------------------------
    // Integration: full workflow test
    // -------------------------------------------------------------------------

    #[test]
    fn command_envelope_full_workflow_from_constructing_to_bytes_and_back() {
        // Build envelope directly
        let envelope = CommandEnvelope {
            schema_version: 1,
            metadata: CommandMetadata {
                command_id: IdempotencyKey::parse("workflow-cmd-001").unwrap(),
                correlation_id: IdempotencyKey::parse("workflow-corr-001").unwrap(),
                causation_id: IdempotencyKey::parse("workflow-cause-001").unwrap(),
                issuer: Issuer::AiAgent,
                issued_at: TimestampMs::try_from(1700000000u64).unwrap(),
            },
        };

        // Serialize to bytes
        let bytes = serde_json::to_vec(&envelope).expect("should serialize");

        // Deserialize back
        let deserialized: CommandEnvelope =
            CommandEnvelope::from_bytes(&bytes).expect("should deserialize");

        // Verify
        assert_eq!(envelope, deserialized);
    }
}
