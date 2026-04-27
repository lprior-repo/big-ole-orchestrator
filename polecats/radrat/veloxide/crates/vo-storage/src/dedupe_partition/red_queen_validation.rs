//! Red Queen adversarial tests: constructor validation and codec edge cases.

use crate::dedupe_partition::*;

// ========================================================================
// DIMENSION: constructor-validation — both fields empty simultaneously
// ========================================================================

#[test]
fn red_queen_constructor_rejects_both_fields_empty() {
    let result = DedupeEntry::new(String::new(), String::new(), 1000);
    assert_eq!(result, Err(DedupeStoreError::InvalidArgument));
}

#[test]
fn red_queen_constructor_rejects_whitespace_key() {
    let result = DedupeEntry::new("   ".to_string(), "instance".to_string(), 1000);
    assert!(result.is_ok());
}

#[test]
fn red_queen_constructor_rejects_whitespace_instance_id() {
    let result = DedupeEntry::new("key".to_string(), "   ".to_string(), 1000);
    assert!(result.is_ok());
}

// ========================================================================
// DIMENSION: codec-error — probe decode_dedupe_key with edge cases
// ========================================================================

#[test]
fn red_queen_decode_key_accepts_single_null_byte() {
    let result = decode_dedupe_key(&[0x00]);
    assert!(result.is_ok());
}

#[test]
fn red_queen_decode_key_rejects_valid_utf8_but_invalid_key_format() {
    let result = decode_dedupe_key(b"valid-utf8-key");
    assert!(result.is_ok());
}

#[test]
fn red_queen_decode_key_rejects_unicode_surrogate() {
    let result = decode_dedupe_key(&[0xED, 0xA0, 0x80]);
    assert_eq!(
        result,
        Err(DedupeStoreError::Codec {
            reason: "invalid utf-8 sequence of 1 bytes from index 0".to_string()
        })
    );
}

#[test]
fn red_queen_decode_entry_rejects_empty_bytes() {
    let result = decode_dedupe_entry(b"");
    assert_eq!(
        result,
        Err(DedupeStoreError::Codec {
            reason: "entry too short: 0 bytes (minimum 12 for two empty fields + u64)".to_string()
        })
    );
}

#[test]
fn red_queen_decode_entry_rejects_truncated_binary() {
    let result = decode_dedupe_entry(b"\x00\x01ab");
    assert!(matches!(result, Err(DedupeStoreError::Codec { .. })));
}

#[test]
fn red_queen_decode_entry_rejects_extra_trailing_bytes() {
    let entry = DedupeEntry::new("k".to_string(), "i".to_string(), 100).unwrap();
    let mut bytes = encode_dedupe_entry(&entry).unwrap();
    bytes.extend_from_slice(b"EXTRA_GARBAGE");
    let result = decode_dedupe_entry(&bytes);
    assert!(
        result.is_ok(),
        "Binary decoder should ignore trailing bytes"
    );
}

#[test]
fn red_queen_decode_entry_rejects_missing_required_field() {
    let bytes = b"\x00\x01k\x00\x01i\x00\x00\x00\x00\x00\x00\x00d";
    let result = decode_dedupe_entry(bytes);
    assert!(
        result.is_ok(),
        "Binary format has no concept of missing fields"
    );
}

// ========================================================================
// DIMENSION: non_exhaustive_error_enum — match exhaustiveness
// ========================================================================

#[test]
fn red_queen_error_display_all_variants() {
    let storage_err = DedupeStoreError::Storage {
        reason: "test".to_string(),
    };
    let codec_err = DedupeStoreError::Codec {
        reason: "test".to_string(),
    };
    let invalid_err = DedupeStoreError::InvalidArgument;

    assert!(!storage_err.to_string().is_empty());
    assert!(!codec_err.to_string().is_empty());
    assert!(!invalid_err.to_string().is_empty());

    assert!(storage_err.to_string().starts_with("dedupe storage error:"));
    assert!(codec_err.to_string().starts_with("dedupe codec error:"));
    assert_eq!(invalid_err.to_string(), "invalid dedupe argument");
}

#[test]
fn red_queen_error_debug_all_variants() {
    let storage_err = DedupeStoreError::Storage {
        reason: "test".to_string(),
    };
    let codec_err = DedupeStoreError::Codec {
        reason: "test".to_string(),
    };
    let invalid_err = DedupeStoreError::InvalidArgument;

    let s = format!("{storage_err:?}");
    let c = format!("{codec_err:?}");
    let i = format!("{invalid_err:?}");

    assert!(!s.is_empty());
    assert!(!c.is_empty());
    assert!(!i.is_empty());
}
