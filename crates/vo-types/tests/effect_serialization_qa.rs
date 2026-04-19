//! QA: Effect record serialization verification (ve-rh355)

#![allow(clippy::unwrap_used)]

use serde_json::json;
use vo_types::{EffectIntent, EffectKind, EffectRecord, TimestampMs};

#[test]
fn effect_record_roundtrip_with_committed_at_timestamp() {
    let ts = TimestampMs::new_unchecked(1_710_000_000_000);
    let record = EffectRecord::new(
        "fx-001".to_string(),
        EffectKind::HttpCall,
        json!({"method": "POST", "url": "https://api.example.com"}),
        EffectIntent::Committed,
        Some(ts),
    )
    .unwrap();

    let serialized = serde_json::to_string(&record).unwrap();
    let deserialized: EffectRecord = serde_json::from_str(&serialized).unwrap();

    assert_eq!(deserialized, record);
    assert_eq!(deserialized.committed_at(), Some(&ts));
}

#[test]
fn effect_record_rejects_truncated_json() {
    let truncated = r#"{"intent_id": "fx-002", "kind": "HttpCall", "params_json": {"url":"#;
    let result: Result<EffectRecord, _> = serde_json::from_str(truncated);
    assert!(result.is_err(), "Truncated JSON should fail to deserialize");
}

#[test]
fn effect_record_rejects_wrong_type_for_kind() {
    let result: Result<EffectRecord, _> = serde_json::from_str(r#"{
        "intent_id": "fx-003",
        "kind": 42,
        "params_json": {},
        "status": "Prepared",
        "committed_at": null
    }"#);
    assert!(result.is_err(), "Integer kind should fail to deserialize");
}

#[test]
fn effect_record_accepts_empty_intent_id_via_deserialize() {
    let result: Result<EffectRecord, _> = serde_json::from_str(r#"{
        "intent_id": "",
        "kind": "HttpCall",
        "params_json": {},
        "status": "Prepared",
        "committed_at": null
    }"#);
    // serde accepts empty string — the empty-id guard is in new(), not serde
    assert!(result.is_ok(), "Empty intent_id is valid JSON, serde accepts it");
    assert_eq!(result.unwrap().intent_id(), "");
}
