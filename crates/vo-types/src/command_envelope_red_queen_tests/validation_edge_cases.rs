//! Red Queen tests: validation-edge-cases dimension for command envelope metadata (ADR-036).
//!
//! bead_id: ve-04w0
//! phase: state-5-red-queen
//!
//! Attacks: issued_at boundaries, version boundaries, issuer edge cases

use crate::{
    CommandEnvelope, CommandEnvelopeError, CommandMetadata, IdempotencyKey, Issuer, TimestampMs,
};

// CE-RQ-16: issued_at = 0 is accepted (epoch start)
#[test]
fn rq_command_envelope_issued_at_zero_accepted() {
    let json = r#"{
        "version": 1,
        "command_id": "cmd-zero",
        "correlation_id": "corr-zero",
        "causation_id": "cause-zero",
        "issuer": "system",
        "issued_at": 0
    }"#;
    let result = CommandEnvelope::from_str(json);
    assert!(result.is_ok(), "issued_at=0 must be accepted (epoch)");
}

// CE-RQ-17: issued_at exceeds u64::MAX is rejected
#[test]
fn rq_command_envelope_issued_at_exceeds_u64_max_rejected() {
    let json = r#"{
        "version": 1,
        "command_id": "cmd-big",
        "correlation_id": "corr-big",
        "causation_id": "cause-big",
        "issuer": "system",
        "issued_at": 18446744073709551616
    }"#;
    let result = CommandEnvelope::from_str(json);
    assert!(
        matches!(result, Err(CommandEnvelopeError::InvalidEnvelopeField(_))),
        "issued_at exceeding u64::MAX must be rejected"
    );
}

// CE-RQ-18: Version 0 is supported
#[test]
fn rq_command_envelope_version_zero_is_supported() {
    let json = r#"{
        "version": 0,
        "command_id": "cmd-v0",
        "correlation_id": "corr-v0",
        "causation_id": "cause-v0",
        "issuer": "system",
        "issued_at": 1700000000
    }"#;
    let env = CommandEnvelope::from_str(json).unwrap();
    assert!(env.is_supported(), "version 0 must be supported");
}

// CE-RQ-19: Version 1 is supported
#[test]
fn rq_command_envelope_version_one_is_supported() {
    let json = r#"{
        "version": 1,
        "command_id": "cmd-v1",
        "correlation_id": "corr-v1",
        "causation_id": "cause-v1",
        "issuer": "system",
        "issued_at": 1700000000
    }"#;
    let env = CommandEnvelope::from_str(json).unwrap();
    assert!(env.is_supported(), "version 1 must be supported");
}

// CE-RQ-20: Version 255 (u8::MAX) is rejected
#[test]
fn rq_command_envelope_version_u8_max_rejected() {
    let json = r#"{
        "version": 255,
        "command_id": "cmd-maxver",
        "correlation_id": "corr-maxver",
        "causation_id": "cause-maxver",
        "issuer": "system",
        "issued_at": 1700000000
    }"#;
    let result = CommandEnvelope::from_str(json);
    assert_eq!(
        result,
        Err(CommandEnvelopeError::UnsupportedEnvelopeVersion(255)),
        "version 255 must be rejected"
    );
}

// CE-RQ-21: Unknown issuer is rejected
#[test]
fn rq_command_envelope_unknown_issuer_rejected() {
    let json = r#"{
        "version": 1,
        "command_id": "cmd-unknown",
        "correlation_id": "corr-unknown",
        "causation_id": "cause-unknown",
        "issuer": "unknown_issuer",
        "issued_at": 1700000000
    }"#;
    let result = CommandEnvelope::from_str(json);
    assert!(
        matches!(result, Err(CommandEnvelopeError::InvalidEnvelopeField(_))),
        "unknown issuer must be rejected"
    );
}

// CE-RQ-22: Issuer case sensitivity (system != System != SYSTEM)
#[test]
fn rq_command_envelope_issuer_case_sensitive() {
    let json_upper = r#"{
        "version": 1,
        "command_id": "cmd-upper",
        "correlation_id": "corr-upper",
        "causation_id": "cause-upper",
        "issuer": "SYSTEM",
        "issued_at": 1700000000
    }"#;
    let result = CommandEnvelope::from_str(json_upper);
    assert!(
        matches!(result, Err(CommandEnvelopeError::InvalidEnvelopeField(_))),
        "SYSTEM (uppercase) must be rejected"
    );
}
