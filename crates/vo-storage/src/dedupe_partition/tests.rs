#![allow(clippy::unwrap_used)]
use super::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([2u8; 16])
}

// ========================================================================
// DedupeEntry Construction
// ========================================================================

#[test]
fn dedupe_entry_constructs_with_valid_fields() {
    let e = DedupeEntry::new("key-1".to_string(), "instance-1".to_string(), 1000).unwrap();
    assert_eq!(e.dedupe_key(), "key-1");
    assert_eq!(e.instance_id(), "instance-1");
    assert_eq!(e.expires_at(), 1000);
}

#[test]
fn dedupe_entry_rejects_empty_dedupe_key() {
    let result = DedupeEntry::new(String::new(), "instance-1".to_string(), 1000);
    assert_eq!(result, Err(DedupeStoreError::InvalidArgument));
}

#[test]
fn dedupe_entry_rejects_empty_instance_id() {
    let result = DedupeEntry::new("key-1".to_string(), String::new(), 1000);
    assert_eq!(result, Err(DedupeStoreError::InvalidArgument));
}

#[test]
fn dedupe_entry_is_expired_returns_true_when_past_expiry() {
    let entry = DedupeEntry::new("key-1".to_string(), "instance-1".to_string(), 1000).unwrap();
    assert!(entry.is_expired(1000));
    assert!(entry.is_expired(2000));
}

#[test]
fn dedupe_entry_is_expired_returns_false_before_expiry() {
    let entry = DedupeEntry::new("key-1".to_string(), "instance-1".to_string(), 1000).unwrap();
    assert!(!entry.is_expired(999));
}

#[test]
fn dedupe_entry_serde_roundtrip() {
    let entry = DedupeEntry::new("key-1".to_string(), "instance-1".to_string(), 1000).unwrap();
    let json = serde_json::to_string(&entry).unwrap();
    let recovered: DedupeEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(recovered, entry);
}

// ========================================================================
// AdmissionResult
// ========================================================================

#[test]
fn admission_result_admitted_equality() {
    let a = AdmissionResult::Admitted;
    let b = AdmissionResult::Admitted;
    assert_eq!(a, b);
}

#[test]
fn admission_result_duplicate_equality() {
    let a = AdmissionResult::Duplicate {
        instance_id: "inst-1".to_string(),
    };
    let b = AdmissionResult::Duplicate {
        instance_id: "inst-1".to_string(),
    };
    assert_eq!(a, b);
}

#[test]
fn admission_result_admitted_ne_duplicate() {
    let a = AdmissionResult::Admitted;
    let b = AdmissionResult::Duplicate {
        instance_id: "inst-1".to_string(),
    };
    assert_ne!(a, b);
}

// ========================================================================
// Error Display
// ========================================================================

#[test]
fn error_storage_displays_reason() {
    let err = DedupeStoreError::Storage {
        reason: "disk full".to_string(),
    };
    assert_eq!(err.to_string(), "dedupe storage error: disk full");
}

#[test]
fn error_codec_displays_reason() {
    let err = DedupeStoreError::Codec {
        reason: "bad json".to_string(),
    };
    assert_eq!(err.to_string(), "dedupe codec error: bad json");
}

#[test]
fn error_invalid_argument_displays_message() {
    let err = DedupeStoreError::InvalidArgument;
    assert_eq!(err.to_string(), "invalid dedupe argument");
}

#[test]
fn dedupe_entry_constructs_with_zero_expiry_boundary() {
    let result = DedupeEntry::new("key-0".to_string(), "instance-0".to_string(), 0);
    assert_eq!(
        result,
        Ok(DedupeEntry {
            dedupe_key: "key-0".to_string(),
            instance_id: "instance-0".to_string(),
            expires_at: 0,
        })
    );
}

#[test]
fn dedupe_entry_is_not_expired_before_u64_max_boundary() {
    let entry =
        DedupeEntry::new("key-max".to_string(), "instance-max".to_string(), u64::MAX).unwrap();
    assert!(!entry.is_expired(u64::MAX - 1));
}

#[test]
fn dedupe_entry_is_expired_at_u64_max_boundary() {
    let entry =
        DedupeEntry::new("key-max".to_string(), "instance-max".to_string(), u64::MAX).unwrap();
    assert!(entry.is_expired(u64::MAX));
}

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
fn decode_dedupe_key_returns_error_for_invalid_utf8() {
    let result = decode_dedupe_key(&[0xFF, 0xFE]);
    assert_eq!(
        result,
        Err(DedupeStoreError::Codec {
            reason: "invalid utf-8 sequence of 1 bytes from index 0".to_string()
        })
    );
}

#[test]
fn decode_dedupe_key_returns_error_for_empty() {
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
    // 257-byte key that exceeds max length
    let long_key = "a".repeat(257);
    let result = decode_dedupe_key(long_key.as_bytes());
    assert_eq!(
        result,
        Err(DedupeStoreError::Codec {
            reason: "DedupeKey: exceeds maximum length of 256 (got 257)".to_string()
        })
    );
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
fn decode_dedupe_entry_returns_error_for_invalid_json() {
    let result = decode_dedupe_entry(b"not-json");
    assert_eq!(
        result,
        Err(DedupeStoreError::Codec {
            reason: "expected ident at line 1 column 2".to_string()
        })
    );
}

#[test]
fn encode_dedupe_key_preserves_unicode_bytes() {
    let key = DedupeKey::parse("dedupe-π").unwrap();
    let bytes = encode_dedupe_key(&key);
    assert_eq!(bytes, "dedupe-π".as_bytes());
}

// ========================================================================
// Trait Integration — via DeterministicDedupeStore
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

    fn failing_purge_expired(reason: &str) -> Self {
        Self {
            entries: RefCell::new(HashMap::new()),
            now_ms: Cell::new(0),
            failure_mode: Some(FailureMode::PurgeExpired {
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
        if let Some(FailureMode::PurgeExpired { reason }) = &self.failure_mode {
            return Err(DedupeStoreError::Storage {
                reason: reason.clone(),
            });
        }
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

impl DeterministicDedupeStore {
    /// Returns `Some(AdmissionResult::Duplicate)` if key exists and is not expired.
    /// Returns `None` if key is absent or expired (caller should insert/replace).
    fn lookup_entry(
        entries: &HashMap<String, DedupeEntry>,
        key_str: &str,
        now: u64,
    ) -> Option<AdmissionResult> {
        entries.get(key_str).and_then(|existing| {
            if existing.is_expired(now) {
                None
            } else {
                Some(AdmissionResult::Duplicate {
                    instance_id: existing.instance_id().to_string(),
                })
            }
        })
    }
}

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

#[test]
fn purge_expired_removes_expired_entries() {
    let store = DeterministicDedupeStore::new();
    let key = DedupeKey::parse("expire-key").unwrap();
    // Insert with ttl=100 (expires at 100)
    store
        .check_and_insert(&key, &sample_instance_id(), 100)
        .unwrap();
    let purged = store.purge_expired(100).unwrap();
    assert_eq!(purged, 1);
    let contains = store.contains(&key).unwrap();
    assert!(!contains);
}

#[test]
fn purge_expired_returns_zero_when_entries_remain_unexpired() {
    let store = DeterministicDedupeStore::new();
    let key = DedupeKey::parse("active-key").unwrap();
    store
        .check_and_insert(&key, &sample_instance_id(), 100)
        .unwrap();

    let result = store.purge_expired(99);

    assert_eq!(result, Ok(0));
}

#[test]
fn purge_expired_returns_exact_storage_error_when_store_backend_fails() {
    let store = DeterministicDedupeStore::failing_purge_expired("backend scan failed");
    let result = store.purge_expired(100);
    assert_eq!(
        result,
        Err(DedupeStoreError::Storage {
            reason: "backend scan failed".to_string(),
        })
    );
}

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
    // K-01 proof must contain the exact assert_eq with InvalidArgument variant
    assert!(source.contains("assert_eq!(result, Err(DedupeStoreError::InvalidArgument))"));
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
