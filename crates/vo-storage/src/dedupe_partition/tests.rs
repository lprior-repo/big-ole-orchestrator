#![allow(clippy::unwrap_used)]
use super::*;
use std::collections::HashMap;

fn test_instance_id() -> InstanceId {
    InstanceId::from_bytes([2u8; 16])
}

// ========================================================================
// DedupeEntry Construction
// ========================================================================

#[test]
fn dedupe_entry_constructs_with_valid_fields() {
    let entry = DedupeEntry::new("key-1".to_string(), "instance-1".to_string(), 1000);
    assert!(entry.is_ok());
    let e = entry.unwrap();
    assert_eq!(e.dedupe_key(), "key-1");
    assert_eq!(e.instance_id(), "instance-1");
    assert_eq!(e.expires_at(), 1000);
}

#[test]
fn dedupe_entry_rejects_empty_dedupe_key() {
    let result = DedupeEntry::new("".to_string(), "instance-1".to_string(), 1000);
    assert_eq!(result, Err(DedupeStoreError::InvalidArgument));
}

#[test]
fn dedupe_entry_rejects_empty_instance_id() {
    let result = DedupeEntry::new("key-1".to_string(), "".to_string(), 1000);
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
    assert!(err.to_string().contains("disk full"));
}

#[test]
fn error_codec_displays_reason() {
    let err = DedupeStoreError::Codec {
        reason: "bad json".to_string(),
    };
    assert!(err.to_string().contains("bad json"));
}

#[test]
fn error_invalid_argument_displays_message() {
    let err = DedupeStoreError::InvalidArgument;
    assert!(err.to_string().contains("invalid"));
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
    assert!(matches!(result, Err(DedupeStoreError::Codec { .. })));
}

#[test]
fn decode_dedupe_key_returns_error_for_empty() {
    let result = decode_dedupe_key(&[]);
    assert!(matches!(result, Err(DedupeStoreError::Codec { .. })));
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
    assert!(matches!(result, Err(DedupeStoreError::Codec { .. })));
}

// ========================================================================
// Trait Integration — via MockDedupeStore
// ========================================================================

struct MockDedupeStore {
    entries: std::cell::RefCell<HashMap<String, DedupeEntry>>,
}

impl MockDedupeStore {
    fn new() -> Self {
        Self {
            entries: std::cell::RefCell::new(HashMap::new()),
        }
    }
}

impl DedupeStore for MockDedupeStore {
    fn check_and_insert(
        &self,
        key: &DedupeKey,
        instance_id: &InstanceId,
        ttl_ms: u64,
    ) -> Result<AdmissionResult, DedupeStoreError> {
        if ttl_ms == 0 {
            return Err(DedupeStoreError::InvalidArgument);
        }
        let key_str = key.as_str().to_string();
        let now = 0u64; // Mock: assume time 0
        let entries = self.entries.borrow();

        if let Some(existing) = entries.get(&key_str) {
            if !existing.is_expired(now) {
                return Ok(AdmissionResult::Duplicate {
                    instance_id: existing.instance_id().to_string(),
                });
            }
        }
        drop(entries);

        let entry = DedupeEntry::new(key_str.clone(), format!("{instance_id}"), ttl_ms)?;
        self.entries.borrow_mut().insert(key_str, entry);
        Ok(AdmissionResult::Admitted)
    }

    fn purge_expired(&self, now_ms: u64) -> Result<u64, DedupeStoreError> {
        let mut entries = self.entries.borrow_mut();
        let before = entries.len();
        entries.retain(|_, v| !v.is_expired(now_ms));
        Ok((before - entries.len()) as u64)
    }

    fn contains(&self, key: &DedupeKey) -> Result<bool, DedupeStoreError> {
        let entries = self.entries.borrow();
        let key_str = key.as_str();
        Ok(entries.get(key_str).is_some_and(|e| !e.is_expired(0)))
    }
}

#[test]
fn check_and_insert_returns_admitted_for_new_key() {
    let store = MockDedupeStore::new();
    let key = DedupeKey::parse("new-key").unwrap();
    let result = store.check_and_insert(&key, &test_instance_id(), 5000);
    assert_eq!(result, Ok(AdmissionResult::Admitted));
}

#[test]
fn check_and_insert_returns_duplicate_for_existing_key() {
    let store = MockDedupeStore::new();
    let key = DedupeKey::parse("dup-key").unwrap();
    store
        .check_and_insert(&key, &test_instance_id(), 5000)
        .unwrap();
    let result = store.check_and_insert(&key, &test_instance_id(), 5000);
    assert!(matches!(result, Ok(AdmissionResult::Duplicate { .. })));
}

#[test]
fn check_and_insert_returns_error_for_zero_ttl() {
    let store = MockDedupeStore::new();
    let key = DedupeKey::parse("ttl-key").unwrap();
    let result = store.check_and_insert(&key, &test_instance_id(), 0);
    assert_eq!(result, Err(DedupeStoreError::InvalidArgument));
}

#[test]
fn purge_expired_removes_expired_entries() {
    let store = MockDedupeStore::new();
    let key = DedupeKey::parse("expire-key").unwrap();
    // Insert with ttl=100 (expires at 100)
    store
        .check_and_insert(&key, &test_instance_id(), 100)
        .unwrap();
    let purged = store.purge_expired(100).unwrap();
    assert_eq!(purged, 1);
    let contains = store.contains(&key).unwrap();
    assert!(!contains);
}

#[test]
fn contains_returns_true_for_existing_key() {
    let store = MockDedupeStore::new();
    let key = DedupeKey::parse("contains-key").unwrap();
    store
        .check_and_insert(&key, &test_instance_id(), 99999)
        .unwrap();
    assert!(store.contains(&key).unwrap());
}

#[test]
fn contains_returns_false_for_missing_key() {
    let store = MockDedupeStore::new();
    let key = DedupeKey::parse("missing-key").unwrap();
    assert!(!store.contains(&key).unwrap());
}
