//! Schema evolution tests for EffectRecord serialization.
//!
//! bead_id: ve-rm7fs

#![allow(clippy::unwrap_used)]

use serde_json::json;
use crate::{EffectIntent, EffectKind, EffectRecord};

fn make_record(intent_id: &str, kind: EffectKind, status: EffectIntent) -> EffectRecord {
    EffectRecord::new(
        intent_id.to_string(),
        kind,
        json!({"key": "value"}),
        status,
        None,
    )
    .unwrap()
}

#[test]
fn effect_record_current_schema_roundtrips() {
    let record = make_record("fx-001", EffectKind::HttpCall, EffectIntent::Prepared);
    let json = serde_json::to_string(&record).unwrap();
    let recovered: EffectRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(recovered, record);
}

#[test]
fn effect_record_ignores_extra_json_fields() {
    let json_str = r#"{
        "intent_id": "fx-002",
        "kind": "SqlQuery",
        "params_json": {"query": "SELECT 1"},
        "status": "Prepared",
        "committed_at": null,
        "future_field_alpha": 42,
        "future_field_beta": "hello",
        "future_nested": {"a": [1, 2, 3]}
    }"#;
    let record: EffectRecord = serde_json::from_str(json_str).unwrap();
    assert_eq!(record.intent_id(), "fx-002");
    assert_eq!(record.kind(), EffectKind::SqlQuery);
    assert_eq!(record.status(), EffectIntent::Prepared);
    assert_eq!(record.committed_at(), None);
}

#[test]
fn effect_record_rejects_missing_intent_id() {
    let result: Result<EffectRecord, _> = serde_json::from_str(r#"{
        "kind": "HttpCall",
        "params_json": {},
        "status": "Prepared",
        "committed_at": null
    }"#);
    assert!(result.is_err(), "Missing intent_id should fail to deserialize");
}

#[test]
fn effect_record_rejects_missing_kind() {
    let result: Result<EffectRecord, _> = serde_json::from_str(r#"{
        "intent_id": "fx-003",
        "params_json": {},
        "status": "Prepared",
        "committed_at": null
    }"#);
    assert!(result.is_err(), "Missing kind should fail to deserialize");
}

#[test]
fn effect_record_rejects_missing_status() {
    let result: Result<EffectRecord, _> = serde_json::from_str(r#"{
        "intent_id": "fx-003",
        "kind": "HttpCall",
        "params_json": {},
        "committed_at": null
    }"#);
    assert!(result.is_err(), "Missing status should fail to deserialize");
}

#[test]
fn effect_record_accepts_missing_optional_committed_at() {
    let json_str = r#"{
        "intent_id": "fx-004",
        "kind": "BlobWrite",
        "params_json": {"bucket": "b"},
        "status": "Prepared"
    }"#;
    let record: EffectRecord = serde_json::from_str(json_str).unwrap();
    assert_eq!(record.intent_id(), "fx-004");
    assert_eq!(record.committed_at(), None);
}

#[test]
fn effect_record_accepts_any_json_type_for_params_json() {
    // params_json is serde_json::Value, so any JSON primitive is valid
    let json_str = r#"{
        "intent_id": "fx-005",
        "kind": "HttpCall",
        "params_json": "just-a-string",
        "status": "Prepared",
        "committed_at": null
    }"#;
    let record: EffectRecord = serde_json::from_str(json_str).unwrap();
    assert_eq!(record.params_json(), &json!("just-a-string"));
}

#[test]
fn effect_record_rejects_unknown_effect_kind_variant() {
    let result: Result<EffectRecord, _> = serde_json::from_str(r#"{
        "intent_id": "fx-006",
        "kind": "GrpcCall",
        "params_json": {},
        "status": "Prepared",
        "committed_at": null
    }"#);
    assert!(result.is_err(), "Unknown EffectKind variant should be rejected");
}

#[test]
fn effect_record_rejects_unknown_effect_intent_variant() {
    let result: Result<EffectRecord, _> = serde_json::from_str(r#"{
        "intent_id": "fx-007",
        "kind": "HttpCall",
        "params_json": {},
        "status": "Pending",
        "committed_at": null
    }"#);
    assert!(result.is_err(), "Unknown EffectIntent variant should be rejected");
}

#[test]
fn effect_record_all_kinds_roundtrip() {
    for kind in EffectKind::all_variants() {
        let record = make_record("fx-kind-test", *kind, EffectIntent::Prepared);
        let json = serde_json::to_string(&record).unwrap();
        let recovered: EffectRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered.kind(), *kind, "Kind {:?} should roundtrip", kind);
    }
}

#[test]
fn effect_record_all_intents_roundtrip() {
    for intent in EffectIntent::all_variants() {
        let record = make_record("fx-intent-test", EffectKind::HttpCall, *intent);
        let json = serde_json::to_string(&record).unwrap();
        let recovered: EffectRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered.status(), *intent, "Intent {:?} should roundtrip", intent);
    }
}
