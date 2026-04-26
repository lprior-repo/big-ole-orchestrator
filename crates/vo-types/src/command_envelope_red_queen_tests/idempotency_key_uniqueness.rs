//! Red Queen tests: idempotency-key-uniqueness dimension for command envelope metadata (ADR-036).
//!
//! bead_id: ve-04w0
//! phase: state-5-red-queen
//!
//! Attacks: IdempotencyKey parsing rejects empty, enforces length, handles special chars

use crate::{
    CommandEnvelope, CommandEnvelopeError, CommandMetadata, IdempotencyKey, Issuer, TimestampMs,
};

// CE-RQ-04: Empty command_id is rejected
#[test]
fn rq_command_envelope_rejects_empty_command_id() {
    let json = r#"{
        "version": 1,
        "command_id": "",
        "correlation_id": "corr-001",
        "causation_id": "cause-001",
        "issuer": "system",
        "issued_at": 1700000000
    }"#;
    let result = CommandEnvelope::from_str(json);
    assert!(
        matches!(result, Err(CommandEnvelopeError::InvalidEnvelopeField(_))),
        "empty command_id must be rejected as InvalidEnvelopeField"
    );
}

// CE-RQ-05: Empty correlation_id is rejected
#[test]
fn rq_command_envelope_rejects_empty_correlation_id() {
    let json = r#"{
        "version": 1,
        "command_id": "cmd-001",
        "correlation_id": "",
        "causation_id": "cause-001",
        "issuer": "system",
        "issued_at": 1700000000
    }"#;
    let result = CommandEnvelope::from_str(json);
    assert!(
        matches!(result, Err(CommandEnvelopeError::InvalidEnvelopeField(_))),
        "empty correlation_id must be rejected"
    );
}

// CE-RQ-06: Empty causation_id is rejected
#[test]
fn rq_command_envelope_rejects_empty_causation_id() {
    let json = r#"{
        "version": 1,
        "command_id": "cmd-001",
        "correlation_id": "corr-001",
        "causation_id": "",
        "issuer": "system",
        "issued_at": 1700000000
    }"#;
    let result = CommandEnvelope::from_str(json);
    assert!(
        matches!(result, Err(CommandEnvelopeError::InvalidEnvelopeField(_))),
        "empty causation_id must be rejected"
    );
}

// CE-RQ-07: IdempotencyKey max length (1024 chars) is enforced
#[test]
fn rq_command_envelope_rejects_command_id_exceeding_max_length() {
    let long_id = "x".repeat(1025);
    let json = format!(
        r#"{{
        "version": 1,
        "command_id": "{}",
        "correlation_id": "corr-001",
        "causation_id": "cause-001",
        "issuer": "system",
        "issued_at": 1700000000
    }}"#,
        long_id
    );
    let result = CommandEnvelope::from_str(&json);
    assert!(
        matches!(result, Err(CommandEnvelopeError::InvalidEnvelopeField(_))),
        "command_id exceeding 1024 chars must be rejected"
    );
}

// CE-RQ-08: IdempotencyKey exactly at max length (1024) is accepted
#[test]
fn rq_command_envelope_accepts_command_id_at_max_length() {
    let max_id = "x".repeat(1024);
    let json = format!(
        r#"{{
        "version": 1,
        "command_id": "{}",
        "correlation_id": "corr-001",
        "causation_id": "cause-001",
        "issuer": "system",
        "issued_at": 1700000000
    }}"#,
        max_id
    );
    let result = CommandEnvelope::from_str(&json);
    assert!(
        result.is_ok(),
        "command_id at exactly 1024 chars must be accepted"
    );
}

// CE-RQ-09: IdempotencyKey with null byte via direct construction (not JSON parsing)
// IdempotencyKey validates identifier chars — null bytes must be rejected
#[test]
fn rq_idempotency_key_rejects_null_byte_directly() {
    let result = IdempotencyKey::parse("key\x00val");
    assert!(
        result.is_err(),
        "IdempotencyKey must reject null byte (identifier chars only)"
    );
}

// CE-RQ-10: All three IdempotencyKeys can be max length simultaneously
#[test]
fn rq_command_envelope_all_ids_at_max_length_accepted() {
    let max_id = "x".repeat(1024);
    let json = format!(
        r#"{{
        "version": 1,
        "command_id": "{}",
        "correlation_id": "{}",
        "causation_id": "{}",
        "issuer": "system",
        "issued_at": 1700000000
    }}"#,
        max_id, max_id, max_id
    );
    let result = CommandEnvelope::from_str(&json);
    assert!(
        result.is_ok(),
        "all three IDs at max length must be accepted"
    );
}
