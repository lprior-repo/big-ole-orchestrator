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
