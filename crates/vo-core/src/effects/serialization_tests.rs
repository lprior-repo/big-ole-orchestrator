//! Serialization round-trip tests for vo-core effect domain types.
//!
//! Tests verify that EffectRecord serializes and deserializes correctly
//! preserving all fields across JSON round-trips.

use super::*;
use vo_types::TimestampMs;

fn make_effect(status: EffectIntent) -> EffectRecord {
    EffectRecord::new(
        "fx-test-001".to_string(),
        EffectKind::HttpCall,
        serde_json::json!({"url": "https://example.com"}),
        status,
        None,
    )
    .expect("valid test data")
}

// =============================================================================
// EffectRecord Serialization Round-Trip
// =============================================================================

#[test]
fn effectrecord_serialize_and_deserialize_prepared_round_trip() {
    let effect = make_effect(EffectIntent::Prepared);
    let json = serde_json::to_string(&effect).expect("serializes");
    let recovered: EffectRecord = serde_json::from_str(&json).expect("deserializes");
    assert_eq!(recovered, effect);
}

#[test]
fn effectrecord_serialize_and_deserialize_committed_round_trip() {
    let now = TimestampMs::try_from(1_700_000_000_000u64).unwrap();
    let effect = EffectRecord::new(
        "fx-test-002".to_string(),
        EffectKind::BlobWrite,
        serde_json::json!({"bucket": "test-bucket", "key": "test-key"}),
        EffectIntent::Committed,
        Some(now),
    )
    .expect("valid test data");
    let json = serde_json::to_string(&effect).expect("serializes");
    let recovered: EffectRecord = serde_json::from_str(&json).expect("deserializes");
    assert_eq!(recovered, effect);
}

#[test]
fn effectrecord_serialize_and_deserialize_rolledback_round_trip() {
    let effect = EffectRecord::new(
        "fx-test-003".to_string(),
        EffectKind::SqlQuery,
        serde_json::json!({"query": "DELETE FROM users"}),
        EffectIntent::RolledBack,
        None,
    )
    .expect("valid test data");
    let json = serde_json::to_string(&effect).expect("serializes");
    let recovered: EffectRecord = serde_json::from_str(&json).expect("deserializes");
    assert_eq!(recovered, effect);
}

#[test]
fn effectrecord_serialize_deserialize_preserves_all_fields() {
    let now = TimestampMs::try_from(1_700_000_000_000u64).unwrap();
    let params = serde_json::json!({
        "method": "POST",
        "url": "https://api.stripe.com/v1/charges",
        "headers": {"Authorization": "Bearer sk_test"},
        "body": {"amount": 1000, "currency": "usd"}
    });
    let effect = EffectRecord::new(
        "fx-stripe-001".to_string(),
        EffectKind::HttpCall,
        params.clone(),
        EffectIntent::Prepared,
        None,
    )
    .expect("valid test data");

    let json = serde_json::to_string(&effect).expect("serializes");
    let recovered: EffectRecord = serde_json::from_str(&json).expect("deserializes");

    assert_eq!(recovered.intent_id(), "fx-stripe-001");
    assert_eq!(recovered.kind(), EffectKind::HttpCall);
    assert_eq!(recovered.params_json(), &params);
    assert_eq!(recovered.status(), EffectIntent::Prepared);
    assert_eq!(recovered.committed_at(), None);

    let committed = commit_effect(&recovered, now).expect("commits");
    let committed_json = serde_json::to_string(&committed).expect("serializes");
    let recovered_committed: EffectRecord =
        serde_json::from_str(&committed_json).expect("deserializes");
    assert_eq!(recovered_committed.status(), EffectIntent::Committed);
    assert_eq!(recovered_committed.committed_at(), Some(&now));
}