//! Behavior tests: is_supported, issuer parsing, round-trip, workflow.

use super::{CommandEnvelope, CommandEnvelopeError};
use crate::{CommandMetadata, IdempotencyKey, Issuer, TimestampMs};

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
