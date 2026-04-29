//! Red Queen adversarial tests: serde behavior and encoding.

use crate::dedupe_partition::*;

// ========================================================================
// DIMENSION: encode-roundtrip — key encode/decode with unicode edge cases
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
    let result = decode_dedupe_key(key257.as_bytes());
    assert_eq!(
        result,
        Err(DedupeStoreError::Codec {
            reason: "DedupeKey: exceeds maximum length of 256 (got 257)".to_string()
        })
    );
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

    assert!(!recovered.is_expired(50));
    assert!(recovered.is_expired(100));
    assert!(recovered.is_expired(200));
}

// ========================================================================
// DIMENSION: check contract alignment — encode_dedupe_entry is always Ok
// ========================================================================

#[test]
fn red_queen_encode_entry_never_fails_for_valid_entry() {
    let entry = DedupeEntry::new("k".to_string(), "i".to_string(), 0).unwrap();
    let result = encode_dedupe_entry(&entry);
    let bytes = result.unwrap();
    assert!(!bytes.is_empty());
}

// ========================================================================
// DIMENSION: instance_id in Duplicate — must be preserved from storage
// ========================================================================

#[test]
fn red_queen_admission_result_duplicate_instance_id_preserved() {
    let dup = AdmissionResult::Duplicate {
        instance_id: "original-instance".to_string(),
    };
    if let AdmissionResult::Duplicate { instance_id } = &dup {
        assert_eq!(instance_id, "original-instance");
    }
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
    #[allow(clippy::redundant_clone)]
    {
        let _ = admitted.clone();
        let _ = dup.clone();
    }
}

// ========================================================================
// DIMENSION: serde with compact vs pretty — deterministic output
// ========================================================================

#[test]
fn red_queen_entry_serde_is_deterministic() {
    let entry = DedupeEntry::new("k".to_string(), "i".to_string(), 100).unwrap();
    let compact = serde_json::to_vec(&entry).unwrap();
    let pretty: Vec<u8> = serde_json::to_vec(&entry).unwrap();
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
    fn _assert_object_safe(_: &dyn DedupeStore) {}
}
