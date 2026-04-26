//! Red Queen tests: metadata-preservation dimension for command envelope metadata (ADR-036).
//!
//! bead_id: ve-04w0
//! phase: state-5-red-queen
//!
//! Attacks: Serde round-trip integrity under various conditions

use crate::{
    CommandEnvelope, CommandEnvelopeError, CommandMetadata, IdempotencyKey, Issuer, TimestampMs,
};

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
