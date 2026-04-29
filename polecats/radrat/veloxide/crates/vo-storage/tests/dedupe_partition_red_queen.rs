//! Red Queen adversarial tests for dedupe_partition contract (vel-7ffu)
//!
//! Attempts to violate contracts and find edge-case bugs missed by the existing test suite.

use vo_storage::dedupe_partition::*;
use vo_types::DedupeKey;

// ========================================================================
// DIMENSION: constant-value — DEDUPE_PARTITION must be exactly "dedupe"
// ========================================================================

#[test]
fn red_queen_constant_is_exactly_dedupe() {
    assert_eq!(DEDUPE_PARTITION, "dedupe");
}

#[test]
fn red_queen_constant_len_is_6() {
    assert_eq!(DEDUPE_PARTITION.len(), 6);
}

// ========================================================================
// DIMENSION: expiry-semantics — boundary at expires_at = 0, now_ms = 0
// ========================================================================

#[test]
fn red_queen_expired_at_zero_boundary() {
    let entry = DedupeEntry::new("key".to_string(), "iid".to_string(), 0).unwrap();
    assert!(entry.is_expired(0), "0 >= 0 → should be expired");
}

#[test]
fn red_queen_expired_at_zero_boundary_with_nonzero_now() {
    let entry = DedupeEntry::new("key".to_string(), "iid".to_string(), 0).unwrap();
    assert!(entry.is_expired(1), "1 >= 0 → should be expired");
}

#[test]
fn red_queen_not_expired_at_u64_max_minus_one() {
    let entry = DedupeEntry::new("key".to_string(), "iid".to_string(), u64::MAX).unwrap();
    assert!(
        !entry.is_expired(u64::MAX - 1),
        "u64::MAX-1 < u64::MAX → NOT expired"
    );
}

// ========================================================================
// DIMENSION: codec-error — decode edge cases
// ========================================================================

#[test]
fn red_queen_decode_key_rejects_empty_bytes() {
    let result = decode_dedupe_key(b"");
    assert!(result.is_err());
}

#[test]
fn red_queen_decode_key_accepts_single_null_byte() {
    // Null byte IS valid UTF-8 and valid DedupeKey (non-empty, ≤256 chars)
    let result = decode_dedupe_key(&[0x00]);
    assert!(
        result.is_ok(),
        "null byte is valid UTF-8 and valid DedupeKey"
    );
}

#[test]
fn red_queen_decode_key_rejects_invalid_utf8() {
    let result = decode_dedupe_key(&[0xFF, 0xFE]);
    assert!(result.is_err());
}

#[test]
fn red_queen_decode_key_rejects_unicode_surrogate() {
    let result = decode_dedupe_key(&[0xED, 0xA0, 0x80]);
    assert!(result.is_err());
}

#[test]
fn red_queen_decode_entry_rejects_empty_bytes() {
    let result = decode_dedupe_entry(b"");
    assert!(result.is_err());
}

#[test]
fn red_queen_decode_entry_rejects_truncated_json() {
    let result = decode_dedupe_entry(b"{\"dedupe_key\":");
    assert!(result.is_err());
}

#[test]
fn red_queen_decode_entry_rejects_missing_required_field() {
    let json = r#"{"dedupe_key":"k","expires_at":100}"#;
    let result = decode_dedupe_entry(json.as_bytes());
    assert!(result.is_err());
}

// ========================================================================
// DIMENSION: key encode/decode roundtrip — unicode edge cases
// ========================================================================

#[test]
fn red_queen_key_encode_decode_unicode_emoji() {
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
    let key256 = "a".repeat(256);
    let key = DedupeKey::parse(&key256).unwrap();
    let bytes = encode_dedupe_key(&key);
    assert_eq!(bytes.len(), 256);
    let recovered = decode_dedupe_key(&bytes).unwrap();
    assert_eq!(recovered.as_str(), key256);
}

#[test]
fn red_queen_key_encode_decode_256_plus_1_rejected() {
    let key257 = "a".repeat(257);
    let result = DedupeKey::parse(&key257);
    assert!(result.is_err());
}

// ========================================================================
// DIMENSION: serde — deterministic and preserves behavior
// ========================================================================

#[test]
fn red_queen_entry_serde_preserves_expiry_behavior() {
    let entry = DedupeEntry::new("k".to_string(), "i".to_string(), 100).unwrap();
    let json = serde_json::to_vec(&entry).unwrap();
    let recovered: DedupeEntry = serde_json::from_slice(&json).unwrap();
    assert!(!recovered.is_expired(50));
    assert!(recovered.is_expired(100));
    assert!(recovered.is_expired(200));
}

#[test]
fn red_queen_entry_serde_is_deterministic() {
    let entry = DedupeEntry::new("k".to_string(), "i".to_string(), 100).unwrap();
    let compact = serde_json::to_vec(&entry).unwrap();
    let pretty = serde_json::to_string(&entry).unwrap();
    // Same serializer for same types → deterministic
    assert_eq!(compact, pretty.as_bytes());
}

#[test]
fn red_queen_key_encode_is_deterministic() {
    let key = DedupeKey::parse("test-key").unwrap();
    let bytes1 = encode_dedupe_key(&key);
    let bytes2 = encode_dedupe_key(&key);
    assert_eq!(bytes1, bytes2);
}

// ========================================================================
// DIMENSION: error display — all variants have correct format
// ========================================================================

#[test]
fn red_queen_error_display_all_variants() {
    let storage_err = DedupeStoreError::Storage {
        reason: "disk full".to_string(),
    };
    let codec_err = DedupeStoreError::Codec {
        reason: "bad json".to_string(),
    };
    let invalid_err = DedupeStoreError::InvalidArgument;

    assert_eq!(storage_err.to_string(), "dedupe storage error: disk full");
    assert_eq!(codec_err.to_string(), "dedupe codec error: bad json");
    assert_eq!(invalid_err.to_string(), "invalid dedupe argument");
}

#[test]
fn red_queen_error_debug_all_variants() {
    let storage_err = DedupeStoreError::Storage {
        reason: "disk full".to_string(),
    };
    let codec_err = DedupeStoreError::Codec {
        reason: "bad json".to_string(),
    };
    let invalid_err = DedupeStoreError::InvalidArgument;

    assert!(!format!("{:?}", storage_err).is_empty());
    assert!(!format!("{:?}", codec_err).is_empty());
    assert!(!format!("{:?}", invalid_err).is_empty());
}

// ========================================================================
// DIMENSION: clone — DedupeEntry, AdmissionResult must be cloneable
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
// DIMENSION: trait object safety — DedupeStore can be used as dyn trait
// ========================================================================

#[test]
fn red_queen_dedupe_store_is_object_safe() {
    fn _assert_object_safe(_: &dyn DedupeStore) {}
}

// ========================================================================
// DIMENSION: encode_dedupe_entry is infallible for valid DedupeEntry
// Contract says fallible but current types make it infallible (alignment gap)
// ========================================================================

#[test]
fn red_queen_encode_entry_infallible_for_valid_entry() {
    let entry = DedupeEntry::new("k".to_string(), "i".to_string(), 0).unwrap();
    let result = encode_dedupe_entry(&entry);
    assert!(result.is_ok());
}

// ========================================================================
// DIMENSION: AdmissionResult — Duplicate preserves instance_id
// ========================================================================

#[test]
fn red_queen_admission_result_duplicate_equality() {
    let a = AdmissionResult::Duplicate {
        instance_id: "inst-1".to_string(),
    };
    let b = AdmissionResult::Duplicate {
        instance_id: "inst-1".to_string(),
    };
    let c = AdmissionResult::Duplicate {
        instance_id: "inst-2".to_string(),
    };
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn red_queen_admission_result_admitted_ne_duplicate() {
    let a = AdmissionResult::Admitted;
    let b = AdmissionResult::Duplicate {
        instance_id: "inst-1".to_string(),
    };
    assert_ne!(a, b);
}
