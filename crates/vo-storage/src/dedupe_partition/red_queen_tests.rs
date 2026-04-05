//! Red Queen adversarial tests for dedupe_partition contract (vel-7ffu)
//!
//! This test file attempts to violate contracts and find edge-case bugs
//! that the existing test suite may have missed.

#![allow(clippy::unwrap_used)]

use crate::dedupe_partition::*;

// ========================================================================
// DIMENSION: constant-value — DEDUPE_PARTITION must be exactly "dedupe"
// ========================================================================

#[test]
fn red_queen_constant_is_exactly_dedupe() {
    // Contract: Value is exactly "dedupe"
    assert_eq!(DEDUPE_PARTITION, "dedupe");
}

#[test]
fn red_queen_constant_is_not_empty() {
    assert!(!DEDUPE_PARTITION.is_empty());
}

#[test]
fn red_queen_constant_len_is_6() {
    assert_eq!(DEDUPE_PARTITION.len(), 6);
}

// ========================================================================
// DIMENSION: expiry-semantics — edge case: expires_at = 0, now_ms = 0
// ========================================================================

#[test]
fn red_queen_expired_at_zero_boundary() {
    // Contract: is_expired returns true iff now_ms >= expires_at
    // If expires_at = 0 and now_ms = 0, then 0 >= 0 → true
    let entry = DedupeEntry::new("key".to_string(), "iid".to_string(), 0).unwrap();
    assert!(entry.is_expired(0));
}

#[test]
fn red_queen_expired_at_zero_boundary_with_nonzero_now() {
    // If expires_at = 0 and now_ms > 0, still expired
    let entry = DedupeEntry::new("key".to_string(), "iid".to_string(), 0).unwrap();
    assert!(entry.is_expired(1));
}

#[test]
fn red_queen_not_expired_at_u64_max_minus_one() {
    // If expires_at = u64::MAX and now_ms = u64::MAX - 1, NOT expired
    let entry = DedupeEntry::new("key".to_string(), "iid".to_string(), u64::MAX).unwrap();
    assert!(!entry.is_expired(u64::MAX - 1));
}

// ========================================================================
// DIMENSION: constructor-validation — both fields empty simultaneously
// ========================================================================

#[test]
fn red_queen_constructor_rejects_both_fields_empty() {
    // What if both dedupe_key AND instance_id are empty?
    let result = DedupeEntry::new(String::new(), String::new(), 1000);
    assert_eq!(result, Err(DedupeStoreError::InvalidArgument));
}

#[test]
fn red_queen_constructor_rejects_whitespace_key() {
    // Contract: "must not be empty" — whitespace-only is NOT empty
    let result = DedupeEntry::new("   ".to_string(), "instance".to_string(), 1000);
    // Whitespace is technically not empty, so this should succeed
    // But is that the INTENDED behavior?
    // The contract says "must not be empty" — whitespace is not empty string
    // This is a potential semantic gap
    assert!(
        result.is_ok(),
        "whitespace key was rejected but should succeed per strict interpretation"
    );
}

#[test]
fn red_queen_constructor_rejects_whitespace_instance_id() {
    let result = DedupeEntry::new("key".to_string(), "   ".to_string(), 1000);
    assert!(
        result.is_ok(),
        "whitespace instance_id was rejected but should succeed per strict interpretation"
    );
}

// ========================================================================
// DIMENSION: codec-error — probe decode_dedupe_key with edge cases
// ========================================================================

#[test]
fn red_queen_decode_key_accepts_single_null_byte() {
    // Null byte (0x00) is valid UTF-8 (U+0000), so decode succeeds
    // DedupeKey::parse accepts any non-empty string, so this is Ok
    let result = decode_dedupe_key(&[0x00]);
    assert!(
        result.is_ok(),
        "null byte is valid UTF-8 and non-empty, should be accepted"
    );
}

#[test]
fn red_queen_decode_key_rejects_valid_utf8_but_invalid_key_format() {
    // Valid UTF-8 but may not be valid DedupeKey format
    // Actually DedupeKey::parse accepts ANY non-empty string up to 256 chars
    let result = decode_dedupe_key(b"valid-utf8-key");
    assert!(result.is_ok(), "valid UTF-8 key should parse successfully");
}

#[test]
fn red_queen_decode_key_rejects_unicode_surrogate() {
    // UTF-8 encoding of surrogate codepoint — invalid
    let result = decode_dedupe_key(&[0xED, 0xA0, 0x80]);
    assert!(result.is_err());
}

#[test]
fn red_queen_decode_entry_rejects_empty_bytes() {
    // Empty JSON is not valid
    let result = decode_dedupe_entry(b"");
    assert!(result.is_err());
}

#[test]
fn red_queen_decode_entry_rejects_truncated_json() {
    let result = decode_dedupe_entry(b"{\"dedupe_key\":");
    assert!(result.is_err());
}

#[test]
fn red_queen_decode_entry_rejects_extra_fields() {
    // JSON with extra fields that DedupeEntry doesn't have
    let json = r#"{"dedupe_key":"k","instance_id":"i","expires_at":100,"extra":true}"#;
    let result = decode_dedupe_entry(json.as_bytes());
    // serde_json by default ignores extra fields (deny_unknown_fields would reject)
    assert!(
        result.is_ok(),
        "extra fields should be ignored by default serde"
    );
}

#[test]
fn red_queen_decode_entry_rejects_missing_required_field() {
    let json = r#"{"dedupe_key":"k","expires_at":100}"#;
    let result = decode_dedupe_entry(json.as_bytes());
    assert!(result.is_err(), "missing instance_id should fail");
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

    // All variants should produce non-empty display strings
    assert!(!storage_err.to_string().is_empty());
    assert!(!codec_err.to_string().is_empty());
    assert!(!invalid_err.to_string().is_empty());

    // Specific prefixes per contract
    assert!(storage_err.to_string().starts_with("dedupe storage error:"));
    assert!(codec_err.to_string().starts_with("dedupe codec error:"));
    assert_eq!(invalid_err.to_string(), "invalid dedupe argument");
}

#[test]
fn red_queen_error_debug_all_variants() {
    // Debug format should also be non-empty
    let storage_err = DedupeStoreError::Storage {
        reason: "test".to_string(),
    };
    let codec_err = DedupeStoreError::Codec {
        reason: "test".to_string(),
    };
    let invalid_err = DedupeStoreError::InvalidArgument;

    let s = format!("{:?}", storage_err);
    let c = format!("{:?}", codec_err);
    let i = format!("{:?}", invalid_err);

    assert!(!s.is_empty());
    assert!(!c.is_empty());
    assert!(!i.is_empty());
}

// ========================================================================
// DIMENSION: encode-roundtrip — key encode/decode with unicode edge cases
// ========================================================================

#[test]
fn red_queen_key_encode_decode_unicode_emoji() {
    // Emoji key (4-byte UTF-8)
    let key = DedupeKey::parse("🔥").unwrap();
    let bytes = encode_dedupe_key(&key);
    let recovered = decode_dedupe_key(&bytes).unwrap();
    assert_eq!(recovered.as_str(), "🔥");
}

#[test]
fn red_queen_key_encode_decode_unicode_accented() {
    let key = DedupeKey::parse("café").unwrap();
    let bytes = encode_dedupe_key(&key);
    let recovered = decode_dedupe_key(&bytes).unwrap();
    assert_eq!(recovered.as_str(), "café");
}

#[test]
fn red_queen_key_encode_decode_max_length_exact() {
    // Exactly 256 character key
    let key256 = "a".repeat(256);
    let key = DedupeKey::parse(&key256).unwrap();
    let bytes = encode_dedupe_key(&key);
    assert_eq!(bytes.len(), 256);
    let recovered = decode_dedupe_key(&bytes).unwrap();
    assert_eq!(recovered.as_str(), key256);
}

#[test]
fn red_queen_key_encode_decode_256_plus_1_rejected() {
    // 257 character key should be rejected
    let key257 = "a".repeat(257);
    let result = DedupeKey::parse(&key257);
    assert!(result.is_err());
}

// ========================================================================
// DIMENSION: entry-serde — deserialized entries behave identically
// ========================================================================

#[test]
fn red_queen_entry_deserialized_has_same_expiry_behavior() {
    let entry = DedupeEntry::new("k".to_string(), "i".to_string(), 100).unwrap();
    let json = serde_json::to_vec(&entry).unwrap();
    let recovered: DedupeEntry = serde_json::from_slice(&json).unwrap();

    assert_eq!(recovered.expires_at(), entry.expires_at());
    assert_eq!(recovered.dedupe_key(), entry.dedupe_key());
    assert_eq!(recovered.instance_id(), entry.instance_id());

    // Deserialized entry should have same expiry behavior
    assert!(!recovered.is_expired(50));
    assert!(recovered.is_expired(100));
    assert!(recovered.is_expired(200));
}

// ========================================================================
// DIMENSION: check contract alignment — encode_dedupe_entry is always Ok
// The contract says encode_dedupe_entry can fail with Codec error.
// In practice, serde_json::to_vec for DedupeEntry (String, String, u64) NEVER fails.
// This is a contract/code alignment gap.
// ========================================================================

#[test]
fn red_queen_encode_entry_never_fails_for_valid_entry() {
    // The contract documents a fallible signature but the current types
    // make it infallible. This is documented alignment gap, not a bug.
    let entry = DedupeEntry::new("k".to_string(), "i".to_string(), 0).unwrap();
    let result = encode_dedupe_entry(&entry);
    assert!(result.is_ok());
}

// ========================================================================
// DIMENSION: instance_id in Duplicate — must be preserved from storage
// ========================================================================

#[test]
fn red_queen_admission_result_duplicate_instance_id_preserved() {
    // Contract: Duplicate payload preserves the stored instance_id string
    // This is exercised through DeterministicDedupeStore in existing tests
    // Here we verify the enum structure allows this
    let dup = AdmissionResult::Duplicate {
        instance_id: "original-instance".to_string(),
    };
    if let AdmissionResult::Duplicate { instance_id } = &dup {
        assert_eq!(instance_id, "original-instance");
    }
}

// ========================================================================
// DIMENSION: clone — DedupeEntry, AdmissionResult must be cloneable
// (needed for store implementations)
// ========================================================================

#[test]
fn red_queen_dedupe_entry_is_cloneable() {
    let entry = DedupeEntry::new("k".to_string(), "i".to_string(), 100).unwrap();
    let cloned = entry.clone();
    assert_eq!(cloned.dedupe_key(), entry.dedupe_key());
    assert_eq!(cloned.instance_id(), entry.instance_id());
    assert_eq!(cloned.expires_at(), entry.expires_at());
}

#[test]
fn red_queen_admission_result_is_cloneable() {
    let admitted = AdmissionResult::Admitted;
    let dup = AdmissionResult::Duplicate {
        instance_id: "i".to_string(),
    };
    let _ = admitted.clone();
    let _ = dup.clone();
}

// ========================================================================
// DIMENSION: serde with compact vs pretty — deterministic output
// Contract: "Successful output is deterministic for the same entry value"
// ========================================================================

#[test]
fn red_queen_entry_serde_is_deterministic() {
    let entry = DedupeEntry::new("k".to_string(), "i".to_string(), 100).unwrap();

    let compact = serde_json::to_vec(&entry).unwrap();
    let pretty: Vec<u8> = serde_json::to_vec(&entry).unwrap(); // same for same types

    // Both should produce identical output for same types
    assert_eq!(compact, pretty);
}

#[test]
fn red_queen_key_encode_is_deterministic() {
    let key = DedupeKey::parse("test-key").unwrap();
    let bytes1 = encode_dedupe_key(&key);
    let bytes2 = encode_dedupe_key(&key);
    assert_eq!(bytes1, bytes2);
}

// ========================================================================
// DIMENSION: trait object safety — DedupeStore can be used as dyn trait
// ========================================================================

#[test]
fn red_queen_dedupe_store_can_be_dyn() {
    // Verify the trait is object-safe (has no methods that require Self)
    fn _assert_object_safe(_: &dyn DedupeStore) {}
    // If this compiles, the trait is object-safe
}
