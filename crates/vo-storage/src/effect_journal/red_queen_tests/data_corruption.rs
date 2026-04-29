//! Red Queen tests — data corruption codec rejection.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::super::{decode_effect_key, decode_effect_record, EffectJournalError};

#[test]
fn red_queen_decode_key_rejects_truncated_utf8() {
    let truncated = vec![0xF0, 0x9F];
    let result = decode_effect_key(&truncated);
    assert!(result.is_err(), "BUG: accepted truncated UTF-8");
    assert!(
        matches!(result.unwrap_err(), EffectJournalError::Codec { .. }),
        "BUG: wrong error variant for truncated UTF-8"
    );
}

#[test]
fn red_queen_decode_record_rejects_truncated_json() {
    let truncated = b"{\"intent_id\": \"fx-1\", \"kind";
    let result = decode_effect_record(truncated);
    assert!(result.is_err(), "BUG: accepted truncated JSON");
    assert!(
        matches!(result.unwrap_err(), EffectJournalError::Codec { .. }),
        "BUG: wrong error variant for truncated JSON"
    );
}

#[test]
fn red_queen_decode_record_rejects_valid_json_wrong_type() {
    let wrong_type = b"42";
    let result = decode_effect_record(wrong_type);
    assert!(
        result.is_err(),
        "BUG: accepted JSON integer as EffectRecord"
    );
}

#[test]
fn red_queen_decode_record_rejects_json_array() {
    let arr = b"[1, 2, 3]";
    let result = decode_effect_record(arr);
    assert!(result.is_err(), "BUG: accepted JSON array as EffectRecord");
}

#[test]
fn red_queen_decode_record_rejects_empty_json_object() {
    let empty = b"{}";
    let result = decode_effect_record(empty);
    assert!(
        result.is_err(),
        "BUG: accepted empty JSON object as EffectRecord"
    );
}

#[test]
fn red_queen_decode_record_rejects_null_bytes() {
    let nulls = vec![0u8; 100];
    let result = decode_effect_record(&nulls);
    assert!(result.is_err(), "BUG: accepted null bytes as EffectRecord");
}
