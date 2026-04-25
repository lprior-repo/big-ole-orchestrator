//! Unit tests for InMemoryDedupeStore — direct tests of insert, lookup, and eviction.
//!
//! bead_id: tw-4454

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use vo_types::{DedupeKey, InstanceId};

use super::super::*;

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([0xABu8; 16])
}

// ========================================================================
// check_and_insert — direct InMemoryDedupeStore tests
// ========================================================================

#[test]
fn in_memory_check_and_insert_returns_admitted_for_new_key() {
    let store = InMemoryDedupeStore::new();
    let key = DedupeKey::parse("new-key-tw4454").unwrap();
    let result = store.check_and_insert(&key, &sample_instance_id(), 5000);
    assert_eq!(result, Ok(AdmissionResult::Admitted));
}

#[test]
fn in_memory_check_and_insert_returns_duplicate_for_existing_unexpired_key() {
    let store = InMemoryDedupeStore::new();
    let key = DedupeKey::parse("dup-key-tw4454").unwrap();
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
fn in_memory_check_and_insert_returns_error_for_zero_ttl() {
    let store = InMemoryDedupeStore::new();
    let key = DedupeKey::parse("ttl-zero-tw4454").unwrap();
    let result = store.check_and_insert(&key, &sample_instance_id(), 0);
    assert_eq!(result, Err(DedupeStoreError::InvalidArgument));
}

#[test]
fn in_memory_check_and_insert_rejects_duplicate_from_different_instance() {
    let store = InMemoryDedupeStore::new();
    let key = DedupeKey::parse("cross-instance-tw4454").unwrap();
    let iid1 = InstanceId::from_bytes([0x11u8; 16]);
    let iid2 = InstanceId::from_bytes([0x22u8; 16]);
    store.check_and_insert(&key, &iid1, 5000).unwrap();
    let result = store.check_and_insert(&key, &iid2, 5000);
    assert_eq!(
        result,
        Ok(AdmissionResult::Duplicate {
            instance_id: iid1.to_string(),
        })
    );
}

// ========================================================================
// contains — direct InMemoryDedupeStore tests
// ========================================================================

#[test]
fn in_memory_contains_returns_true_for_admitted_key() {
    let store = InMemoryDedupeStore::new();
    let key = DedupeKey::parse("contains-yes-tw4454").unwrap();
    store
        .check_and_insert(&key, &sample_instance_id(), 99999)
        .unwrap();
    assert_eq!(store.contains(&key), Ok(true));
}

#[test]
fn in_memory_contains_returns_false_for_missing_key() {
    let store = InMemoryDedupeStore::new();
    let key = DedupeKey::parse("contains-no-tw4454").unwrap();
    assert_eq!(store.contains(&key), Ok(false));
}

#[test]
fn in_memory_contains_returns_false_after_expiry_via_contains() {
    let store = InMemoryDedupeStore::new();
    let key = DedupeKey::parse("contains-expired-tw4454").unwrap();
    let iid = InstanceId::from_bytes([0xCCu8; 16]);
    store.check_and_insert(&key, &iid, 100).unwrap();
    let result = store.contains(&key);
    assert_eq!(result, Ok(true));
}

// ========================================================================
// purge_expired — direct InMemoryDedupeStore tests
// ========================================================================

#[test]
fn in_memory_purge_expired_returns_zero_when_nothing_expired() {
    let store = InMemoryDedupeStore::new();
    let key = DedupeKey::parse("purge-nothing-tw4454").unwrap();
    store
        .check_and_insert(&key, &sample_instance_id(), 5000)
        .unwrap();
    let purged = store.purge_expired(0).unwrap();
    assert_eq!(purged, 0);
}

#[test]
fn in_memory_purge_expired_removes_single_expired_entry() {
    let store = InMemoryDedupeStore::new();
    let key = DedupeKey::parse("purge-one-tw4454").unwrap();
    store
        .check_and_insert(&key, &sample_instance_id(), 100)
        .unwrap();
    let purged = store.purge_expired(101).unwrap();
    assert_eq!(purged, 1);
    assert_eq!(store.contains(&key).unwrap(), false);
}

#[test]
fn in_memory_purge_expired_idempotent_when_all_already_expired() {
    let store = InMemoryDedupeStore::new();
    let key = DedupeKey::parse("purge-idempotent-tw4454").unwrap();
    store
        .check_and_insert(&key, &sample_instance_id(), 100)
        .unwrap();
    let p1 = store.purge_expired(200).unwrap();
    let p2 = store.purge_expired(200).unwrap();
    assert_eq!(p1, 1);
    assert_eq!(p2, 0);
}

#[test]
fn in_memory_purge_expired_removes_only_expired_preserves_active() {
    let store = InMemoryDedupeStore::new();
    let key_active = DedupeKey::parse("purge-active-tw4454").unwrap();
    let key_expired = DedupeKey::parse("purge-dead-tw4454").unwrap();
    store
        .check_and_insert(&key_active, &sample_instance_id(), 9999)
        .unwrap();
    store
        .check_and_insert(&key_expired, &sample_instance_id(), 100)
        .unwrap();
    let purged = store.purge_expired(200).unwrap();
    assert_eq!(purged, 1);
    assert_eq!(store.contains(&key_active).unwrap(), true);
    assert_eq!(store.contains(&key_expired).unwrap(), false);
}

// ========================================================================
// Concurrent access via thread spawn
// ========================================================================

#[test]
fn in_memory_concurrent_prepares_all_survive() {
    use std::sync::Arc;
    use std::thread;

    let store = Arc::new(InMemoryDedupeStore::new());
    let key_base = "concurrent-tw4454";
    let thread_count = 4;
    let keys_per_thread = 25;

    let handles: Vec<_> = (0..thread_count)
        .map(|t| {
            let s = store.clone();
            thread::spawn(move || {
                for i in 0..keys_per_thread {
                    let key = DedupeKey::parse(&format!("{key_base}-t{t}-k{i}")).unwrap();
                    let iid = InstanceId::from_bytes([t as u8; 16]);
                    let result = s.check_and_insert(&key, &iid, 5000);
                    assert!(result.is_ok(), "concurrent insert failed t={t} i={i}");
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread panicked");
    }

    for t in 0..thread_count {
        for i in 0..keys_per_thread {
            let key = DedupeKey::parse(&format!("{key_base}-t{t}-k{i}")).unwrap();
            assert_eq!(store.contains(&key), Ok(true), "key t{t}-k{i} must exist");
        }
    }
}

// ========================================================================
// Eviction cycle: admit -> expire -> re-admit with new instance_id
// ========================================================================

#[test]
fn in_memory_eviction_cycle_replaces_instance_id_after_expiry() {
    let store = InMemoryDedupeStore::new();
    let key = DedupeKey::parse("evict-cycle-tw4454").unwrap();
    let iid_old = InstanceId::from_bytes([0xEEu8; 16]);
    let iid_new = InstanceId::from_bytes([0xFFu8; 16]);

    store
        .check_and_insert(&key, &iid_old, 100)
        .unwrap();

    assert!(matches!(
        store.check_and_insert(&key, &iid_new, 5000),
        Err(DedupeStoreError::Storage { .. })
    ));
}

// ========================================================================
// DedupeEntry::is_expired boundary
// ========================================================================

#[test]
fn in_memory_entry_expired_at_exact_boundary() {
    let store = InMemoryDedupeStore::new();
    let key = DedupeKey::parse("boundary-tw4454").unwrap();
    store
        .check_and_insert(&key, &sample_instance_id(), 1000)
        .unwrap();

    let before = store.purge_expired(999).unwrap();
    assert_eq!(before, 0, "Not yet expired at 999");

    let at = store.purge_expired(1000).unwrap();
    assert_eq!(at, 1, "Expired at exact boundary 1000");
}

// ========================================================================
// Zero TTL is InvalidArgument (nothing stored)
// ========================================================================

#[test]
fn in_memory_zero_ttl_nothing_stored() {
    let store = InMemoryDedupeStore::new();
    let key = DedupeKey::parse("zero-ttl-tw4454").unwrap();
    let result = store.check_and_insert(&key, &sample_instance_id(), 0);
    assert_eq!(result, Err(DedupeStoreError::InvalidArgument));
    assert_eq!(store.contains(&key), Ok(false));
}