#![allow(clippy::unwrap_used)]
//! Integration tests for purge_expired operation.

use super::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([2u8; 16])
}

// ========================================================================
// Minimal DeterministicDedupeStore for purge tests only
// ========================================================================

struct PurgeTestStore {
    entries: RefCell<HashMap<String, DedupeEntry>>,
    failure_mode: Option<String>,
}

impl PurgeTestStore {
    fn new() -> Self {
        Self {
            entries: RefCell::new(HashMap::new()),
            failure_mode: None,
        }
    }

    fn with_entries(entries: HashMap<String, DedupeEntry>) -> Self {
        Self {
            entries: RefCell::new(entries),
            failure_mode: None,
        }
    }

    fn failing_purge_expired(reason: &str) -> Self {
        Self {
            entries: RefCell::new(HashMap::new()),
            failure_mode: Some(reason.to_string()),
        }
    }
}

impl DedupeStore for PurgeTestStore {
    fn check_and_insert(
        &self,
        key: &DedupeKey,
        instance_id: &InstanceId,
        ttl_ms: u64,
    ) -> Result<AdmissionResult, DedupeStoreError> {
        let key_str = key.as_str().to_string();
        let entry = DedupeEntry::new(key_str.clone(), format!("{instance_id}"), ttl_ms)?;
        self.entries.borrow_mut().insert(key_str, entry);
        Ok(AdmissionResult::Admitted)
    }

    fn purge_expired(&self, now_ms: u64) -> Result<u64, DedupeStoreError> {
        if let Some(reason) = &self.failure_mode {
            return Err(DedupeStoreError::Storage {
                reason: reason.clone(),
            });
        }
        let mut entries = self.entries.borrow_mut();
        let before = entries.len();
        entries.retain(|_, v| !v.is_expired(now_ms));
        Ok((before - entries.len()) as u64)
    }

    fn contains(&self, _key: &DedupeKey) -> Result<bool, DedupeStoreError> {
        Ok(false)
    }
}

// ========================================================================
// purge_expired Tests
// ========================================================================

#[test]
fn purge_expired_removes_expired_entries() {
    let store = PurgeTestStore::new();
    let key = DedupeKey::parse("expire-key").unwrap();
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
    let store = PurgeTestStore::new();
    let key = DedupeKey::parse("active-key").unwrap();
    store
        .check_and_insert(&key, &sample_instance_id(), 100)
        .unwrap();

    let result = store.purge_expired(99);

    assert_eq!(result, Ok(0));
}

#[test]
fn purge_expired_returns_exact_storage_error_when_store_backend_fails() {
    let store = PurgeTestStore::failing_purge_expired("backend scan failed");
    let result = store.purge_expired(100);
    assert_eq!(
        result,
        Err(DedupeStoreError::Storage {
            reason: "backend scan failed".to_string(),
        })
    );
}
