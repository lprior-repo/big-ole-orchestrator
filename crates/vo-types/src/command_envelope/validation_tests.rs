//! Validation tests: version boundaries and field type validation.

use super::{CommandEnvelope, CommandEnvelopeError};

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
