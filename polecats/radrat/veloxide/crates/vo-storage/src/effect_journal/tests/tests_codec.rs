#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
use super::super::{
    decode_effect_key, decode_effect_record, encode_effect_key, encode_effect_record, EffectId,
    EffectJournalError,
};
use serde_json::json;
use vo_types::{EffectIntent, EffectKind, EffectRecord, InstanceId};

// Helper: decode JSON bytes (uses unwrap which is allowed in tests)
fn decode_json_lease(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).unwrap()
}

// ========================================================================
// Calc Layer — Key Encode/Decode
// ========================================================================

#[test]
fn encode_effect_key_produces_utf8_bytes() {
    let id = InstanceId::from_bytes([1u8; 16]);
    let eid = EffectId::new(&id, "instance::fx-123").unwrap();
    let bytes = encode_effect_key(&eid);
    assert_eq!(bytes, eid.as_str().as_bytes());
}

#[test]
fn decode_effect_key_recovers_effect_id() {
    let id = InstanceId::from_bytes([1u8; 16]);
    let eid = EffectId::new(&id, "test-key").unwrap();
    let bytes = encode_effect_key(&eid);
    let recovered = decode_effect_key(&bytes).unwrap();
    assert_eq!(recovered, eid);
}

#[test]
fn decode_effect_key_returns_error_for_invalid_utf8() {
    let bad_bytes: &[u8] = &[0xFF, 0xFE];
    assert_eq!(
        decode_effect_key(bad_bytes),
        Err(EffectJournalError::Codec {
            reason: "invalid utf-8 sequence of 1 bytes from index 0".to_string(),
        })
    );
}

#[test]
fn decode_effect_key_returns_error_for_empty_bytes() {
    assert_eq!(
        decode_effect_key(&[]),
        Err(EffectJournalError::Codec {
            reason: "empty effect key".to_string(),
        })
    );
}

// ========================================================================
// Calc Layer — Record Encode/Decode
// ========================================================================

#[test]
fn encode_decode_effect_record_roundtrip() {
    let record = EffectRecord::new(
        "fx-roundtrip".to_string(),
        EffectKind::HttpCall,
        json!({"url": "https://example.com"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let bytes = encode_effect_record(&record).unwrap();
    let recovered = decode_effect_record(&bytes).unwrap();
    assert_eq!(recovered, record);
}

#[test]
fn decode_effect_record_returns_error_for_invalid_json() {
    assert_eq!(
        decode_effect_record(b"not-json"),
        Err(EffectJournalError::Codec {
            reason: "expected ident at line 1 column 2".to_string(),
        })
    );
}

#[test]
fn encode_decode_record_roundtrip_for_prepared_status() {
    let ts = vo_types::TimestampMs::parse("42").unwrap();
    let record = EffectRecord::new(
        "fx-status-prepared".to_string(),
        EffectKind::SqlQuery,
        json!({"q": "SELECT 1"}),
        EffectIntent::Prepared,
        Some(ts),
    )
    .unwrap();
    let bytes = encode_effect_record(&record).unwrap();
    assert!(!bytes.is_empty(), "encoded bytes must not be empty");
    let json_obj: serde_json::Value = decode_json_lease(&bytes);
    assert_eq!(
        json_obj.get("intent_id").and_then(|v| v.as_str()),
        Some("fx-status-prepared"),
        "encoded JSON must preserve intent_id"
    );
    assert_eq!(
        json_obj.get("status").and_then(|v| v.as_str()),
        Some("Prepared"),
        "encoded JSON must preserve Prepared status"
    );
    let recovered = decode_effect_record(&bytes).unwrap();
    assert_eq!(recovered.intent_id(), record.intent_id());
    assert_eq!(recovered.status(), EffectIntent::Prepared);
}

#[test]
fn encode_decode_record_roundtrip_for_committed_status() {
    let ts = vo_types::TimestampMs::parse("42").unwrap();
    let record = EffectRecord::new(
        "fx-status-committed".to_string(),
        EffectKind::SqlQuery,
        json!({"q": "SELECT 2"}),
        EffectIntent::Committed,
        Some(ts),
    )
    .unwrap();
    let bytes = encode_effect_record(&record).unwrap();
    assert!(!bytes.is_empty(), "encoded bytes must not be empty");
    let json_obj: serde_json::Value = decode_json_lease(&bytes);
    assert_eq!(
        json_obj.get("intent_id").and_then(|v| v.as_str()),
        Some("fx-status-committed"),
        "encoded JSON must preserve intent_id"
    );
    assert_eq!(
        json_obj.get("status").and_then(|v| v.as_str()),
        Some("Committed"),
        "encoded JSON must preserve Committed status"
    );
    let recovered = decode_effect_record(&bytes).unwrap();
    assert_eq!(recovered.intent_id(), record.intent_id());
    assert_eq!(recovered.status(), EffectIntent::Committed);
}

#[test]
fn encode_decode_record_roundtrip_for_rolledback_status() {
    let ts = vo_types::TimestampMs::parse("42").unwrap();
    let record = EffectRecord::new(
        "fx-status-rolledback".to_string(),
        EffectKind::SqlQuery,
        json!({"q": "SELECT 3"}),
        EffectIntent::RolledBack,
        Some(ts),
    )
    .unwrap();
    let bytes = encode_effect_record(&record).unwrap();
    assert!(!bytes.is_empty(), "encoded bytes must not be empty");
    let json_obj: serde_json::Value = decode_json_lease(&bytes);
    assert_eq!(
        json_obj.get("intent_id").and_then(|v| v.as_str()),
        Some("fx-status-rolledback"),
        "encoded JSON must preserve intent_id"
    );
    assert_eq!(
        json_obj.get("status").and_then(|v| v.as_str()),
        Some("RolledBack"),
        "encoded JSON must preserve RolledBack status"
    );
    let recovered = decode_effect_record(&bytes).unwrap();
    assert_eq!(recovered.intent_id(), record.intent_id());
    assert_eq!(recovered.status(), EffectIntent::RolledBack);
}

// ========================================================================
// Schema Evolution Tests
// ========================================================================

#[test]
fn schema_evolution_old_format_without_committed_at_through_codec() {
    let old_json = br#"{"intent_id":"fx-codec-old","kind":"HttpCall","params_json":{},"status":"Prepared"}"#;
    let record = decode_effect_record(old_json).unwrap();
    assert_eq!(record.intent_id(), "fx-codec-old");
    assert_eq!(record.kind(), EffectKind::HttpCall);
    assert_eq!(record.status(), EffectIntent::Prepared);
    assert_eq!(record.committed_at(), None);

    // Round-trip: re-encode and decode
    let bytes = encode_effect_record(&record).unwrap();
    let recovered = decode_effect_record(&bytes).unwrap();
    assert_eq!(record, recovered);
}

#[test]
fn schema_evolution_future_fields_stripped_on_reencode() {
    let future_json = br#"{"intent_id":"fx-codec-future","kind":"SqlQuery","params_json":{"q":"SELECT 1"},"status":"Committed","committed_at":999,"new_field":"hello","another":true}"#;
    let record = decode_effect_record(future_json).unwrap();
    assert_eq!(record.intent_id(), "fx-codec-future");
    assert_eq!(record.status(), EffectIntent::Committed);

    // Re-encode strips unknown fields but preserves known data
    let bytes = encode_effect_record(&record).unwrap();
    let json_val: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(
        json_val.get("new_field").is_none(),
        "unknown fields must be stripped on re-encode"
    );
    assert!(
        json_val.get("another").is_none(),
        "unknown fields must be stripped on re-encode"
    );
    let recovered = decode_effect_record(&bytes).unwrap();
    assert_eq!(record, recovered);
}

#[test]
fn schema_evolution_null_committed_at_through_codec() {
    let null_json = br#"{"intent_id":"fx-null-ts","kind":"BlobWrite","params_json":{},"status":"Prepared","committed_at":null}"#;
    let record = decode_effect_record(null_json).unwrap();
    assert_eq!(record.committed_at(), None);
}

#[test]
fn schema_evolution_all_kinds_old_format() {
    for kind_str in &["HttpCall", "SqlQuery", "BlobWrite"] {
        let old_json = format!(
            r#"{{"intent_id":"fx-kind-test","kind":"{kind_str}","params_json":{{}},"status":"Prepared"}}"#
        );
        let record = decode_effect_record(old_json.as_bytes()).unwrap();
        assert_eq!(record.committed_at(), None);
        let bytes = encode_effect_record(&record).unwrap();
        let recovered = decode_effect_record(&bytes).unwrap();
        assert_eq!(record, recovered);
    }
}
