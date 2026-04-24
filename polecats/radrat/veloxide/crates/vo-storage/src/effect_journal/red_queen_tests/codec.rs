//! Red Queen tests — key and record codec.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::super::{
    decode_effect_key, decode_effect_record, encode_effect_key, encode_effect_record, EffectId,
    EffectJournalError, InstanceId,
};
use vo_types::{EffectIntent, EffectKind};

// Helper
fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

// ========================================================================
// DIMENSION: key-codec
// Contract: encode/decode roundtrip, invalid UTF-8 returns Codec error
// ========================================================================

#[test]
fn red_queen_encode_decode_key_roundtrip_preserves_id() {
    let id = InstanceId::from_bytes([1u8; 16]);
    let effect_id = EffectId::new(&id, "roundtrip-test").unwrap();
    let bytes = encode_effect_key(&effect_id);
    let recovered = decode_effect_key(&bytes).unwrap();
    assert_eq!(
        recovered, effect_id,
        "BUG: key codec roundtrip changed EffectId"
    );
}

#[test]
fn red_queen_decode_effect_key_rejects_invalid_utf8() {
    let bad_bytes: Vec<u8> = vec![0x80, 0x81, 0xFF, 0xFE];
    let result = decode_effect_key(&bad_bytes);
    assert!(
        result.is_err(),
        "BUG: decode_effect_key accepted invalid UTF-8"
    );
    match result.unwrap_err() {
        EffectJournalError::Codec { .. } => {} // Expected
        other => panic!("BUG: Wrong error variant for invalid UTF-8: {:?}", other),
    }
}

#[test]
fn red_queen_decode_effect_key_rejects_empty_bytes() {
    let result = decode_effect_key(&[]);
    assert!(
        result.is_err(),
        "BUG: decode_effect_key accepted empty bytes"
    );
    match result.unwrap_err() {
        EffectJournalError::Codec { reason } => {
            assert!(
                reason.contains("empty"),
                "BUG: Empty key error message doesn't mention 'empty'"
            );
        }
        other => panic!("BUG: Wrong error variant for empty bytes: {:?}", other),
    }
}

#[test]
fn red_queen_encode_effect_key_produces_utf8() {
    let id = InstanceId::from_bytes([1u8; 16]);
    let effect_id = EffectId::new(&id, "utf8-test").unwrap();
    let bytes = encode_effect_key(&effect_id);
    // Must be valid UTF-8
    let s = String::from_utf8(bytes.clone()).expect("BUG: encode_effect_key produced non-UTF-8");
    assert_eq!(
        s,
        effect_id.as_str(),
        "BUG: UTF-8 encoding doesn't match original"
    );
}

// ========================================================================
// DIMENSION: record-codec
// Contract: encode/decode roundtrip preserves EffectRecord
// ========================================================================

#[test]
fn red_queen_encode_decode_record_roundtrip() {
    let record = vo_types::EffectRecord::new(
        "fx-record".to_string(),
        EffectKind::HttpCall,
        serde_json::json!({"url": "https://example.com"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let bytes = encode_effect_record(&record).expect("BUG: encode failed");
    let recovered = decode_effect_record(&bytes).expect("BUG: decode failed");
    assert_eq!(
        recovered, record,
        "BUG: record codec roundtrip changed EffectRecord"
    );
}

#[test]
fn red_queen_decode_effect_record_rejects_invalid_json() {
    let result = decode_effect_record(b"not json at all");
    assert!(
        result.is_err(),
        "BUG: decode_effect_record accepted invalid JSON"
    );
    match result.unwrap_err() {
        EffectJournalError::Codec { .. } => {} // Expected
        other => panic!("BUG: Wrong error variant for invalid JSON: {:?}", other),
    }
}
