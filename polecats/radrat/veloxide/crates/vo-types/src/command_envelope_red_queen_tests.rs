//! Red Queen adversarial tests for command envelope metadata (ADR-036).
//!
//! bead_id: ve-04w0
//! phase: state-5-red-queen
//!
//! Dimensions attacked:
//!   - identity-collision: command_id/correlation_id/causation_id collision resistance
//!   - idempotency-key-uniqueness: empty keys, max length, special chars, parsing edge cases
//!   - metadata-preservation: serde round-trip integrity under adversarial conditions
//!   - concurrent-simulated: rapid sequential parsing preserves identity
//!   - validation-edge-cases: issued_at boundaries, version boundaries, issuer variants
//!   - json-attacks: malformed JSON, wrong types, nulls, extra fields
//!   - error-semantics: error messages are correct and non-misleading

use crate::{
    CommandEnvelope, CommandEnvelopeError, CommandMetadata, IdempotencyKey, Issuer, TimestampMs,
};
use std::collections::HashSet;

// ===========================================================================
// DIMENSION: identity-collision
// Can two different command envelopes end up with the same identity?
// ===========================================================================

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

// ===========================================================================
// DIMENSION: idempotency-key-uniqueness
// IdempotencyKey parsing rejects empty, enforces length, handles special chars
// ===========================================================================

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

// ===========================================================================
// DIMENSION: metadata-preservation
// Serde round-trip integrity under various conditions
// ===========================================================================

// CE-RQ-11: Full metadata round-trip preserves all fields
#[test]
fn rq_command_envelope_metadata_round_trip_preserves_all_fields() {
    let original_json = r#"{
        "version": 1,
        "command_id": "cmd-test",
        "correlation_id": "corr-test",
        "causation_id": "cause-test",
        "issuer": "ai_agent",
        "issued_at": 1700000000
    }"#;

    let env = CommandEnvelope::from_str(original_json).unwrap();
    let serialized = serde_json::to_string(&env).unwrap();
    let restored: CommandEnvelope = CommandEnvelope::from_str(&serialized).unwrap();

    assert_eq!(env.schema_version, restored.schema_version);
    assert_eq!(env.metadata.command_id, restored.metadata.command_id);
    assert_eq!(
        env.metadata.correlation_id,
        restored.metadata.correlation_id
    );
    assert_eq!(env.metadata.causation_id, restored.metadata.causation_id);
    assert_eq!(env.metadata.issuer, restored.metadata.issuer);
    assert_eq!(env.metadata.issued_at, restored.metadata.issued_at);
}

// CE-RQ-12: Round-trip with u64::MAX timestamp
#[test]
fn rq_command_envelope_timestamp_max_round_trip() {
    let json = r#"{
        "version": 1,
        "command_id": "cmd-max",
        "correlation_id": "corr-max",
        "causation_id": "cause-max",
        "issuer": "recovery_loop",
        "issued_at": 18446744073709551615
    }"#;
    let env = CommandEnvelope::from_str(json).unwrap();
    let serialized = serde_json::to_string(&env).unwrap();
    let restored: CommandEnvelope = CommandEnvelope::from_str(&serialized).unwrap();
    assert_eq!(env.metadata.issued_at, restored.metadata.issued_at);
}

// CE-RQ-13: Round-trip with all issuer variants
#[test]
fn rq_command_envelope_all_issuers_round_trip() {
    let issuers = [
        "system",
        "api_client",
        "operator",
        "ai_agent",
        "timer_loop",
        "recovery_loop",
    ];

    for issuer_str in issuers {
        let json = format!(
            r#"{{
            "version": 1,
            "command_id": "cmd-{}",
            "correlation_id": "corr-{}",
            "causation_id": "cause-{}",
            "issuer": "{}",
            "issued_at": 1700000000
        }}"#,
            issuer_str, issuer_str, issuer_str, issuer_str
        );

        let env = CommandEnvelope::from_str(&json).unwrap();
        let serialized = serde_json::to_string(&env).unwrap();
        let restored: CommandEnvelope = CommandEnvelope::from_str(&serialized).unwrap();
        assert_eq!(env.metadata.issuer, restored.metadata.issuer);
    }
}

// CE-RQ-14: Multiple rapid parses produce consistent results
#[test]
fn rq_command_envelope_rapid_sequential_parses_produce_consistent_results() {
    let json = r#"{
        "version": 1,
        "command_id": "cmd-rapid",
        "correlation_id": "corr-rapid",
        "causation_id": "cause-rapid",
        "issuer": "system",
        "issued_at": 1700000000
    }"#;

    let first = CommandEnvelope::from_str(json).unwrap();
    let all_same = (0..100)
        .map(|_| CommandEnvelope::from_str(json).unwrap())
        .all(|env| env == first);

    assert!(
        all_same,
        "100 rapid parses should all produce identical result"
    );
}

// CE-RQ-15: Serde produces re-parsable JSON (not just valid JSON)
#[test]
fn rq_command_envelope_serde_produces_re_parsable_json() {
    let original_json = r#"{
        "version": 1,
        "command_id": "cmd-reserial",
        "correlation_id": "corr-reserial",
        "causation_id": "cause-reserial",
        "issuer": "operator",
        "issued_at": 1700000000
    }"#;

    let env = CommandEnvelope::from_str(original_json).unwrap();
    let json_str = serde_json::to_string(&env).unwrap();
    let bytes = json_str.as_bytes();
    let reparsed: CommandEnvelope = CommandEnvelope::from_bytes(bytes).unwrap();
    assert_eq!(env, reparsed);
}

// ===========================================================================
// DIMENSION: validation-edge-cases
// issued_at boundaries, version boundaries, issuer edge cases
// ===========================================================================

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

// ===========================================================================
// DIMENSION: json-attacks
// Malformed JSON, wrong types, nulls, extra fields
// ===========================================================================

// CE-RQ-23: command_id as number is rejected
#[test]
fn rq_command_envelope_command_id_number_rejected() {
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
        "command_id as number must be rejected"
    );
}

// CE-RQ-24: issued_at as string is rejected
#[test]
fn rq_command_envelope_issued_at_string_rejected() {
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
        "issued_at as string must be rejected"
    );
}

// CE-RQ-25: issuer as number is rejected
#[test]
fn rq_command_envelope_issuer_number_rejected() {
    let json = r#"{
        "version": 1,
        "command_id": "cmd-001",
        "correlation_id": "corr-001",
        "causation_id": "cause-001",
        "issuer": 42,
        "issued_at": 1700000000
    }"#;
    let result = CommandEnvelope::from_str(json);
    assert!(
        matches!(result, Err(CommandEnvelopeError::InvalidEnvelopeField(_))),
        "issuer as number must be rejected"
    );
}

// CE-RQ-26: version as string is rejected
#[test]
fn rq_command_envelope_version_string_rejected() {
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
        "version as string must be rejected"
    );
}

// CE-RQ-27: Extra fields are silently ignored (lenient parsing)
#[test]
fn rq_command_envelope_extra_fields_ignored() {
    let json = r#"{
        "version": 1,
        "command_id": "cmd-extra",
        "correlation_id": "corr-extra",
        "causation_id": "cause-extra",
        "issuer": "system",
        "issued_at": 1700000000,
        "extra_field": "should be ignored",
        "another_extra": 42,
        "nested": {"foo": "bar"}
    }"#;
    let result = CommandEnvelope::from_str(json);
    assert!(result.is_ok(), "extra JSON fields must be silently ignored");
}

// CE-RQ-28: Null command_id is rejected
#[test]
fn rq_command_envelope_null_command_id_rejected() {
    let json = r#"{
        "version": 1,
        "command_id": null,
        "correlation_id": "corr-001",
        "causation_id": "cause-001",
        "issuer": "system",
        "issued_at": 1700000000
    }"#;
    let result = CommandEnvelope::from_str(json);
    assert!(
        matches!(result, Err(CommandEnvelopeError::InvalidEnvelopeField(_))),
        "null command_id must be rejected"
    );
}

// CE-RQ-29: Empty JSON object is rejected
#[test]
fn rq_command_envelope_empty_object_rejected() {
    let json = r#"{}"#;
    let result = CommandEnvelope::from_str(json);
    assert!(
        matches!(result, Err(CommandEnvelopeError::MissingEnvelopeField(_))),
        "empty JSON object must be rejected with MissingEnvelopeField"
    );
}

// CE-RQ-30: Non-object JSON (array) is rejected
#[test]
fn rq_command_envelope_non_object_rejected() {
    let json = r#"[]"#;
    let result = CommandEnvelope::from_str(json);
    assert_eq!(
        result,
        Err(CommandEnvelopeError::InvalidEnvelopeFormat),
        "array instead of object must be rejected"
    );
}

// CE-RQ-31: Non-object JSON (string) is rejected
#[test]
fn rq_command_envelope_string_instead_of_object_rejected() {
    let json = r#""just a string""#;
    let result = CommandEnvelope::from_str(json);
    assert_eq!(
        result,
        Err(CommandEnvelopeError::InvalidEnvelopeFormat),
        "string instead of object must be rejected"
    );
}

// CE-RQ-32: Malformed JSON is rejected
#[test]
fn rq_command_envelope_malformed_json_rejected() {
    let json = r#"{"version": 1, "command_id": "cmd-001""#;
    let result = CommandEnvelope::from_str(json);
    assert_eq!(
        result,
        Err(CommandEnvelopeError::InvalidEnvelopeFormat),
        "malformed JSON must be rejected"
    );
}

// CE-RQ-33: Invalid UTF-8 bytes are rejected
#[test]
fn rq_command_envelope_invalid_utf8_rejected() {
    let bytes = vec![0xFF, 0xFE, 0xFD, 0x00];
    let result = CommandEnvelope::from_bytes(&bytes);
    assert_eq!(
        result,
        Err(CommandEnvelopeError::InvalidInput),
        "invalid UTF-8 must be rejected"
    );
}

// CE-RQ-34: Empty bytes are rejected (they're valid UTF-8 but fail JSON parsing)
#[test]
fn rq_command_envelope_empty_bytes_rejected() {
    let bytes = b"";
    let result = CommandEnvelope::from_bytes(bytes);
    assert!(
        matches!(result, Err(CommandEnvelopeError::InvalidEnvelopeFormat)),
        "empty bytes must be rejected as InvalidEnvelopeFormat (valid UTF-8 but invalid JSON)"
    );
}

// ===========================================================================
// DIMENSION: concurrent-simulated
// Simulate rapid concurrent command processing - metadata isolation
// ===========================================================================

// CE-RQ-35: Many unique envelopes parsed rapidly maintain distinct identities
#[test]
fn rq_command_envelope_many_unique_envelopes_maintain_distinct_identities() {
    let mut ids = HashSet::new();

    for i in 0u64..1000 {
        let json = format!(
            r#"{{
            "version": 1,
            "command_id": "cmd-{}",
            "correlation_id": "corr-{}",
            "causation_id": "cause-{}",
            "issuer": "system",
            "issued_at": {}
        }}"#,
            i,
            i,
            i,
            1700000000 + i
        );

        let env = CommandEnvelope::from_str(&json).unwrap();
        ids.insert(env.metadata.command_id.clone());
    }

    assert_eq!(
        ids.len(),
        1000,
        "1000 unique command_ids must produce 1000 unique IdempotencyKeys"
    );
}

// CE-RQ-36: All three identity fields independently maintain uniqueness across 100 envelopes
#[test]
fn rq_command_envelope_three_identity_fields_maintain_independent_uniqueness() {
    let cmd_ids: HashSet<_> = (0..100)
        .map(|i| {
            CommandEnvelope::from_str(&format!(
                r#"{{
        "version": 1,
        "command_id": "cmd-{}",
        "correlation_id": "corr-0",
        "causation_id": "cause-0",
        "issuer": "system",
        "issued_at": 1700000000
    }}"#,
                i
            ))
            .unwrap()
            .metadata
            .command_id
        })
        .collect();

    let corr_ids: HashSet<_> = (0..100)
        .map(|i| {
            CommandEnvelope::from_str(&format!(
                r#"{{
        "version": 1,
        "command_id": "cmd-0",
        "correlation_id": "corr-{}",
        "causation_id": "cause-0",
        "issuer": "system",
        "issued_at": 1700000000
    }}"#,
                i
            ))
            .unwrap()
            .metadata
            .correlation_id
        })
        .collect();

    let cause_ids: HashSet<_> = (0..100)
        .map(|i| {
            CommandEnvelope::from_str(&format!(
                r#"{{
        "version": 1,
        "command_id": "cmd-0",
        "correlation_id": "corr-0",
        "causation_id": "cause-{}",
        "issuer": "system",
        "issued_at": 1700000000
    }}"#,
                i
            ))
            .unwrap()
            .metadata
            .causation_id
        })
        .collect();

    assert_eq!(cmd_ids.len(), 100, "100 unique command_ids");
    assert_eq!(corr_ids.len(), 100, "100 unique correlation_ids");
    assert_eq!(cause_ids.len(), 100, "100 unique causation_ids");
}

// CE-RQ-37: Same command_id with different correlation_id/causation_id preserves command_id uniqueness
#[test]
fn rq_command_envelope_command_id_preserves_uniqueness_regardless_of_other_fields() {
    let base = CommandEnvelope::from_str(
        r#"{
        "version": 1,
        "command_id": "cmd-unique",
        "correlation_id": "corr-original",
        "causation_id": "cause-original",
        "issuer": "system",
        "issued_at": 1700000000
    }"#,
    )
    .unwrap();

    let modified = CommandEnvelope::from_str(
        r#"{
        "version": 1,
        "command_id": "cmd-unique",
        "correlation_id": "corr-modified",
        "causation_id": "cause-modified",
        "issuer": "ai_agent",
        "issued_at": 9999999999
    }"#,
    )
    .unwrap();

    // command_id is the same, so these should be equal in identity
    assert_eq!(base.metadata.command_id, modified.metadata.command_id);

    // But other fields differ
    assert_ne!(
        base.metadata.correlation_id,
        modified.metadata.correlation_id
    );
    assert_ne!(base.metadata.causation_id, modified.metadata.causation_id);
    assert_ne!(base.metadata.issuer, modified.metadata.issuer);
}

// ===========================================================================
// DIMENSION: error-semantics
// Error messages are correct and non-misleading
// ===========================================================================

// CE-RQ-38: Missing command_id error message contains field name
#[test]
fn rq_command_envelope_missing_command_id_error_contains_field_name() {
    let json = r#"{
        "version": 1,
        "correlation_id": "corr-001",
        "causation_id": "cause-001",
        "issuer": "system",
        "issued_at": 1700000000
    }"#;
    let result = CommandEnvelope::from_str(json);
    match result {
        Err(CommandEnvelopeError::MissingEnvelopeField(field)) => {
            assert_eq!(field, "command_id", "error must name the missing field");
        }
        other => panic!("expected MissingEnvelopeField(command_id), got {:?}", other),
    }
}

// CE-RQ-39: Missing correlation_id error message contains field name
#[test]
fn rq_command_envelope_missing_correlation_id_error_contains_field_name() {
    let json = r#"{
        "version": 1,
        "command_id": "cmd-001",
        "causation_id": "cause-001",
        "issuer": "system",
        "issued_at": 1700000000
    }"#;
    let result = CommandEnvelope::from_str(json);
    match result {
        Err(CommandEnvelopeError::MissingEnvelopeField(field)) => {
            assert_eq!(field, "correlation_id", "error must name the missing field");
        }
        other => panic!(
            "expected MissingEnvelopeField(correlation_id), got {:?}",
            other
        ),
    }
}

// CE-RQ-40: Missing causation_id error message contains field name
#[test]
fn rq_command_envelope_missing_causation_id_error_contains_field_name() {
    let json = r#"{
        "version": 1,
        "command_id": "cmd-001",
        "correlation_id": "corr-001",
        "issuer": "system",
        "issued_at": 1700000000
    }"#;
    let result = CommandEnvelope::from_str(json);
    match result {
        Err(CommandEnvelopeError::MissingEnvelopeField(field)) => {
            assert_eq!(field, "causation_id", "error must name the missing field");
        }
        other => panic!(
            "expected MissingEnvelopeField(causation_id), got {:?}",
            other
        ),
    }
}

// CE-RQ-41: Unsupported version error contains the invalid version
#[test]
fn rq_command_envelope_unsupported_version_error_contains_version() {
    let json = r#"{
        "version": 99,
        "command_id": "cmd-001",
        "correlation_id": "corr-001",
        "causation_id": "cause-001",
        "issuer": "system",
        "issued_at": 1700000000
    }"#;
    let result = CommandEnvelope::from_str(json);
    match result {
        Err(CommandEnvelopeError::UnsupportedEnvelopeVersion(v)) => {
            assert_eq!(v, 99, "error must contain the invalid version");
        }
        other => panic!("expected UnsupportedEnvelopeVersion(99), got {:?}", other),
    }
}

// CE-RQ-42: InvalidEnvelopeField error message for issuer contains "issuer"
#[test]
fn rq_command_envelope_invalid_issuer_error_message_contains_issuer() {
    let json = r#"{
        "version": 1,
        "command_id": "cmd-001",
        "correlation_id": "corr-001",
        "causation_id": "cause-001",
        "issuer": "not_an_issuer",
        "issued_at": 1700000000
    }"#;
    let result = CommandEnvelope::from_str(json);
    match result {
        Err(CommandEnvelopeError::InvalidEnvelopeField(msg)) => {
            assert!(
                msg.contains("issuer") || msg.contains("unknown issuer"),
                "error message should mention issuer: got '{}'",
                msg
            );
        }
        other => panic!(
            "expected InvalidEnvelopeField for bad issuer, got {:?}",
            other
        ),
    }
}

// ===========================================================================
// DIMENSION: parse-determinism
// Same input always produces same output
// ===========================================================================

// CE-RQ-43: Parse is deterministic - same input produces same output
#[test]
fn rq_command_envelope_parse_is_deterministic() {
    let json = r#"{
        "version": 1,
        "command_id": "cmd-det",
        "correlation_id": "corr-det",
        "causation_id": "cause-det",
        "issuer": "timer_loop",
        "issued_at": 1700000000
    }"#;

    let r1 = CommandEnvelope::from_str(json).unwrap();
    let r2 = CommandEnvelope::from_str(json).unwrap();
    assert_eq!(r1, r2, "parse must be deterministic");
}

// CE-RQ-44: Parse error is deterministic - same bad input produces same error
#[test]
fn rq_command_envelope_parse_error_is_deterministic() {
    let bad_json = r#"{"version": 1}"#;

    let r1 = CommandEnvelope::from_str(bad_json);
    let r2 = CommandEnvelope::from_str(bad_json);

    assert_eq!(r1, r2, "parse error must be deterministic");
    assert_eq!(
        r1.unwrap_err().to_string(),
        r2.unwrap_err().to_string(),
        "error message must be identical"
    );
}

// ===========================================================================
// DIMENSION: trait-compliance
// Required traits are properly implemented
// ===========================================================================

// CE-RQ-45: CommandEnvelope is Clone
#[test]
fn rq_command_envelope_is_clone() {
    fn require_clone<T: Clone>(_v: T) {}
    let env = CommandEnvelope::from_str(
        r#"{
        "version": 1,
        "command_id": "cmd-clone",
        "correlation_id": "corr-clone",
        "causation_id": "cause-clone",
        "issuer": "system",
        "issued_at": 1700000000
    }"#,
    )
    .unwrap();
    require_clone(env.clone());
}

// CE-RQ-46: CommandEnvelope is PartialEq
#[test]
fn rq_command_envelope_is_partial_eq() {
    let json = r#"{
        "version": 1,
        "command_id": "cmd-eq",
        "correlation_id": "corr-eq",
        "causation_id": "cause-eq",
        "issuer": "system",
        "issued_at": 1700000000
    }"#;
    let env1 = CommandEnvelope::from_str(json).unwrap();
    let env2 = CommandEnvelope::from_str(json).unwrap();
    assert_eq!(env1, env2);
}

// CE-RQ-47: CommandMetadata is Clone
#[test]
fn rq_command_metadata_is_clone() {
    fn require_clone<T: Clone>(_v: T) {}
    let metadata = CommandMetadata {
        command_id: IdempotencyKey::parse("cmd-clone").unwrap(),
        correlation_id: IdempotencyKey::parse("corr-clone").unwrap(),
        causation_id: IdempotencyKey::parse("cause-clone").unwrap(),
        issuer: Issuer::System,
        issued_at: TimestampMs::try_from(1700000000u64).unwrap(),
    };
    require_clone(metadata.clone());
}

// CE-RQ-48: IdempotencyKey is PartialEq
#[test]
fn rq_idempotency_key_is_partial_eq() {
    let key1 = IdempotencyKey::parse("cmd-test").unwrap();
    let key2 = IdempotencyKey::parse("cmd-test").unwrap();
    let key3 = IdempotencyKey::parse("cmd-other").unwrap();
    assert_eq!(key1, key2);
    assert_ne!(key1, key3);
}

// CE-RQ-49: IdempotencyKey supports Eq (reflexive, transitive, symmetric)
#[test]
fn rq_idempotency_key_is_eq() {
    fn require_eq<T: Eq>(_v: T) {}
    let key = IdempotencyKey::parse("cmd-eq").unwrap();
    require_eq(key);
}

// CE-RQ-50: IdempotencyKey implements Hash (needed for HashSet)
#[test]
fn rq_idempotency_key_implements_hash() {
    use std::collections::HashMap;
    let mut map = HashMap::new();
    let key1 = IdempotencyKey::parse("cmd-hash").unwrap();
    let key2 = IdempotencyKey::parse("cmd-hash").unwrap();
    let key3 = IdempotencyKey::parse("cmd-other").unwrap();

    map.insert(key1.clone(), "value1");
    assert_eq!(map.get(&key2), Some(&"value1"));
    assert_eq!(map.get(&key3), None);
}

// CE-RQ-51: Issuer derives Clone, Copy, PartialEq, Eq
#[test]
fn rq_issuer_traits() {
    fn require_clone<T: Clone>() {}
    fn require_copy<T: Copy>() {}
    fn require_partial_eq<T: PartialEq>() {}
    fn require_eq<T: Eq>() {}

    require_clone::<Issuer>();
    require_copy::<Issuer>();
    require_partial_eq::<Issuer>();
    require_eq::<Issuer>();
}
