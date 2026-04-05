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
