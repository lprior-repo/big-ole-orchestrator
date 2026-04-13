#![allow(clippy::unwrap_used)]
#![allow(clippy::indexing_ref)]
//! Red Queen adversarial tests for FjallDedupeStore (ve-g06a).
//!
//! Probes race conditions, partition boundary behavior, key collision handling,
//! post-compaction integrity, and zero-data-loss guarantees.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

use tempfile::tempdir;
use vo_types::DedupeKey;

use vo_storage::dedupe_partition::{
    AdmissionResult, DedupeEntry, DedupeStore, DedupeStoreError, FjallDedupeStore, DEDUPE_PARTITION,
};

fn create_test_keyspace() -> fjall::Keyspace {
    let dir = tempdir().unwrap();
    fjall::Config::new(dir.path()).open().unwrap()
}

fn sample_instance_id() -> vo_types::InstanceId {
    vo_types::InstanceId::from_bytes([1u8; 16])
}

// ========================================================================
// DIMENSION: concurrent same-key — FjallDedupeStore thread safety
// Tests concurrent check_and_insert with same key on real Fjall store
// ========================================================================

#[test]
fn rq_fjall_concurrent_same_key_exactly_one_admitted() {
    let keyspace = create_test_keyspace();
    let store = Arc::new(FjallDedupeStore::open(&keyspace).unwrap());
    let key = DedupeKey::parse("rq-fjall-same-key-vega").unwrap();
    let num_threads = 8usize;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let store = Arc::clone(&store);
            let key = key.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let iid = vo_types::InstanceId::from_bytes([i as u8; 16]);
                store.check_and_insert(&key, &iid, 60_000)
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let admitted_count = results
        .iter()
        .filter(|r| matches!(r, Ok(AdmissionResult::Admitted)))
        .count();
    let dup_count = results
        .iter()
        .filter(|r| matches!(r, Ok(AdmissionResult::Duplicate { .. })))
        .count();

    assert_eq!(admitted_count, 1, "Exactly one thread must win");
    assert_eq!(dup_count, num_threads - 1, "All others must be Duplicate");
    assert_eq!(admitted_count + dup_count, num_threads);
}

#[test]
fn rq_fjall_concurrent_distinct_keys_all_admitted() {
    let keyspace = create_test_keyspace();
    let store = Arc::new(FjallDedupeStore::open(&keyspace).unwrap());
    let num_threads = 16usize;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let key = DedupeKey::parse(&format!("rq-fjall-distinct-{i}-vega")).unwrap();
                let iid = vo_types::InstanceId::from_bytes([i as u8; 16]);
                store.check_and_insert(&key, &iid, 60_000)
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let admitted_count = results
        .iter()
        .filter(|r| *r == &Ok(AdmissionResult::Admitted))
        .count();

    assert_eq!(
        admitted_count, num_threads,
        "All distinct keys must be admitted"
    );
}

#[test]
fn rq_fjall_concurrent_duplicate_preserves_winner_instance_id() {
    let keyspace = create_test_keyspace();
    let store = Arc::new(FjallDedupeStore::open(&keyspace).unwrap());
    let key = DedupeKey::parse("rq-fjall-iid-preserve-vega").unwrap();
    let winner_iid = vo_types::InstanceId::from_bytes([0xAA; 16]);
    let num_threads = 4usize;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads));

    store.check_and_insert(&key, &winner_iid, 60_000).unwrap();

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let store = Arc::clone(&store);
            let key = key.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let iid = vo_types::InstanceId::from_bytes([i as u8; 16]);
                store.check_and_insert(&key, &iid, 60_000)
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    for result in &results {
        match result {
            Ok(AdmissionResult::Duplicate { instance_id }) => {
                assert_eq!(
                    instance_id,
                    &winner_iid.to_string(),
                    "Duplicate must report the ORIGINAL winner's instance_id"
                );
            }
            other => panic!("Expected Duplicate, got: {other:?}"),
        }
    }
}

// ========================================================================
// DIMENSION: zero data loss — admitted entries are fully recoverable
// ========================================================================

#[test]
fn rq_fjall_zero_data_loss_admitted_entries_recoverable() {
    let keyspace = create_test_keyspace();
    let store = FjallDedupeStore::open(&keyspace).unwrap();
    let mut admitted_keys = Vec::new();

    for i in 0..100 {
        let key = DedupeKey::parse(&format!("rq-fjall-zdl-{i}-vega")).unwrap();
        let iid = vo_types::InstanceId::from_bytes([i as u8; 16]);
        let result = store.check_and_insert(&key, &iid, 99999).unwrap();
        assert_eq!(
            result,
            AdmissionResult::Admitted,
            "Key {i} should be admitted"
        );
        admitted_keys.push((key, iid));
    }

    for (key, expected_iid) in &admitted_keys {
        let contains = store.contains(key).unwrap();
        assert!(contains, "Admitted key must be contained");

        let dup_result = store.check_and_insert(key, expected_iid, 99999).unwrap();
        assert!(
            matches!(dup_result, AdmissionResult::Duplicate { .. }),
            "Re-insert of admitted key must return Duplicate"
        );
    }
}

#[test]
fn rq_fjall_zero_data_loss_after_mixed_admit_deny() {
    let keyspace = create_test_keyspace();
    let store = FjallDedupeStore::open(&keyspace).unwrap();

    let key_a = DedupeKey::parse("rq-fjall-mixed-a-vega").unwrap();
    let key_b = DedupeKey::parse("rq-fjall-mixed-b-vega").unwrap();
    let key_c = DedupeKey::parse("rq-fjall-mixed-c-vega").unwrap();

    let iid_a = vo_types::InstanceId::from_bytes([0xAA; 16]);
    let iid_b = vo_types::InstanceId::from_bytes([0xBB; 16]);
    let iid_c = vo_types::InstanceId::from_bytes([0xCC; 16]);

    assert_eq!(
        store.check_and_insert(&key_a, &iid_a, 99999).unwrap(),
        AdmissionResult::Admitted
    );
    assert_eq!(
        store.check_and_insert(&key_a, &iid_b, 99999).unwrap(),
        AdmissionResult::Admitted
    );
    assert_eq!(
        store.check_and_insert(&key_b, &iid_b, 99999).unwrap(),
        AdmissionResult::Admitted
    );
    assert_eq!(
        store.check_and_insert(&key_a, &iid_c, 99999).unwrap(),
        AdmissionResult::Admitted
    );
    assert_eq!(
        store.check_and_insert(&key_c, &iid_c, 99999).unwrap(),
        AdmissionResult::Admitted
    );

    assert_eq!(store.contains(&key_a).unwrap(), true);
    assert_eq!(store.contains(&key_b).unwrap(), true);
    assert_eq!(store.contains(&key_c).unwrap(), true);

    let count: u64 = store.purge_expired(u64::MAX).unwrap();
    assert_eq!(count, 0, "No entries should be expired");
}

// ========================================================================
// DIMENSION: dedup across partition boundaries
// Verifies DedupeStore works correctly when using different partition configs
// ========================================================================

#[test]
fn rq_fjall_partition_constant_is_exactly_dedupe() {
    assert_eq!(DEDUPE_PARTITION, "dedupe");
}

#[test]
fn rq_fjall_different_keyspaces_isolated() {
    let dir = tempdir().unwrap();
    let config = fjall::Config::new(dir.path());

    let keyspace_a = config.open().unwrap();
    let keyspace_b = config.open().unwrap();

    let store_a = FjallDedupeStore::open(&keyspace_a).unwrap();
    let store_b = FjallDedupeStore::open(&keyspace_b).unwrap();

    let key = DedupeKey::parse("rq-fjall-iso-key-vega").unwrap();
    let iid_a = vo_types::InstanceId::from_bytes([0xAA; 16]);
    let iid_b = vo_types::InstanceId::from_bytes([0xBB; 16]);

    let result_a = store_a.check_and_insert(&key, &iid_a, 99999).unwrap();
    assert_eq!(result_a, AdmissionResult::Admitted);

    let result_a2 = store_a.check_and_insert(&key, &iid_a, 99999).unwrap();
    assert!(matches!(result_a2, AdmissionResult::Duplicate { .. }));

    let result_b = store_b.check_and_insert(&key, &iid_b, 99999).unwrap();
    assert_eq!(
        result_b,
        AdmissionResult::Admitted,
        "Different keyspace must not share dedupe state"
    );
}

// ========================================================================
// DIMENSION: dedup key collision — probe edge cases
// ========================================================================

#[test]
fn rq_fjall_keys_with_special_characters() {
    let keyspace = create_test_keyspace();
    let store = FjallDedupeStore::open(&keyspace).unwrap();

    let special_keys = [
        "key\x00with\x00nulls",
        "key/with/slashes",
        "key.with.dots",
        "key:with:colons",
        "key#with#hashes",
        "key?with?questions",
        "key&with&ampersands",
        "key=with=equals",
        "key+with+pluses",
        "key%with%percent",
    ];

    for (i, key_str) in special_keys.iter().enumerate() {
        let key = DedupeKey::parse(key_str).unwrap();
        let iid = vo_types::InstanceId::from_bytes([i as u8; 16]);
        let result = store.check_and_insert(&key, &iid, 99999);
        assert_eq!(
            result,
            Ok(AdmissionResult::Admitted),
            "Key with special char {key_str} must be admitted"
        );
        let dup = store.check_and_insert(&key, &iid, 99999);
        assert!(
            matches!(dup, Ok(AdmissionResult::Duplicate { .. })),
            "Duplicate must be detected for {key_str}"
        );
    }
}

#[test]
fn rq_fjall_unicode_keys_roundtrip() {
    let keyspace = create_test_keyspace();
    let store = FjallDedupeStore::open(&keyspace).unwrap();

    let unicode_keys = [
        "日本語テストキー",
        "🎉emoji🎊",
        "café",
        "naïve",
        "Абракадабра",
        "🦀rustacean�🦂",
    ];

    for (i, key_str) in unicode_keys.iter().enumerate() {
        let key = DedupeKey::parse(key_str).unwrap();
        let iid = vo_types::InstanceId::from_bytes([i as u8; 16]);
        let result = store.check_and_insert(&key, &iid, 99999);
        assert_eq!(
            result,
            Ok(AdmissionResult::Admitted),
            "Unicode key {key_str} must be admitted"
        );
        assert!(
            store.contains(&key).unwrap(),
            "Unicode key must be contained after insert"
        );
    }
}

#[test]
fn rq_fjall_max_length_key_256_chars() {
    let keyspace = create_test_keyspace();
    let store = FjallDedupeStore::open(&keyspace).unwrap();
    let key256 = "a".repeat(256);
    let key = DedupeKey::parse(&key256).unwrap();
    let iid = sample_instance_id();

    let result = store.check_and_insert(&key, &iid, 99999);
    assert_eq!(
        result,
        Ok(AdmissionResult::Admitted),
        "256-char key must be admitted"
    );
    assert!(
        store.contains(&key).unwrap(),
        "256-char key must be contained"
    );

    let dup = store.check_and_insert(&key, &iid, 99999);
    assert!(
        matches!(dup, Ok(AdmissionResult::Duplicate { .. })),
        "Duplicate 256-char key must be detected"
    );
}

// ========================================================================
// DIMENSION: dedup after compaction
// ========================================================================

#[test]
fn rq_fjall_post_compaction_dedupe_still_works() {
    let dir = tempdir().unwrap();
    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let store = FjallDedupeStore::open(&keyspace).unwrap();

    let keys: Vec<_> = (0..50)
        .map(|i| DedupeKey::parse(&format!("rq-fjall-compact-{i}-vega")).unwrap())
        .collect();
    let iid = sample_instance_id();

    for key in &keys {
        store.check_and_insert(key, &iid, 99999).unwrap();
    }

    keyspace.persist(fjall::PersistMode::SyncAll).unwrap();

    let partition = keyspace
        .open_partition(DEDUPE_PARTITION, fjall::PartitionCreateOptions::default())
        .unwrap();
    partition.major_compact().unwrap();

    for key in &keys {
        assert!(store.contains(key).unwrap(), "Key must survive compaction");
        let dup = store.check_and_insert(key, &iid, 99999);
        assert!(
            matches!(dup, Ok(AdmissionResult::Duplicate { .. })),
            "Duplicate detection must work after compaction"
        );
    }
}

#[test]
fn rq_fjall_post_compaction_zero_data_loss() {
    let dir = tempdir().unwrap();
    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let store = FjallDedupeStore::open(&keyspace).unwrap();

    let mut admitted = Vec::new();
    for i in 0..100 {
        let key = DedupeKey::parse(&format!("rq-fjall-pcl-{i}-vega")).unwrap();
        let iid = vo_types::InstanceId::from_bytes([i as u8; 16]);
        store.check_and_insert(&key, &iid, 99999).unwrap();
        admitted.push((key, iid));
    }

    keyspace.persist(fjall::PersistMode::SyncAll).unwrap();

    let partition = keyspace
        .open_partition(DEDUPE_PARTITION, fjall::PartitionCreateOptions::default())
        .unwrap();
    partition.major_compact().unwrap();

    for (key, expected_iid) in &admitted {
        assert!(
            store.contains(key).unwrap(),
            "All admitted keys must survive compaction"
        );
        let dup_result = store.check_and_insert(key, expected_iid, 99999).unwrap();
        assert!(
            matches!(dup_result, AdmissionResult::Duplicate { .. }),
            "Duplicate must be detected post-compaction"
        );
    }
}

#[test]
fn rq_fjall_post_compaction_new_keys_still_admitted() {
    let dir = tempdir().unwrap();
    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let store = FjallDedupeStore::open(&keyspace).unwrap();

    let key_pre = DedupeKey::parse("rq-fjall-pre-compact-vega").unwrap();
    store
        .check_and_insert(&key_pre, &sample_instance_id(), 99999)
        .unwrap();

    keyspace.persist(fjall::PersistMode::SyncAll).unwrap();

    let partition = keyspace
        .open_partition(DEDUPE_PARTITION, fjall::PartitionCreateOptions::default())
        .unwrap();
    partition.major_compact().unwrap();

    let key_post = DedupeKey::parse("rq-fjall-post-compact-vega").unwrap();
    let result = store
        .check_and_insert(&key_post, &sample_instance_id(), 99999)
        .unwrap();
    assert_eq!(
        result,
        AdmissionResult::Admitted,
        "New keys must still be admitted after compaction"
    );
    assert!(
        store.contains(&key_pre).unwrap(),
        "Pre-compaction key must still exist"
    );
    assert!(
        store.contains(&key_post).unwrap(),
        "Post-compaction key must exist"
    );
}

// ========================================================================
// DIMENSION: purge after compaction
// ========================================================================

#[test]
fn rq_fjall_post_compaction_purge_removes_expired() {
    let dir = tempdir().unwrap();
    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let store = FjallDedupeStore::open(&keyspace).unwrap();

    let key_short = DedupeKey::parse("rq-fjall-short-ttl-vega").unwrap();
    let key_long = DedupeKey::parse("rq-fjall-long-ttl-vega").unwrap();

    store
        .check_and_insert(&key_short, &sample_instance_id(), 1)
        .unwrap();
    store
        .check_and_insert(&key_long, &sample_instance_id(), u64::MAX)
        .unwrap();

    keyspace.persist(fjall::PersistMode::SyncAll).unwrap();

    let partition = keyspace
        .open_partition(DEDUPE_PARTITION, fjall::PartitionCreateOptions::default())
        .unwrap();
    partition.major_compact().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(10));

    let purged = store.purge_expired(u64::MAX).unwrap();
    assert_eq!(purged, 1, "Exactly one expired entry should be purged");

    assert!(
        !store.contains(&key_short).unwrap(),
        "Short TTL key must be purged"
    );
    assert!(
        store.contains(&key_long).unwrap(),
        "Long TTL key must survive"
    );
}

// ========================================================================
// DIMENSION: concurrent purge + insert after compaction
// ========================================================================

#[test]
fn rq_fjall_concurrent_purge_insert_post_compaction() {
    let dir = tempdir().unwrap();
    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let store = Arc::new(FjallDedupeStore::open(&keyspace).unwrap());

    for i in 0..20 {
        let key = DedupeKey::parse(&format!("rq-fjall-cpi-{i}-vega")).unwrap();
        store
            .check_and_insert(&key, &sample_instance_id(), 1)
            .unwrap();
    }

    keyspace.persist(fjall::PersistMode::SyncAll).unwrap();

    let partition = keyspace
        .open_partition(DEDUPE_PARTITION, fjall::PartitionCreateOptions::default())
        .unwrap();
    partition.major_compact().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(10));

    let num_inserters = 4usize;
    let num_purgers = 2usize;
    let barrier = Arc::new(std::sync::Barrier::new(num_inserters + num_purgers));

    let inserter_handles: Vec<_> = (0..num_inserters)
        .map(|t| {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let key = DedupeKey::parse(&format!("rq-fjall-cpi-new-{t}-vega")).unwrap();
                let iid = vo_types::InstanceId::from_bytes([t as u8; 16]);
                store.check_and_insert(&key, &iid, 99999)
            })
        })
        .collect();

    let purger_handles: Vec<_> = (0..num_purgers)
        .map(|_| {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                store.purge_expired(u64::MAX)
            })
        })
        .collect();

    let purger_results: Vec<_> = purger_handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect();
    let inserter_results: Vec<_> = inserter_handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect();

    for result in purger_results {
        assert!(result.is_ok());
    }
    for result in inserter_results {
        assert_eq!(result, Ok(AdmissionResult::Admitted));
    }
}

// ========================================================================
// DIMENSION: high contention stress test
// ========================================================================

#[test]
fn rq_fjall_high_contention_many_keys() {
    let keyspace = create_test_keyspace();
    let store = Arc::new(FjallDedupeStore::open(&keyspace).unwrap());
    let num_threads = 16usize;
    let num_keys = 64usize;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|t| {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let mut admitted = 0u32;
                let mut duplicates = 0u32;
                for k in 0..num_keys {
                    let key = DedupeKey::parse(&format!("rq-fjall-stress-k{k}-vega")).unwrap();
                    let iid =
                        vo_types::InstanceId::from_bytes([(t as u8).wrapping_add(k as u8); 16]);
                    match store.check_and_insert(&key, &iid, 99999).unwrap() {
                        AdmissionResult::Admitted => admitted += 1,
                        AdmissionResult::Duplicate { .. } => duplicates += 1,
                    }
                }
                (admitted, duplicates)
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let total_admitted: u32 = results.iter().map(|(a, _)| a).sum();
    let total_dupes: u32 = results.iter().map(|(_, d)| d).sum();

    assert_eq!(
        total_admitted + total_dupes,
        (num_threads * num_keys) as u32
    );
    assert_eq!(
        total_admitted, num_keys as u32,
        "Exactly one admission per key"
    );
    assert_eq!(
        total_dupes,
        ((num_threads - 1) * num_keys) as u32,
        "All other threads must get Duplicate"
    );
}

// ========================================================================
// DIMENSION: instance_id preservation invariant
// ========================================================================

#[test]
fn rq_fjall_instance_id_preserved_across_multiple_duplicates() {
    let keyspace = create_test_keyspace();
    let store = FjallDedupeStore::open(&keyspace).unwrap();
    let key = DedupeKey::parse("rq-fjall-iid-4ever-vega").unwrap();
    let original_iid = vo_types::InstanceId::from_bytes([0xDE; 16]);

    store.check_and_insert(&key, &original_iid, 99999).unwrap();

    for _ in 0..10 {
        let dup_result = store
            .check_and_insert(&key, &sample_instance_id(), 99999)
            .unwrap();
        match dup_result {
            AdmissionResult::Duplicate { instance_id } => {
                assert_eq!(instance_id, original_iid.to_string());
            }
            other => panic!("Expected Duplicate, got {other:?}"),
        }
    }
}

// ========================================================================
// DIMENSION: read-your-writes consistency
// ========================================================================

#[test]
fn rq_fjall_read_your_writes_consistency() {
    let keyspace = create_test_keyspace();
    let store = FjallDedupeStore::open(&keyspace).unwrap();

    for i in 0..100 {
        let key = DedupeKey::parse(&format!("rq-fjall-ryw-{i}-vega")).unwrap();
        let iid = vo_types::InstanceId::from_bytes([i as u8; 16]);
        store.check_and_insert(&key, &iid, 99999).unwrap();
        assert!(
            store.contains(&key).unwrap(),
            "Write must be immediately visible"
        );
    }
}

// ========================================================================
// DIMENSION: batch operations preserve invariants
// ========================================================================

#[test]
fn rq_fjall_many_small_ttls_eventually_expire() {
    let keyspace = create_test_keyspace();
    let store = FjallDedupeStore::open(&keyspace).unwrap();

    for i in 0..10 {
        let key = DedupeKey::parse(&format!("rq-fjall-ttl-{i}-vega")).unwrap();
        store
            .check_and_insert(&key, &sample_instance_id(), 1)
            .unwrap();
    }

    std::thread::sleep(std::time::Duration::from_millis(20));

    let purged = store.purge_expired(u64::MAX).unwrap();
    assert_eq!(purged, 10, "All short TTL entries must be purgeable");

    for i in 0..10 {
        let key = DedupeKey::parse(&format!("rq-fjall-ttl-{i}-vega")).unwrap();
        assert!(
            !store.contains(&key).unwrap(),
            "Purged key must not be contained"
        );
    }
}
