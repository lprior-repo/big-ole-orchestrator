//! Red Queen tests: identity-collision dimension for command envelope metadata (ADR-036).
//!
//! bead_id: ve-04w0
//! phase: state-5-red-queen
//!
//! Attacks: Can two different command envelopes end up with the same identity?

use crate::{
    CommandEnvelope, CommandEnvelopeError, CommandMetadata, IdempotencyKey, Issuer, TimestampMs,
};

// CE-RQ-01: Different command_ids produce different IdempotencyKeys
#[test]
fn rq_command_envelope_different_command_ids_produce_different_keys() {
    let json1 = r#"{
        "version": 1,
        "command_id": "cmd-001",
        "correlation_id": "corr-shared",
        "causation_id": "cause-shared",
        "issuer": "system",
        "issued_at": 1700000000
    }"#;
    let json2 = r#"{
        "version": 1,
        "command_id": "cmd-002",
        "correlation_id": "corr-shared",
        "causation_id": "cause-shared",
        "issuer": "system",
        "issued_at": 1700000000
    }"#;

    let env1 = CommandEnvelope::from_str(json1).unwrap();
    let env2 = CommandEnvelope::from_str(json2).unwrap();

    assert_ne!(
        env1.metadata.command_id, env2.metadata.command_id,
        "different command_ids must produce different IdempotencyKeys"
    );
}

// CE-RQ-02: Same command_id parsed twice produces identical identity
#[test]
fn rq_command_envelope_same_command_id_produces_identical_identity() {
    let json = r#"{
        "version": 1,
        "command_id": "cmd-abc",
        "correlation_id": "corr-xyz",
        "causation_id": "cause-123",
        "issuer": "operator",
        "issued_at": 1700000000
    }"#;

    let env1 = CommandEnvelope::from_str(json).unwrap();
    let env2 = CommandEnvelope::from_str(json).unwrap();

    assert_eq!(env1.metadata.command_id, env2.metadata.command_id);
    assert_eq!(env1.metadata.correlation_id, env2.metadata.correlation_id);
    assert_eq!(env1.metadata.causation_id, env2.metadata.causation_id);
}

// CE-RQ-03: Identity fields are independent (changing one doesn't affect others)
#[test]
fn rq_command_envelope_identity_fields_are_independent() {
    let base_json = r#"{
        "version": 1,
        "command_id": "cmd-base",
        "correlation_id": "corr-base",
        "causation_id": "cause-base",
        "issuer": "system",
        "issued_at": 1700000000
    }"#;

    let base = CommandEnvelope::from_str(base_json).unwrap();

    // Change command_id only
    let modified_cmd = CommandEnvelope::from_str(
        r#"{
        "version": 1,
        "command_id": "cmd-modified",
        "correlation_id": "corr-base",
        "causation_id": "cause-base",
        "issuer": "system",
        "issued_at": 1700000000
    }"#,
    )
    .unwrap();

    assert_ne!(base.metadata.command_id, modified_cmd.metadata.command_id);
    assert_eq!(
        base.metadata.correlation_id,
        modified_cmd.metadata.correlation_id
    );
    assert_eq!(
        base.metadata.causation_id,
        modified_cmd.metadata.causation_id
    );

    // Change correlation_id only
    let modified_corr = CommandEnvelope::from_str(
        r#"{
        "version": 1,
        "command_id": "cmd-base",
        "correlation_id": "corr-modified",
        "causation_id": "cause-base",
        "issuer": "system",
        "issued_at": 1700000000
    }"#,
    )
    .unwrap();

    assert_eq!(base.metadata.command_id, modified_corr.metadata.command_id);
    assert_ne!(
        base.metadata.correlation_id,
        modified_corr.metadata.correlation_id
    );

    // Change causation_id only
    let modified_cause = CommandEnvelope::from_str(
        r#"{
        "version": 1,
        "command_id": "cmd-base",
        "correlation_id": "corr-base",
        "causation_id": "cause-modified",
        "issuer": "system",
        "issued_at": 1700000000
    }"#,
    )
    .unwrap();

    assert_eq!(base.metadata.command_id, modified_cause.metadata.command_id);
    assert_eq!(
        base.metadata.correlation_id,
        modified_cause.metadata.correlation_id
    );
    assert_ne!(
        base.metadata.causation_id,
        modified_cause.metadata.causation_id
    );
}
