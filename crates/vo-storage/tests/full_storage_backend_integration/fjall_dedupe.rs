//! Fjall-backed deduplication store tests: basic admit/contain, power failure survival, expiry.
//!
//! Covers PERS-015 through PERS-017.

use crate::full_storage_backend_integration::config::*;
use vo_storage::dedupe_partition::{AdmissionResult, FjallDedupeStore};

// ---------------------------------------------------------------------------
// PERS-015: FjallDedupeStore basic check_and_insert lifecycle
// ---------------------------------------------------------------------------

#[test]
fn pers_015_fjall_dedupe_basic_admit_new_key() {
    let dir = tempfile::tempdir().unwrap();
    let keyspace = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallDedupeStore::open(&keyspace).unwrap();
    let key = sample_dedupe_key("pers-dedupe-basic");
    let id = sample_instance_id();

    let result = store.check_and_insert(&key, &id, 5000).unwrap();
    assert!(
        matches!(result, AdmissionResult::Admitted),
        "First insert must be admitted"
    );

    let result2 = store.check_and_insert(&key, &id, 5000).unwrap();
    assert!(
        matches!(result2, AdmissionResult::Duplicate { .. }),
        "Second insert must be duplicate"
    );
}

#[test]
fn pers_015_fjall_dedupe_contains_after_insert() {
    let dir = tempfile::tempdir().unwrap();
    let keyspace = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallDedupeStore::open(&keyspace).unwrap();
    let key = sample_dedupe_key("pers-dedupe-contain");
    let id = sample_instance_id();

    assert!(
        !store.contains(&key).unwrap(),
        "Key must not exist before insert"
    );

    store.check_and_insert(&key, &id, 5000).unwrap();

    assert!(store.contains(&key).unwrap(), "Key must exist after insert");
}

// ---------------------------------------------------------------------------
// PERS-016: FjallDedupeStore power failure survival
// ---------------------------------------------------------------------------

#[test]
fn pers_016_fjall_dedupe_power_failure_survives() {
    let dir = tempfile::tempdir().unwrap();
    let key1 = sample_dedupe_key("pers-dedup-pf-1");
    let key2 = sample_dedupe_key("pers-dedup-pf-2");
    let id = sample_instance_id();

    {
        let keyspace = fjall::Database::builder(dir.path()).open().unwrap();
        let store = FjallDedupeStore::open(&keyspace).unwrap();
        store.check_and_insert(&key1, &id, 10000).unwrap();
        store.check_and_insert(&key2, &id, 10000).unwrap();
    }

    let keyspace = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallDedupeStore::open(&keyspace).unwrap();

    assert!(
        store.contains(&key1).unwrap(),
        "Key1 must persist after crash"
    );
    assert!(
        store.contains(&key2).unwrap(),
        "Key2 must persist after crash"
    );
}

// ---------------------------------------------------------------------------
// PERS-017: FjallDedupeStore expiry persists across restart
// ---------------------------------------------------------------------------

#[test]
fn pers_017_fjall_dedupe_expiry_persists_across_restart() {
    let dir = tempfile::tempdir().unwrap();
    let key = sample_dedupe_key("pers-dedup-expiry");
    let id = sample_instance_id();

    {
        let keyspace = fjall::Database::builder(dir.path()).open().unwrap();
        let store = FjallDedupeStore::open(&keyspace).unwrap();
        store.check_and_insert(&key, &id, 100).unwrap();
    }

    std::thread::sleep(std::time::Duration::from_millis(150));

    let keyspace = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallDedupeStore::open(&keyspace).unwrap();

    assert!(
        !store.contains(&key).unwrap(),
        "Key must be expired after restart"
    );
}
