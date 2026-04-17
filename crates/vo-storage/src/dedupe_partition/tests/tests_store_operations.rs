#![allow(clippy::unwrap_used)]
//! Integration tests for check_and_insert and contains operations.

use super::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([2u8; 16])
}

// ========================================================================
// DeterministicDedupeStore Test Harness
// ========================================================================

struct DeterministicDedupeStore {
    entries: RefCell<HashMap<String, DedupeEntry>>,
    now_ms: Cell<u64>,
    failure_mode: Option<FailureMode>,
}

enum FailureMode {
    CheckAndInsert { reason: String },
    PurgeExpired { reason: String },
    Contains { reason: String },
}

impl DeterministicDedupeStore {
    fn new() -> Self {
        Self {
            entries: RefCell::new(HashMap::new()),
            now_ms: Cell::new(0),
            failure_mode: None,
        }
    }

    fn set_time(&self, now_ms: u64) {
        self.now_ms.set(now_ms);
    }

    fn failing_check_and_insert(reason: &str) -> Self {
        Self {
            entries: RefCell::new(HashMap::new()),
            now_ms: Cell::new(0),
            failure_mode: Some(FailureMode::CheckAndInsert {
                reason: reason.to_string(),
            }),
        }
    }

    fn failing_contains(reason: &str) -> Self {
        Self {
            entries: RefCell::new(HashMap::new()),
            now_ms: Cell::new(0),
            failure_mode: Some(FailureMode::Contains {
                reason: reason.to_string(),
            }),
        }
    }
}

impl DedupeStore for DeterministicDedupeStore {
    fn check_and_insert(
        &self,
        key: &DedupeKey,
        instance_id: &InstanceId,
        ttl_ms: u64,
    ) -> Result<AdmissionResult, DedupeStoreError> {
        if let Some(FailureMode::CheckAndInsert { reason }) = &self.failure_mode {
            return Err(DedupeStoreError::Storage {
                reason: reason.clone(),
            });
        }
        if ttl_ms == 0 {
            return Err(DedupeStoreError::InvalidArgument);
        }
        let key_str = key.as_str().to_string();
        let now = self.now_ms.get();
        let admission = {
            let entries = self.entries.borrow();
            entries.get(key_str.as_str()).and_then(|existing| {
                if existing.is_expired(now) {
                    None
                } else {
                    Some(AdmissionResult::Duplicate {
                        instance_id: existing.instance_id().to_string(),
                    })
                }
            })
        };
        if let Some(a) = admission {
            return Ok(a);
        }
        let entry = DedupeEntry::new(key_str.clone(), format!("{instance_id}"), ttl_ms)?;
        self.entries.borrow_mut().insert(key_str, entry);
        Ok(AdmissionResult::Admitted)
    }

    fn purge_expired(&self, now_ms: u64) -> Result<u64, DedupeStoreError> {
        // Stubbed - actual tests are in tests_purge.rs
        let mut entries = self.entries.borrow_mut();
        let before = entries.len();
        entries.retain(|_, v| !v.is_expired(now_ms));
        Ok((before - entries.len()) as u64)
    }

    fn contains(&self, key: &DedupeKey) -> Result<bool, DedupeStoreError> {
        if let Some(FailureMode::Contains { reason }) = &self.failure_mode {
            return Err(DedupeStoreError::Storage {
                reason: reason.clone(),
            });
        }
        let entries = self.entries.borrow();
        let key_str = key.as_str();
        Ok(entries
            .get(key_str)
            .is_some_and(|entry| !entry.is_expired(self.now_ms.get())))
    }
}

// ========================================================================
// check_and_insert Tests
// ========================================================================

#[test]
fn check_and_insert_returns_admitted_for_new_key() {
    let store = DeterministicDedupeStore::new();
    let key = DedupeKey::parse("new-key").unwrap();
    let result = store.check_and_insert(&key, &sample_instance_id(), 5000);
    assert_eq!(result, Ok(AdmissionResult::Admitted));
}

#[test]
fn check_and_insert_returns_duplicate_for_existing_key() {
    let store = DeterministicDedupeStore::new();
    let key = DedupeKey::parse("dup-key").unwrap();
    store
        .check_and_insert(&key, &sample_instance_id(), 5000)
        .unwrap();
    let result = store.check_and_insert(&key, &sample_instance_id(), 5000);
    assert_eq!(
        result,
        Ok(AdmissionResult::Duplicate {
            instance_id: sample_instance_id().to_string(),
        })
    );
}

#[test]
fn check_and_insert_returns_error_for_zero_ttl() {
    let store = DeterministicDedupeStore::new();
    let key = DedupeKey::parse("ttl-key").unwrap();
    let result = store.check_and_insert(&key, &sample_instance_id(), 0);
    assert_eq!(result, Err(DedupeStoreError::InvalidArgument));
}

#[test]
fn check_and_insert_returns_exact_storage_error_when_store_backend_fails() {
    let store = DeterministicDedupeStore::failing_check_and_insert("backend write failed");
    let key = DedupeKey::parse("storage-key").unwrap();
    let result = store.check_and_insert(&key, &sample_instance_id(), 5000);
    assert_eq!(
        result,
        Err(DedupeStoreError::Storage {
            reason: "backend write failed".to_string(),
        })
    );
}

#[test]
fn check_and_insert_returns_admitted_when_existing_entry_is_expired() {
    let store = DeterministicDedupeStore::new();
    let key = DedupeKey::parse("expired-key").unwrap();
    let expired_entry =
        DedupeEntry::new("expired-key".to_string(), "stale-instance".to_string(), 0).unwrap();
    store
        .entries
        .borrow_mut()
        .insert("expired-key".to_string(), expired_entry);

    let result = store.check_and_insert(&key, &sample_instance_id(), 5000);

    assert_eq!(result, Ok(AdmissionResult::Admitted));
}

#[test]
fn check_and_insert_replaces_expired_entry_with_new_instance_id() {
    let store = DeterministicDedupeStore::new();
    let key = DedupeKey::parse("reinsert-key").unwrap();
    let expired_entry =
        DedupeEntry::new("reinsert-key".to_string(), "stale-instance".to_string(), 0).unwrap();
    store
        .entries
        .borrow_mut()
        .insert("reinsert-key".to_string(), expired_entry);

    let result = store.check_and_insert(&key, &sample_instance_id(), 5000);
    let stored_instance_id = store
        .entries
        .borrow()
        .get("reinsert-key")
        .unwrap()
        .instance_id()
        .to_string();

    assert_eq!(result, Ok(AdmissionResult::Admitted));
    assert_eq!(stored_instance_id, sample_instance_id().to_string());
}

// ========================================================================
// contains Tests
// ========================================================================

#[test]
fn contains_returns_true_for_existing_unexpired_key_in_real_store() {
    let store = DeterministicDedupeStore::new();
    let key = DedupeKey::parse("contains-key").unwrap();
    store
        .check_and_insert(&key, &sample_instance_id(), 99999)
        .unwrap();

    assert_eq!(store.contains(&key), Ok(true));
}

#[test]
fn contains_returns_false_for_missing_key_in_real_store() {
    let store = DeterministicDedupeStore::new();
    let key = DedupeKey::parse("missing-key").unwrap();

    assert_eq!(store.contains(&key), Ok(false));
}

#[test]
fn contains_returns_false_for_expired_key_in_real_store() {
    let store = DeterministicDedupeStore::new();
    let key = DedupeKey::parse("expired-contains-key").unwrap();
    store
        .check_and_insert(&key, &sample_instance_id(), 100)
        .unwrap();
    store.set_time(100);

    assert_eq!(store.contains(&key), Ok(false));
}

#[test]
fn contains_returns_exact_storage_error_when_store_backend_fails() {
    let store = DeterministicDedupeStore::failing_contains("backend lookup failed");
    let key = DedupeKey::parse("contains-key").unwrap();
    let result = store.contains(&key);
    assert_eq!(
        result,
        Err(DedupeStoreError::Storage {
            reason: "backend lookup failed".to_string(),
        })
    );
}

// ========================================================================
// Red Queen: adversarial expiry boundary — expired entry allows reinsert
// ========================================================================

#[test]
fn rq_expired_entry_allows_reinsert_preserves_new_instance_id() {
    let store = DeterministicDedupeStore::new();
    let key = DedupeKey::parse("rq-reinsert-boundary-b2uv").unwrap();
    let old_iid = InstanceId::from_bytes([0xDE; 16]);
    let new_iid = InstanceId::from_bytes([0xAD; 16]);

    // Insert entry that expires at time 0
    let expired = DedupeEntry::new(
        "rq-reinsert-boundary-b2uv".to_string(),
        format!("{old_iid}"),
        0,
    )
    .unwrap();
    store
        .entries
        .borrow_mut()
        .insert("rq-reinsert-boundary-b2uv".to_string(), expired);

    // At time 0, the entry is expired — reinsert must succeed
    let result = store.check_and_insert(&key, &new_iid, 5000).unwrap();
    assert_eq!(result, AdmissionResult::Admitted);

    // Verify new instance_id replaced the old one
    let entries = store.entries.borrow();
    let stored = entries.get("rq-reinsert-boundary-b2uv").unwrap();
    assert_eq!(stored.instance_id(), &new_iid.to_string());
    assert_ne!(stored.instance_id(), &old_iid.to_string());
}

// ========================================================================
// Red Queen: purge idempotency — repeated purge returns zero after first
// ========================================================================

#[test]
fn rq_purge_repeated_is_idempotent() {
    let store = DeterministicDedupeStore::new();

    store
        .check_and_insert(
            &DedupeKey::parse("rq-idem-a-b2uv").unwrap(),
            &sample_instance_id(),
            100,
        )
        .unwrap();
    store
        .check_and_insert(
            &DedupeKey::parse("rq-idem-b-b2uv").unwrap(),
            &sample_instance_id(),
            200,
        )
        .unwrap();

    let p1 = store.purge_expired(500).unwrap();
    let p2 = store.purge_expired(500).unwrap();
    let p3 = store.purge_expired(500).unwrap();

    assert_eq!(p1, 2, "First purge must remove both expired entries");
    assert_eq!(p2, 0, "Second purge must find nothing");
    assert_eq!(p3, 0, "Third purge must find nothing");
}

// ========================================================================
// Red Queen: purge at exact expiry boundary
// ========================================================================

#[test]
fn rq_purge_at_exact_expiry_boundary() {
    let store = DeterministicDedupeStore::new();
    store
        .check_and_insert(
            &DedupeKey::parse("rq-boundary-b2uv").unwrap(),
            &sample_instance_id(),
            1000,
        )
        .unwrap();

    assert_eq!(
        store.purge_expired(999).unwrap(),
        0,
        "Not yet expired at 999"
    );
    assert_eq!(
        store.purge_expired(1000).unwrap(),
        1,
        "Expired at exact boundary 1000"
    );
}

// ========================================================================
// Red Queen: partial expiry — mixed expired/unexpired entries
// ========================================================================

#[test]
fn rq_partial_expiry_preserves_unexpired_entries() {
    let store = DeterministicDedupeStore::new();
    store
        .check_and_insert(
            &DedupeKey::parse("rq-partial-a-b2uv").unwrap(),
            &sample_instance_id(),
            100,
        )
        .unwrap();
    store
        .check_and_insert(
            &DedupeKey::parse("rq-partial-b-b2uv").unwrap(),
            &sample_instance_id(),
            200,
        )
        .unwrap();
    store
        .check_and_insert(
            &DedupeKey::parse("rq-partial-c-b2uv").unwrap(),
            &sample_instance_id(),
            300,
        )
        .unwrap();
    store
        .check_and_insert(
            &DedupeKey::parse("rq-partial-d-b2uv").unwrap(),
            &sample_instance_id(),
            400,
        )
        .unwrap();

    let purged = store.purge_expired(250).unwrap();
    assert_eq!(purged, 2, "Only keys a and b should be purged");

    assert_eq!(
        store
            .contains(&DedupeKey::parse("rq-partial-a-b2uv").unwrap())
            .unwrap(),
        false
    );
    assert_eq!(
        store
            .contains(&DedupeKey::parse("rq-partial-b-b2uv").unwrap())
            .unwrap(),
        false
    );
    assert_eq!(
        store
            .contains(&DedupeKey::parse("rq-partial-c-b2uv").unwrap())
            .unwrap(),
        true
    );
    assert_eq!(
        store
            .contains(&DedupeKey::parse("rq-partial-d-b2uv").unwrap())
            .unwrap(),
        true
    );
}

// ========================================================================
// Red Queen: purge then reinsert — full eviction cycle
// ========================================================================

#[test]
fn rq_purge_then_reinsert_same_key() {
    let store = DeterministicDedupeStore::new();
    let key = DedupeKey::parse("rq-evict-b2uv").unwrap();
    let new_iid = InstanceId::from_bytes([0xBB; 16]);

    store
        .check_and_insert(&key, &sample_instance_id(), 100)
        .unwrap();
    store.set_time(200);
    assert_eq!(store.purge_expired(200).unwrap(), 1);
    assert_eq!(store.contains(&key).unwrap(), false);

    let result = store.check_and_insert(&key, &new_iid, 5000).unwrap();
    assert_eq!(result, AdmissionResult::Admitted);
    assert_eq!(store.contains(&key).unwrap(), true);
}

// ========================================================================
// Red Queen: purge leaves unexpired entries rejecting duplicates
// ========================================================================

#[test]
fn rq_purge_preserves_duplicate_rejection_for_survivors() {
    let store = DeterministicDedupeStore::new();
    let survivor = DedupeKey::parse("rq-survivor-b2uv").unwrap();
    let doomed = DedupeKey::parse("rq-doomed-b2uv").unwrap();

    store
        .check_and_insert(&survivor, &sample_instance_id(), 9999)
        .unwrap();
    store
        .check_and_insert(&doomed, &sample_instance_id(), 100)
        .unwrap();

    let purged = store.purge_expired(500).unwrap();
    assert_eq!(purged, 1);
    assert_eq!(store.contains(&survivor).unwrap(), true);

    let result = store
        .check_and_insert(&survivor, &sample_instance_id(), 5000)
        .unwrap();
    assert!(matches!(result, AdmissionResult::Duplicate { .. }));
}

// ========================================================================
// Red Queen: u64::MAX expiry — immortal entry survives purge
// ========================================================================

#[test]
fn rq_immortal_entry_survives_purge_at_max_minus_one() {
    let store = DeterministicDedupeStore::new();
    let key = DedupeKey::parse("rq-immortal-b2uv").unwrap();
    store
        .check_and_insert(&key, &sample_instance_id(), u64::MAX)
        .unwrap();

    let purged = store.purge_expired(u64::MAX - 1).unwrap();
    assert_eq!(purged, 0);
    assert_eq!(store.contains(&key).unwrap(), true);
}

// ========================================================================
// Red Queen: interleaved insert-purge-insert cycle
// ========================================================================

#[test]
fn rq_interleaved_insert_purge_insert() {
    let store = DeterministicDedupeStore::new();
    let key1 = DedupeKey::parse("rq-iter-1-b2uv").unwrap();
    let key2 = DedupeKey::parse("rq-iter-2-b2uv").unwrap();

    store
        .check_and_insert(&key1, &sample_instance_id(), 100)
        .unwrap();
    store
        .check_and_insert(&key2, &sample_instance_id(), 9999)
        .unwrap();

    store.set_time(100);
    assert_eq!(store.purge_expired(100).unwrap(), 1);

    let new_iid = InstanceId::from_bytes([0xCC; 16]);
    assert_eq!(
        store.check_and_insert(&key1, &new_iid, 5000).unwrap(),
        AdmissionResult::Admitted
    );
    assert!(matches!(
        store
            .check_and_insert(&key2, &sample_instance_id(), 5000)
            .unwrap(),
        AdmissionResult::Duplicate { .. }
    ));
}

// ========================================================================
// Red Queen: zero TTL rejected — nothing stored
// ========================================================================

#[test]
fn rq_zero_ttl_rejected_and_nothing_stored() {
    let store = DeterministicDedupeStore::new();
    let key = DedupeKey::parse("rq-zero-ttl-b2uv").unwrap();

    assert_eq!(
        store.check_and_insert(&key, &sample_instance_id(), 0),
        Err(DedupeStoreError::InvalidArgument)
    );
    assert_eq!(store.contains(&key).unwrap(), false);
}

// ========================================================================
// Red Queen: key with null bytes — duplicate detection still works
// ========================================================================

#[test]
fn rq_key_with_null_bytes_duplicate_detected() {
    let store = DeterministicDedupeStore::new();
    let key = DedupeKey::parse("key\x00with\x00nulls").unwrap();

    assert_eq!(
        store
            .check_and_insert(&key, &sample_instance_id(), 5000)
            .unwrap(),
        AdmissionResult::Admitted
    );
    assert!(matches!(
        store
            .check_and_insert(&key, &sample_instance_id(), 5000)
            .unwrap(),
        AdmissionResult::Duplicate { .. }
    ));
}

// ========================================================================
// Red Queen: 256-char boundary key admitted
// ========================================================================

#[test]
fn rq_key_256_chars_admitted() {
    let store = DeterministicDedupeStore::new();
    let key256 = "a".repeat(256);
    let key = DedupeKey::parse(&key256).unwrap();
    assert_eq!(
        store
            .check_and_insert(&key, &sample_instance_id(), 5000)
            .unwrap(),
        AdmissionResult::Admitted
    );
}

// ========================================================================
// Red Queen: single-char key admitted
// ========================================================================

#[test]
fn rq_single_char_key_admitted() {
    let store = DeterministicDedupeStore::new();
    let key = DedupeKey::parse("x").unwrap();
    assert_eq!(
        store
            .check_and_insert(&key, &sample_instance_id(), 5000)
            .unwrap(),
        AdmissionResult::Admitted
    );
}

// ========================================================================
// Red Queen: expiry monotonicity — all timestamps
// ========================================================================

#[test]
fn rq_expiry_monotonic_across_boundary() {
    let entry = DedupeEntry::new("k".to_string(), "i".to_string(), 1000).unwrap();

    for now in [0u64, 1, 500, 999] {
        assert!(!entry.is_expired(now), "Must not be expired at {now}");
    }
    for now in [1000u64, 1001, 5000, u64::MAX] {
        assert!(entry.is_expired(now), "Must be expired at {now}");
    }
}

// ========================================================================
// Red Queen: zero expiry — expired at all timestamps
// ========================================================================

#[test]
fn rq_zero_expiry_expired_at_all_timestamps() {
    let entry = DedupeEntry::new("k".to_string(), "i".to_string(), 0).unwrap();
    assert!(entry.is_expired(0));
    assert!(entry.is_expired(1));
    assert!(entry.is_expired(u64::MAX));
}

// ========================================================================
// Red Queen: AdmissionResult cross-variant inequality
// ========================================================================

#[test]
fn rq_admission_result_cross_variant_inequality() {
    let admitted = AdmissionResult::Admitted;
    let dup = AdmissionResult::Duplicate {
        instance_id: "x".to_string(),
    };
    assert_ne!(admitted, dup);
}

#[test]
fn rq_admission_result_duplicate_different_iids_not_equal() {
    let d1 = AdmissionResult::Duplicate {
        instance_id: "i1".to_string(),
    };
    let d2 = AdmissionResult::Duplicate {
        instance_id: "i2".to_string(),
    };
    assert_ne!(d1, d2);
}

// ========================================================================
// Red Queen: DedupeEntry equality — all fields matter
// ========================================================================

#[test]
fn rq_entry_equality_all_fields_matter() {
    let e1 = DedupeEntry::new("k".to_string(), "i".to_string(), 100).unwrap();
    let e2 = DedupeEntry::new("k".to_string(), "i".to_string(), 100).unwrap();
    let e3 = DedupeEntry::new("k-diff".to_string(), "i".to_string(), 100).unwrap();
    let e4 = DedupeEntry::new("k".to_string(), "i-diff".to_string(), 100).unwrap();
    let e5 = DedupeEntry::new("k".to_string(), "i".to_string(), 200).unwrap();

    assert_eq!(e1, e2);
    assert_ne!(e1, e3);
    assert_ne!(e1, e4);
    assert_ne!(e1, e5);
}

// ========================================================================
// Red Queen: error cross-variant inequality
// ========================================================================

#[test]
fn rq_error_cross_variant_inequality() {
    let storage = DedupeStoreError::Storage {
        reason: "x".to_string(),
    };
    let codec = DedupeStoreError::Codec {
        reason: "x".to_string(),
    };
    let invalid = DedupeStoreError::InvalidArgument;
    assert_ne!(storage, codec);
    assert_ne!(storage, invalid);
    assert_ne!(codec, invalid);
}

// ========================================================================
// Red Queen: error implements std::error::Error
// ========================================================================

#[test]
fn rq_error_implements_std_error_trait() {
    let err: Box<dyn std::error::Error> =
        Box::new(DedupeStoreError::Storage { reason: "e".into() });
    assert!(!err.to_string().is_empty());
}

// ========================================================================
// Red Queen: encode_dedupe_entry produces valid binary structure
// ========================================================================

#[test]
fn rq_encode_entry_produces_valid_binary() {
    let entry = DedupeEntry::new("key-b2uv".to_string(), "iid-b2uv".to_string(), 42).unwrap();
    let bytes = encode_dedupe_entry(&entry).unwrap();

    let dk_len = u16::from_be_bytes([bytes[0], bytes[1]]) as usize;
    assert_eq!(&bytes[2..2 + dk_len], b"key-b2uv");
    let offset = 2 + dk_len;
    let iid_len = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]) as usize;
    assert_eq!(&bytes[offset + 2..offset + 2 + iid_len], b"iid-b2uv");
    let ts_offset = offset + 2 + iid_len;
    let expires_at = u64::from_be_bytes(bytes[ts_offset..ts_offset + 8].try_into().unwrap());
    assert_eq!(expires_at, 42);

    let recovered = decode_dedupe_entry(&bytes).unwrap();
    assert_eq!(recovered, entry);
}

// ========================================================================
// Red Queen: decode_entry rejects binary garbage
// ========================================================================

#[test]
fn rq_decode_entry_binary_garbage_rejected() {
    let garbage = vec![0xFF, 0xFE, 0xFD, 0xFC, 0xFB, 0xFA];
    assert!(decode_dedupe_entry(&garbage).is_err());
}

// ========================================================================
// Red Queen: decode_entry rejects wrong type for expires_at
// ========================================================================

#[test]
fn rq_decode_entry_wrong_type_rejected() {
    let json = r#"{"dedupe_key":"k","instance_id":"i","expires_at":"not-a-number"}"#;
    assert!(decode_dedupe_entry(json.as_bytes()).is_err());
}

// ========================================================================
// Red Queen: decode_key with BOM prefix
// ========================================================================

#[test]
fn rq_decode_key_bom_prefix_accepted() {
    let mut with_bom = Vec::new();
    with_bom.extend_from_slice(&[0xEF, 0xBB, 0xBF]); // UTF-8 BOM
    with_bom.extend_from_slice(b"test-key");
    assert!(decode_dedupe_key(&with_bom).is_ok());
}

// ========================================================================
// Red Queen: max Unicode codepoint roundtrip
// ========================================================================

#[test]
fn rq_decode_key_max_unicode_codepoint() {
    let max_char = "\u{10FFFF}";
    let key = DedupeKey::parse(max_char).unwrap();
    let bytes = encode_dedupe_key(&key);
    let recovered = decode_dedupe_key(&bytes).unwrap();
    assert_eq!(recovered.as_str(), max_char);
}

// ========================================================================
// Red Queen: constructor rejects both empty fields
// ========================================================================

#[test]
fn rq_entry_both_empty_rejected() {
    assert_eq!(
        DedupeEntry::new(String::new(), String::new(), 1000),
        Err(DedupeStoreError::InvalidArgument)
    );
}

// ========================================================================
// Red Queen: entry with zero expiry allowed (immediately expired)
// ========================================================================

#[test]
fn rq_entry_zero_expiry_allowed_but_immediately_expired() {
    let entry = DedupeEntry::new("k".to_string(), "i".to_string(), 0).unwrap();
    assert_eq!(entry.expires_at(), 0);
    assert!(entry.is_expired(0));
    assert!(entry.is_expired(u64::MAX));
}
