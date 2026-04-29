//! Corruption detection tests for dedupe_partition verification module.
//!
//! Proves that encode/decode functions catch malformed data.
//! These are unit tests that complement the Kani proofs in verification.rs.
//!
//! bead_id: tw-4454

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::super::*;

// ========================================================================
// decode_dedupe_key — corruption detection tests
// ========================================================================

#[test]
fn verify_decode_dedupe_key_rejects_empty_bytes() {
    let result = decode_dedupe_key(&[]);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), DedupeStoreError::Codec { .. }));
}

#[test]
fn verify_decode_dedupe_key_rejects_non_utf8_bytes() {
    let malformed = vec![0xFF, 0xFE, 0xFD, 0xFC, 0xFB];
    let result = decode_dedupe_key(&malformed);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), DedupeStoreError::Codec { .. }));
}

#[test]
fn verify_decode_dedupe_key_rejects_partial_utf8_continuation() {
    let partial = b"valid-key\x80";
    let result = decode_dedupe_key(partial);
    assert!(result.is_err());
}

#[test]
fn verify_decode_dedupe_key_roundtrip_valid_key() {
    let key = DedupeKey::parse("test-dedupe-key-tw4454").unwrap();
    let encoded = encode_dedupe_key(&key);
    let decoded = decode_dedupe_key(&encoded).unwrap();
    assert_eq!(decoded, key);
}

#[test]
fn verify_decode_dedupe_key_roundtrip_unicode_key() {
    let key = DedupeKey::parse("dedupe-key-日本語-🎉").unwrap();
    let encoded = encode_dedupe_key(&key);
    let decoded = decode_dedupe_key(&encoded).unwrap();
    assert_eq!(decoded, key);
}

#[test]
fn verify_decode_dedupe_key_roundtrip_empty_string_rejected() {
    let key = DedupeKey::parse("").unwrap();
    let encoded = encode_dedupe_key(&key);
    let result = decode_dedupe_key(&encoded);
    assert!(result.is_err());
}

#[test]
fn verify_encode_dedupe_key_produces_valid_utf8() {
    let key = DedupeKey::parse("verify-utf8-tw4454").unwrap();
    let bytes = encode_dedupe_key(&key);
    let result = std::str::from_utf8(&bytes);
    assert!(result.is_ok(), "encode_dedupe_key must produce valid UTF-8");
    assert_eq!(result.unwrap(), key.as_str());
}

// ========================================================================
// decode_dedupe_entry — corruption detection tests
// ========================================================================

#[test]
fn verify_decode_dedupe_entry_rejects_truncated_buffer() {
    let truncated = vec![0x00, 0x03, 0x61, 0x62]; // only partial header
    let result = decode_dedupe_entry(&truncated);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), DedupeStoreError::Codec { .. }));
}

#[test]
fn verify_decode_dedupe_entry_rejects_missing_iid_length() {
    let entry = DedupeEntry::new("dk".to_string(), "iid".to_string(), 1000).unwrap();
    let mut bytes = encode_dedupe_entry(&entry).unwrap();
    bytes.truncate(bytes.len() - 2); // chop last 2 bytes (iid_len)
    let result = decode_dedupe_entry(&bytes);
    assert!(result.is_err());
}

#[test]
fn verify_decode_dedupe_entry_rejects_missing_expires_at() {
    let entry = DedupeEntry::new("dk".to_string(), "iid".to_string(), 1000).unwrap();
    let mut bytes = encode_dedupe_entry(&entry).unwrap();
    bytes.truncate(bytes.len() - 8); // chop last 8 bytes (expires_at)
    let result = decode_dedupe_entry(&bytes);
    assert!(result.is_err());
}

#[test]
fn verify_decode_dedupe_entry_rejects_invalid_utf8_in_dedupe_key() {
    let dk_len = u16::to_be_bytes(4);
    let iid_len = u16::to_be_bytes(3);
    let invalid_utf8 = vec![0x80, 0x80, 0x80, 0x80]; // invalid UTF-8
    let iid_bytes = b"abc".to_vec();
    let expires = 1000u64.to_be_bytes();

    let mut bytes = Vec::new();
    bytes.extend_from_slice(&dk_len);
    bytes.extend_from_slice(&invalid_utf8);
    bytes.extend_from_slice(&iid_len);
    bytes.extend_from_slice(&iid_bytes);
    bytes.extend_from_slice(&expires);

    let result = decode_dedupe_entry(&bytes);
    assert!(result.is_err());
}

#[test]
fn verify_decode_dedupe_entry_rejects_invalid_utf8_in_instance_id() {
    let dk = "valid-dk";
    let iid_len = u16::to_be_bytes(4);
    let invalid_utf8 = vec![0xC0, 0xC0, 0xC0, 0xC0];
    let expires = 1000u64.to_be_bytes();

    let mut bytes = Vec::new();
    bytes.extend_from_slice(&u16::to_be_bytes(dk.len() as u16));
    bytes.extend_from_slice(dk.as_bytes());
    bytes.extend_from_slice(&iid_len);
    bytes.extend_from_slice(&invalid_utf8);
    bytes.extend_from_slice(&expires);

    let result = decode_dedupe_entry(&bytes);
    assert!(result.is_err());
}

#[test]
fn verify_decode_dedupe_entry_roundtrip_valid_entry() {
    let entry = DedupeEntry::new("roundtrip-dk-tw4454".to_string(), "instance-xyz".to_string(), 9999).unwrap();
    let bytes = encode_dedupe_entry(&entry).unwrap();
    let decoded = decode_dedupe_entry(&bytes).unwrap();
    assert_eq!(decoded, entry);
}

#[test]
fn verify_decode_dedupe_entry_roundtrip_with_special_chars() {
    let entry = DedupeEntry::new(
        "key:with:colons/and\\slashes".to_string(),
        "iid:with:special/chars".to_string(),
        42,
    )
    .unwrap();
    let bytes = encode_dedupe_entry(&entry).unwrap();
    let decoded = decode_dedupe_entry(&bytes).unwrap();
    assert_eq!(decoded, entry);
}

#[test]
fn verify_decode_dedupe_entry_rejects_zero_dedupe_key() {
    let dk_len = u16::to_be_bytes(0);
    let iid_len = u16::to_be_bytes(3);
    let iid_bytes = b"abc".to_vec();
    let expires = 1000u64.to_be_bytes();

    let mut bytes = Vec::new();
    bytes.extend_from_slice(&dk_len);
    bytes.extend_from_slice(&iid_len);
    bytes.extend_from_slice(&iid_bytes);
    bytes.extend_from_slice(&expires);

    let result = decode_dedupe_entry(&bytes);
    assert!(result.is_err());
}

#[test]
fn verify_decode_dedupe_entry_rejects_zero_instance_id() {
    let dk = "valid-dk";
    let dk_len = u16::to_be_bytes(dk.len() as u16);
    let iid_len = u16::to_be_bytes(0);
    let expires = 1000u64.to_be_bytes();

    let mut bytes = Vec::new();
    bytes.extend_from_slice(&dk_len);
    bytes.extend_from_slice(dk.as_bytes());
    bytes.extend_from_slice(&iid_len);
    bytes.extend_from_slice(&expires);

    let result = decode_dedupe_entry(&bytes);
    assert!(result.is_err());
}

// ========================================================================
// DedupeEntry::new — boundary tests
// ========================================================================

#[test]
fn verify_dedupe_entry_rejects_empty_instance_id() {
    let result = DedupeEntry::new("valid-key".to_string(), String::new(), 1000);
    assert_eq!(result, Err(DedupeStoreError::InvalidArgument));
}

#[test]
fn verify_dedupe_entry_rejects_both_empty() {
    assert_eq!(
        DedupeEntry::new(String::new(), String::new(), 1000),
        Err(DedupeStoreError::InvalidArgument)
    );
}

#[test]
fn verify_dedupe_entry_accepts_max_length_fields() {
    let max_key = "x".repeat(10000);
    let max_iid = "y".repeat(10000);
    let result = DedupeEntry::new(max_key, max_iid, u64::MAX);
    assert!(result.is_ok());
}

// ========================================================================
// DedupeStoreError — error string invariants
// ========================================================================

#[test]
fn verify_dedupe_store_error_display_is_non_empty() {
    let storage = DedupeStoreError::Storage {
        reason: "disk full".to_string(),
    };
    assert!(!storage.to_string().is_empty());

    let codec = DedupeStoreError::Codec {
        reason: "invalid encoding".to_string(),
    };
    assert!(!codec.to_string().is_empty());

    let invalid = DedupeStoreError::InvalidArgument;
    assert!(!invalid.to_string().is_empty());
}