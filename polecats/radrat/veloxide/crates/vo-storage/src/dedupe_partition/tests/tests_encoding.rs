#![allow(clippy::unwrap_used)]
//! Unit tests for encode/decode functions.

use super::*;

// ========================================================================
// Calc Layer — Key Encode/Decode
// ========================================================================

#[test]
fn encode_dedupe_key_produces_utf8_bytes() {
    let key = DedupeKey::parse("test-key").unwrap();
    let bytes = encode_dedupe_key(&key);
    assert_eq!(bytes, b"test-key");
}

#[test]
fn decode_dedupe_key_recovers_key() {
    let key = DedupeKey::parse("test-key").unwrap();
    let bytes = encode_dedupe_key(&key);
    let recovered = decode_dedupe_key(&bytes).unwrap();
    assert_eq!(recovered.as_str(), "test-key");
}

#[test]
fn decode_dedupe_key_returns_exact_codec_error_for_invalid_utf8_bytes() {
    let result = decode_dedupe_key(&[0xFF, 0xFE]);
    assert_eq!(
        result,
        Err(DedupeStoreError::Codec {
            reason: "invalid utf-8 sequence of 1 bytes from index 0".to_string()
        })
    );
}

#[test]
fn decode_dedupe_key_returns_exact_codec_error_for_empty_bytes() {
    let result = decode_dedupe_key(&[]);
    assert_eq!(
        result,
        Err(DedupeStoreError::Codec {
            reason: "DedupeKey: value must not be empty".to_string()
        })
    );
}

#[test]
fn decode_dedupe_key_returns_error_for_key_exceeding_256_bytes() {
    let long_key = "a".repeat(257);
    let result = decode_dedupe_key(long_key.as_bytes());
    assert_eq!(
        result,
        Err(DedupeStoreError::Codec {
            reason: "DedupeKey: exceeds maximum length of 256 (got 257)".to_string()
        })
    );
}

#[test]
fn encode_dedupe_key_preserves_unicode_bytes() {
    let key = DedupeKey::parse("dedupe-π").unwrap();
    let bytes = encode_dedupe_key(&key);
    assert_eq!(bytes, "dedupe-π".as_bytes());
}

#[test]
fn encode_dedupe_key_never_returns_empty_for_valid_key() {
    let key = DedupeKey::parse("testkey").unwrap();
    let bytes = encode_dedupe_key(&key);
    assert!(!bytes.is_empty());
    assert_eq!(bytes.len(), key.as_str().len());
    assert!(bytes.len() > 1);
}

#[test]
fn encode_dedupe_key_produces_exact_bytes_for_multibyte_key() {
    let key = DedupeKey::parse("日本語テストキー").unwrap();
    let bytes = encode_dedupe_key(&key);
    assert_eq!(bytes, "日本語テストキー".as_bytes());
    assert!(bytes.len() > 1);
}

// ========================================================================
// Calc Layer — Entry Encode/Decode
// ========================================================================

#[test]
fn encode_decode_dedupe_entry_roundtrip() {
    let entry = DedupeEntry::new("key-1".to_string(), "instance-1".to_string(), 5000).unwrap();
    let bytes = encode_dedupe_entry(&entry).unwrap();
    let recovered = decode_dedupe_entry(&bytes).unwrap();
    assert_eq!(recovered, entry);
}

#[test]
fn decode_dedupe_entry_returns_codec_error_for_truncated_binary_bytes() {
    let result = decode_dedupe_entry(b"\x00\x01");
    assert!(matches!(result, Err(DedupeStoreError::Codec { .. })));
}

#[test]
fn encode_dedupe_entry_never_returns_empty_for_valid_entry() {
    let entry = DedupeEntry::new(
        "unique-key-7ffu".to_string(),
        "unique-iid-7ffu".to_string(),
        9999,
    )
    .unwrap();
    let bytes = encode_dedupe_entry(&entry).unwrap();
    assert!(!bytes.is_empty());
    assert!(bytes.len() > 1);
    assert!(bytes.len() >= 12);
}

#[test]
fn encode_dedupe_entry_produces_binary_with_correct_structure() {
    let entry = DedupeEntry::new("k1".to_string(), "i1".to_string(), 12345).unwrap();
    let bytes = encode_dedupe_entry(&entry).unwrap();
    assert_eq!(bytes.len(), 2 + 2 + 2 + 2 + 8);
    let dk_len = u16::from_be_bytes([bytes[0], bytes[1]]) as usize;
    assert_eq!(&bytes[2..2 + dk_len], b"k1");
    let offset = 2 + dk_len;
    let iid_len = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]) as usize;
    assert_eq!(&bytes[offset + 2..offset + 2 + iid_len], b"i1");
    let ts_offset = offset + 2 + iid_len;
    let expires_at = u64::from_be_bytes(bytes[ts_offset..ts_offset + 8].try_into().unwrap());
    assert_eq!(expires_at, 12345);
}

// ========================================================================
// Kani Verification Stubs
// ========================================================================

#[test]
fn kani_verify_dedupe_entry_rejects_empty_key() {
    assert_eq!(
        DedupeEntry::new(String::new(), "instance-1".to_string(), 1_000),
        Err(DedupeStoreError::InvalidArgument)
    );
}

#[test]
fn kani_verify_encode_decode_dedupe_key_roundtrip() {
    let key = DedupeKey::parse("verify-key").unwrap();
    let bytes = encode_dedupe_key(&key);
    assert_eq!(decode_dedupe_key(&bytes), Ok(key));
}

#[test]
fn verification_source_keeps_both_kani_proof_gates_present() {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/dedupe_partition/verification.rs"
    ))
    .unwrap();
    assert_eq!(
        source
            .matches("fn verify_dedupe_entry_rejects_empty_key_returns_invalid_argument()")
            .count(),
        1
    );
    assert_eq!(
        source
            .matches("fn verify_encode_decode_dedupe_key_roundtrip_returns_original_key()")
            .count(),
        1
    );
    assert_eq!(source.matches("#[kani::proof]").count(), 2);
}

#[test]
fn verification_source_asserts_empty_key_proof_contains_exact_invalid_argument_assertion() {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/dedupe_partition/verification.rs"
    ))
    .unwrap();
    assert!(source.contains("assert_eq!(result, Err(DedupeStoreError::InvalidArgument))"));
}

#[test]
fn verification_source_asserts_roundtrip_proof_contains_encode_decode_equality_assertion() {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/dedupe_partition/verification.rs"
    ))
    .unwrap();
    assert!(source.contains("assert_eq!(decode_dedupe_key(&encoded), Ok(dedupe_key))"));
}
