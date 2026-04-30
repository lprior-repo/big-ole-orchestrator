//! Parsing tests: from_bytes, from_str, missing fields.

use super::{CommandEnvelope, CommandEnvelopeError};

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
    assert_eq!(envelope.metadata.issuer, super::Issuer::System);
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
