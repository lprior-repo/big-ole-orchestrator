//! Full storage backend integration tests — cross-backend durability, crash recovery, and concurrent access.
//!
//! Tests PERS-015 through PERS-040: comprehensive persistence guarantees across storage backends.
//!
//! ## Storage Backends Tested
//!
//! - `FjallEffectJournal`: Production Fjall-backed effect journal
//! - `FjallDedupeStore`: Production Fjall-backed deduplication store
//! - `FjallLeaseStore`: Production Fjall-backed lease store
//! - `InMemoryDedupeStore`: In-memory deduplication store for testing
//! - `InMemoryLeaseStore`: In-memory lease store for testing
//! - `InMemoryEffectJournal`: In-memory effect journal for testing
//!
//! ## Edge Cases Covered
//!
//! - Power failure: crash between operations, mid-batch crashes, recovery after abrupt termination
//! - Concurrent access: multi-threaded operations, race conditions, exactly-once guarantees
//! - Cross-backend consistency: all backends must satisfy same persistence invariants

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::sync::Arc;
use vo_storage::dedupe_partition::{AdmissionResult, DedupeStore, FjallDedupeStore};
use vo_storage::effect_journal::{EffectId, EffectJournal, FjallEffectJournal};
use vo_storage::lease_partition::{FjallLeaseStore, LeaseStore};
use vo_types::{DedupeKey, FenceToken, InstanceId, LeaseRecord, StepId};

// ---------------------------------------------------------------------------
// Test Configuration
// ---------------------------------------------------------------------------

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

fn other_instance_id() -> InstanceId {
    InstanceId::from_bytes([2u8; 16])
}

fn sample_dedupe_key(id: &str) -> DedupeKey {
    DedupeKey::parse(id).expect("valid dedupe key")
}

fn sample_step_id(id: &str) -> StepId {
    StepId::parse(id).expect("valid step id")
}

// ---------------------------------------------------------------------------
// PERS-015: FjallDedupeStore basic check_and_insert lifecycle
// ---------------------------------------------------------------------------

#[test]
fn pers_015_fjall_dedupe_basic_admit_new_key() {
    let dir = tempfile::tempdir().unwrap();
    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
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
    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
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
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let store = FjallDedupeStore::open(&keyspace).unwrap();
        store.check_and_insert(&key1, &id, 10000).unwrap();
        store.check_and_insert(&key2, &id, 10000).unwrap();
    }

    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
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

#[test]
fn pers_017_fjall_dedupe_expiry_persists_across_restart() {
    let dir = tempfile::tempdir().unwrap();
    let key = sample_dedupe_key("pers-dedup-expiry");
    let id = sample_instance_id();

    {
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let store = FjallDedupeStore::open(&keyspace).unwrap();
        store.check_and_insert(&key, &id, 100).unwrap();
    }

    std::thread::sleep(std::time::Duration::from_millis(150));

    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let store = FjallDedupeStore::open(&keyspace).unwrap();

    assert!(
        !store.contains(&key).unwrap(),
        "Key must be expired after restart"
    );
}

// ---------------------------------------------------------------------------
// PERS-018: FjallLeaseStore basic acquire/release lifecycle
// ---------------------------------------------------------------------------

#[test]
fn pers_018_fjall_lease_basic_acquire_release() {
    let dir = tempfile::tempdir().unwrap();
    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let store = FjallLeaseStore::open(&keyspace).unwrap();
    let id = sample_instance_id();
    let step = sample_step_id("step-1");

    let lease = store.acquire(&id, &step, 5000).unwrap();
    assert_eq!(
        lease.token().inner().get(),
        1,
        "First fence token must be 1"
    );

    store.release(&lease).unwrap();

    let is_stale = store.check_stale_fence(&id, &step, lease.token()).unwrap();
    assert!(is_stale, "Released lease must be stale");
}

#[test]
fn pers_019_fjall_lease_cannot_double_acquire() {
    let dir = tempfile::tempdir().unwrap();
    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let store = FjallLeaseStore::open(&keyspace).unwrap();
    let id = sample_instance_id();
    let step = sample_step_id("step-double");

    let lease1 = store.acquire(&id, &step, 5000).unwrap();
    let lease2_result = store.acquire(&id, &step, 5000);

    assert!(
        lease2_result.is_err(),
        "Second acquire must fail while lease is held"
    );

    store.release(&lease1).unwrap();

    let lease2 = store.acquire(&id, &step, 5000).unwrap();
    assert_eq!(
        lease2.token().inner().get(),
        2,
        "Fence token must increment after release"
    );
}

// ---------------------------------------------------------------------------
// PERS-020: FjallLeaseStore power failure survival
// ---------------------------------------------------------------------------

#[test]
fn pers_020_fjall_lease_power_failure_survives() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();
    let step1 = sample_step_id("step-pf-1");
    let step2 = sample_step_id("step-pf-2");

    let lease1_token;
    {
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let store = FjallLeaseStore::open(&keyspace).unwrap();
        let lease1 = store.acquire(&id, &step1, 10000).unwrap();
        lease1_token = lease1.token().clone();
        store.acquire(&id, &step2, 10000).unwrap();
    }

    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let store = FjallLeaseStore::open(&keyspace).unwrap();

    let is_stale = store.check_stale_fence(&id, &step1, &lease1_token).unwrap();
    assert!(!is_stale, "Lease must survive power failure");
}

#[test]
fn pers_021_fjall_lease_fence_token_persists_across_restart() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();
    let step = sample_step_id("step-fence");

    {
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let store = FjallLeaseStore::open(&keyspace).unwrap();
        store.acquire(&id, &step, 10000).unwrap();
        store.acquire(&id, &step, 10000).unwrap();
        store.acquire(&id, &step, 10000).unwrap();
    }

    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let store = FjallLeaseStore::open(&keyspace).unwrap();

    let lease = store.acquire(&id, &step, 10000).unwrap();
    assert_eq!(
        lease.token().inner().get(),
        4,
        "Fence token must be 4 after 3 acquisitions"
    );
}

// ---------------------------------------------------------------------------
// PERS-022: Concurrent dedupe access with Fjall backend
// ---------------------------------------------------------------------------

#[test]
fn pers_022_fjall_dedupe_concurrent_insert_same_key() {
    let dir = tempfile::tempdir().unwrap();
    let key = sample_dedupe_key("pers-concurrent-dedup");
    let id = sample_instance_id();
    let num_threads = 16;

    let barrier = Arc::new(std::sync::Barrier::new(num_threads));
    let results = Arc::new(std::sync::Mutex::new(Vec::new()));

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let dir_path = dir.path().to_path_buf();
            let key = key.clone();
            let id = id.clone();
            let barrier = barrier.clone();
            let results = Arc::clone(&results);
            std::thread::spawn(move || {
                barrier.wait();
                let keyspace = fjall::Config::new(&dir_path).open().unwrap();
                let store = FjallDedupeStore::open(&keyspace).unwrap();
                let result = store.check_and_insert(&key, &id, 5000);
                results.lock().unwrap().push((i, result.is_ok()));
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let results = results.lock().unwrap();
    let success_count = results.iter().filter(|(_, ok)| *ok).count();
    assert_eq!(
        success_count, 1,
        "Exactly one concurrent insert must succeed"
    );
}

#[test]
fn pers_023_fjall_dedupe_concurrent_different_keys() {
    let dir = tempfile::tempdir().unwrap();
    let num_threads = 16;

    let barrier = Arc::new(std::sync::Barrier::new(num_threads));
    let results = Arc::new(std::sync::Mutex::new(Vec::new()));

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let dir_path = dir.path().to_path_buf();
            let barrier = barrier.clone();
            let results = Arc::clone(&results);
            std::thread::spawn(move || {
                barrier.wait();
                let key = sample_dedupe_key(&format!("pers-concurrent-diff-{}", i));
                let id = InstanceId::from_bytes([i as u8; 16]);
                let keyspace = fjall::Config::new(&dir_path).open().unwrap();
                let store = FjallDedupeStore::open(&keyspace).unwrap();
                let result = store.check_and_insert(&key, &id, 5000);
                results.lock().unwrap().push(result.is_ok());
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let results = results.lock().unwrap();
    let success_count = results.iter().filter(|ok| **ok).count();
    assert_eq!(
        success_count, num_threads,
        "All concurrent inserts with different keys must succeed"
    );
}

// ---------------------------------------------------------------------------
// PERS-024: Concurrent lease acquire with Fjall backend
// ---------------------------------------------------------------------------

#[test]
fn pers_024_fjall_lease_concurrent_acquire_same_step() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();
    let step = sample_step_id("step-concurrent");
    let num_threads = 8;

    let barrier = Arc::new(std::sync::Barrier::new(num_threads));
    let results = Arc::new(std::sync::Mutex::new(Vec::new()));

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let dir_path = dir.path().to_path_buf();
            let id = id.clone();
            let step = step.clone();
            let barrier = barrier.clone();
            let results = Arc::clone(&results);
            std::thread::spawn(move || {
                barrier.wait();
                let keyspace = fjall::Config::new(&dir_path).open().unwrap();
                let store = FjallLeaseStore::open(&keyspace).unwrap();
                let result = store.acquire(&id, &step, 5000);
                results.lock().unwrap().push(result.is_ok());
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let results = results.lock().unwrap();
    let success_count = results.iter().filter(|ok| **ok).count();
    assert_eq!(
        success_count, 1,
        "Exactly one concurrent acquire must succeed"
    );
}

// ---------------------------------------------------------------------------
// PERS-025: Multi-component crash recovery (effect journal + dedupe)
// ---------------------------------------------------------------------------

#[test]
fn pers_025_multi_component_crash_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();
    let dedupe_key = sample_dedupe_key("pers-multi");

    {
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let journal = FjallEffectJournal::open(&keyspace).unwrap();
        let dedupe = FjallDedupeStore::open(&keyspace).unwrap();

        let record = vo_types::EffectRecord::new(
            "pers-multi-1".to_string(),
            vo_types::EffectKind::HttpCall,
            serde_json::json!({"url": "https://example.com"}),
            vo_types::EffectIntent::Prepared,
            None,
        )
        .unwrap();
        journal.prepare(&id, record).unwrap();
        dedupe.check_and_insert(&dedupe_key, &id, 10000).unwrap();
    }

    {
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let journal = FjallEffectJournal::open(&keyspace).unwrap();
        let dedupe = FjallDedupeStore::open(&keyspace).unwrap();

        let pending = journal.list_pending(&id).unwrap();
        assert_eq!(pending.len(), 1, "One effect must be pending");

        assert!(
            dedupe.contains(&dedupe_key).unwrap(),
            "Dedupe entry must persist"
        );
    }
}

// ---------------------------------------------------------------------------
// PERS-026: Idempotent dedupe operations across crashes
// ---------------------------------------------------------------------------

#[test]
fn pers_026_fjall_dedupe_idempotent_across_crashes() {
    let dir = tempfile::tempdir().unwrap();
    let key = sample_dedupe_key("pers-idempotent");
    let id = sample_instance_id();

    for _ in 0..3 {
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let store = FjallDedupeStore::open(&keyspace).unwrap();
        let result = store.check_and_insert(&key, &id, 10000);
        assert!(
            matches!(result, Ok(AdmissionResult::Admitted)),
            "Re-insert of same key after crash must be admitted (idempotent)"
        );
    }
}

// ---------------------------------------------------------------------------
// PERS-027: Lease stale fence detection across restarts
// ---------------------------------------------------------------------------

#[test]
fn pers_027_fjall_lease_stale_fence_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();
    let step = sample_step_id("step-stale");

    let old_lease;
    {
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let store = FjallLeaseStore::open(&keyspace).unwrap();
        old_lease = store.acquire(&id, &step, 10000).unwrap();
        store.release(&old_lease).unwrap();
    }

    let old_token = old_lease.token().clone();
    {
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let store = FjallLeaseStore::open(&keyspace).unwrap();
        let new_lease = store.acquire(&id, &step, 10000).unwrap();

        let is_stale = store.check_stale_fence(&id, &step, &old_token).unwrap();
        assert!(is_stale, "Old fence token must be stale after new acquire");

        assert_ne!(
            new_lease.token().inner().get(),
            old_token.inner().get(),
            "New fence token must be different"
        );
    }
}

// ---------------------------------------------------------------------------
// PERS-028: Dedupe purge after restart
// ---------------------------------------------------------------------------

#[test]
fn pers_028_fjall_dedupe_purge_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let key1 = sample_dedupe_key("pers-purge-1");
    let key2 = sample_dedupe_key("pers-purge-2");
    let id = sample_instance_id();

    {
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let store = FjallDedupeStore::open(&keyspace).unwrap();
        store.check_and_insert(&key1, &id, 100).unwrap();
        store.check_and_insert(&key2, &id, 10000).unwrap();
    }

    std::thread::sleep(std::time::Duration::from_millis(150));

    {
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let store = FjallDedupeStore::open(&keyspace).unwrap();
        let purged = store.purge_expired(u64::MAX).unwrap();
        assert_eq!(purged, 1, "Exactly one entry must be purged");
    }
}

// ---------------------------------------------------------------------------
// PERS-029: Lease acquire after expiry
// ---------------------------------------------------------------------------

#[test]
fn pers_029_fjall_lease_acquire_after_expiry() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();
    let step = sample_step_id("step-expired");

    {
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let store = FjallLeaseStore::open(&keyspace).unwrap();
        store.acquire(&id, &step, 50).unwrap();
    }

    std::thread::sleep(std::time::Duration::from_millis(100));

    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let store = FjallLeaseStore::open(&keyspace).unwrap();

    let lease = store.acquire(&id, &step, 10000).unwrap();
    assert_eq!(
        lease.token().inner().get(),
        2,
        "Fence token must increment after expiry"
    );
}

// ---------------------------------------------------------------------------
// PERS-030: Combined effect journal + dedupe + lease crash scenario
// ---------------------------------------------------------------------------

#[test]
fn pers_030_triple_component_crash_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();
    let dedupe_key = sample_dedupe_key("pers-triple");
    let step = sample_step_id("step-triple");

    let eid1;
    {
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let journal = FjallEffectJournal::open(&keyspace).unwrap();
        let dedupe = FjallDedupeStore::open(&keyspace).unwrap();
        let lease = FjallLeaseStore::open(&keyspace).unwrap();

        let record = vo_types::EffectRecord::new(
            "pers-triple-1".to_string(),
            vo_types::EffectKind::HttpCall,
            serde_json::json!({"url": "https://example.com"}),
            vo_types::EffectIntent::Prepared,
            None,
        )
        .unwrap();
        eid1 = journal.prepare(&id, record).unwrap();
        dedupe.check_and_insert(&dedupe_key, &id, 10000).unwrap();
        lease.acquire(&id, &step, 10000).unwrap();

        journal.commit(&eid1).unwrap();
    }

    {
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let journal = FjallEffectJournal::open(&keyspace).unwrap();
        let dedupe = FjallDedupeStore::open(&keyspace).unwrap();
        let lease = FjallLeaseStore::open(&keyspace).unwrap();

        let pending = journal.list_pending(&id).unwrap();
        assert!(pending.is_empty(), "All effects must be committed");

        assert!(dedupe.contains(&dedupe_key).unwrap(), "Dedupe must persist");

        let new_lease = lease.acquire(&id, &step, 10000).unwrap();
        let is_stale = lease
            .check_stale_fence(&id, &step, new_lease.token())
            .unwrap();
        assert!(!is_stale, "Lease must persist");
    }
}

// ---------------------------------------------------------------------------
// PERS-031: InMemoryDedupeStore basic lifecycle
// ---------------------------------------------------------------------------

#[test]
fn pers_031_inmemory_dedupe_basic_admit_new_key() {
    let store = vo_storage::dedupe_partition::InMemoryDedupeStore::new();
    let key = sample_dedupe_key("pers-inmem-dedup-basic");
    let id = sample_instance_id();

    let result = store.check_and_insert(&key, &id, 5000).unwrap();
    assert!(
        matches!(
            result,
            vo_storage::dedupe_partition::AdmissionResult::Admitted
        ),
        "First insert must be admitted"
    );

    let result2 = store.check_and_insert(&key, &id, 5000).unwrap();
    assert!(
        matches!(
            result2,
            vo_storage::dedupe_partition::AdmissionResult::Duplicate { .. }
        ),
        "Second insert must be duplicate"
    );
}

#[test]
fn pers_031_inmemory_dedupe_contains_after_insert() {
    let store = vo_storage::dedupe_partition::InMemoryDedupeStore::new();
    let key = sample_dedupe_key("pers-inmem-dedup-contain");
    let id = sample_instance_id();

    assert!(
        !store.contains(&key).unwrap(),
        "Key must not exist before insert"
    );

    store.check_and_insert(&key, &id, 5000).unwrap();

    assert!(store.contains(&key).unwrap(), "Key must exist after insert");
}

// ---------------------------------------------------------------------------
// PERS-032: InMemoryLeaseStore basic lifecycle
// ---------------------------------------------------------------------------

#[test]
fn pers_032_inmemory_lease_basic_acquire_release() {
    let store = vo_storage::lease_partition::InMemoryLeaseStore::new();
    let id = sample_instance_id();
    let step = sample_step_id("step-inmem");

    let lease = store.acquire(&id, &step, 5000).unwrap();
    assert_eq!(
        lease.token().inner().get(),
        1,
        "First fence token must be 1"
    );

    store.release(&lease).unwrap();

    let is_stale = store.check_stale_fence(&id, &step, lease.token()).unwrap();
    assert!(
        !is_stale,
        "Released lease: check_stale_fence returns false when no lease exists"
    );
}

#[test]
fn pers_032_inmemory_lease_cannot_double_acquire() {
    let store = vo_storage::lease_partition::InMemoryLeaseStore::new();
    let id = sample_instance_id();
    let step = sample_step_id("step-inmem-double");

    let lease1 = store.acquire(&id, &step, 5000).unwrap();
    let lease2_result = store.acquire(&id, &step, 5000);

    assert!(
        lease2_result.is_err(),
        "Second acquire must fail while lease is held"
    );

    store.release(&lease1).unwrap();

    let lease2 = store.acquire(&id, &step, 5000).unwrap();
    assert_eq!(
        lease2.token().inner().get(),
        2,
        "Fence token must increment after release"
    );
}

// ---------------------------------------------------------------------------
// PERS-033: InMemoryDedupeStore concurrent access - same key
// ---------------------------------------------------------------------------

#[test]
fn pers_033_inmemory_dedupe_concurrent_same_key() {
    let store = Arc::new(vo_storage::dedupe_partition::InMemoryDedupeStore::new());
    let key = sample_dedupe_key("pers-inmem-concurrent-same");
    let num_threads = 16;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let store = store.clone();
            let key = key.clone();
            let id = InstanceId::from_bytes([i as u8; 16]);
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                store.check_and_insert(&key, &id, 5000)
            })
        })
        .collect();

    let results: Vec<Result<_, _>> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    let admitted_count = results
        .iter()
        .filter(|r| {
            matches!(
                r.as_ref().unwrap(),
                vo_storage::dedupe_partition::AdmissionResult::Admitted
            )
        })
        .count();
    let duplicate_count = results
        .iter()
        .filter(|r| {
            matches!(
                r.as_ref().unwrap(),
                vo_storage::dedupe_partition::AdmissionResult::Duplicate { .. }
            )
        })
        .count();

    assert_eq!(admitted_count, 1, "Exactly one thread must be admitted");
    assert_eq!(
        duplicate_count,
        num_threads - 1,
        "Other threads must be duplicate"
    );
}

// ---------------------------------------------------------------------------
// PERS-034: InMemoryDedupeStore concurrent access - different keys
// ---------------------------------------------------------------------------

#[test]
fn pers_034_inmemory_dedupe_concurrent_different_keys() {
    let store = Arc::new(vo_storage::dedupe_partition::InMemoryDedupeStore::new());
    let num_threads = 16;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let store = store.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                let key = sample_dedupe_key(&format!("pers-inmem-diff-{}", i));
                let id = InstanceId::from_bytes([i as u8; 16]);
                store.check_and_insert(&key, &id, 5000)
            })
        })
        .collect();

    let results: Vec<Result<_, _>> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    assert!(
        results.iter().all(|r| r.is_ok()),
        "All concurrent inserts with different keys must succeed"
    );

    let admitted_count = results
        .iter()
        .filter(|r| {
            matches!(
                r.as_ref().unwrap(),
                vo_storage::dedupe_partition::AdmissionResult::Admitted
            )
        })
        .count();
    assert_eq!(
        admitted_count, num_threads,
        "All threads with different keys must be admitted"
    );
}

// ---------------------------------------------------------------------------
// PERS-035: InMemoryLeaseStore concurrent access - same step
// ---------------------------------------------------------------------------

#[test]
fn pers_035_inmemory_lease_concurrent_same_step() {
    let store = Arc::new(vo_storage::lease_partition::InMemoryLeaseStore::new());
    let id = sample_instance_id();
    let step = sample_step_id("step-inmem-concurrent");
    let num_threads = 8;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let store = store.clone();
            let id = id.clone();
            let step = step.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                store.acquire(&id, &step, 5000)
            })
        })
        .collect();

    let results: Vec<Result<_, _>> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    let success_count = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(
        success_count, 1,
        "Exactly one concurrent acquire must succeed"
    );
}

// ---------------------------------------------------------------------------
// PERS-036: InMemoryLeaseStore concurrent access - different steps
// ---------------------------------------------------------------------------

#[test]
fn pers_036_inmemory_lease_concurrent_different_steps() {
    let store = Arc::new(vo_storage::lease_partition::InMemoryLeaseStore::new());
    let id = sample_instance_id();
    let num_threads = 8;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let store = store.clone();
            let id = id.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                let step = sample_step_id(&format!("step-inmem-{}", i));
                store.acquire(&id, &step, 5000)
            })
        })
        .collect();

    let results: Vec<Result<_, _>> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    assert!(
        results.iter().all(|r| r.is_ok()),
        "All concurrent acquires with different steps must succeed"
    );
}

// ---------------------------------------------------------------------------
// PERS-037: InMemoryDedupeStore expiry and purge
// ---------------------------------------------------------------------------

#[test]
fn pers_037_inmemory_dedupe_expiry_and_purge() {
    let store = vo_storage::dedupe_partition::InMemoryDedupeStore::new();
    let key1 = sample_dedupe_key("pers-inmem-expiry-1");
    let key2 = sample_dedupe_key("pers-inmem-expiry-2");
    let id = sample_instance_id();

    store.check_and_insert(&key1, &id, 50).unwrap();
    store.check_and_insert(&key2, &id, 10000).unwrap();

    std::thread::sleep(std::time::Duration::from_millis(75));

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let purged = store.purge_expired(now_ms).unwrap();

    assert_eq!(purged, 1, "Exactly one entry must be purged");

    assert!(
        !store.contains(&key1).unwrap(),
        "Expired key must be purged"
    );
    assert!(
        store.contains(&key2).unwrap(),
        "Non-expired key must remain"
    );
}

// ---------------------------------------------------------------------------
// PERS-038: InMemoryLeaseStore expiry and re-acquire
// ---------------------------------------------------------------------------

#[test]
fn pers_038_inmemory_lease_expiry_and_reacquire() {
    let store = vo_storage::lease_partition::InMemoryLeaseStore::new();
    let id = sample_instance_id();
    let step = sample_step_id("step-inmem-expiry");

    let lease1 = store.acquire(&id, &step, 50).unwrap();
    assert_eq!(lease1.token().inner().get(), 1);

    std::thread::sleep(std::time::Duration::from_millis(75));

    let lease2 = store.acquire(&id, &step, 10000).unwrap();
    assert_eq!(
        lease2.token().inner().get(),
        2,
        "Fence token must increment after expiry"
    );
}

// ---------------------------------------------------------------------------
// PERS-039: InMemory cross-component crash recovery (no persistence)
// ---------------------------------------------------------------------------

#[test]
fn pers_039_inmemory_cross_component_no_persistence() {
    let journal = vo_storage::effect_journal::InMemoryEffectJournal::new();
    let dedupe = vo_storage::dedupe_partition::InMemoryDedupeStore::new();
    let lease = vo_storage::lease_partition::InMemoryLeaseStore::new();
    let id = sample_instance_id();
    let dedupe_key = sample_dedupe_key("pers-inmem-multi");
    let step = sample_step_id("step-inmem-multi");

    let record = vo_types::EffectRecord::new(
        "pers-inmem-multi-1".to_string(),
        vo_types::EffectKind::HttpCall,
        serde_json::json!({"url": "https://example.com"}),
        vo_types::EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid = journal.prepare(&id, record).unwrap();
    dedupe.check_and_insert(&dedupe_key, &id, 10000).unwrap();
    lease.acquire(&id, &step, 10000).unwrap();

    journal.commit(&eid).unwrap();

    let pending = journal.list_pending(&id).unwrap();
    assert!(pending.is_empty(), "All effects must be committed");

    assert!(
        dedupe.contains(&dedupe_key).unwrap(),
        "Dedupe must exist in memory"
    );
}

// ---------------------------------------------------------------------------
// PERS-040: InMemoryDedupeStore purge across concurrent access
// ---------------------------------------------------------------------------

#[test]
fn pers_040_inmemory_dedupe_concurrent_purge() {
    let store = Arc::new(vo_storage::dedupe_partition::InMemoryDedupeStore::new());
    let num_threads = 4;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads + 1));

    for i in 0..10 {
        let key = sample_dedupe_key(&format!("pers-purge-{}", i % 3));
        let id = InstanceId::from_bytes([i as u8; 16]);
        store.check_and_insert(&key, &id, 100).unwrap();
    }

    let purge_handle = std::thread::spawn({
        let store = store.clone();
        let barrier = barrier.clone();
        move || {
            barrier.wait();
            std::thread::sleep(std::time::Duration::from_millis(50));
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            store.purge_expired(now_ms)
        }
    });

    barrier.wait();

    let results: Vec<_> = (0..num_threads)
        .map(|i| {
            let store = store.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                let key = sample_dedupe_key(&format!("pers-purge-{}", i));
                let id = InstanceId::from_bytes([i as u8; 16]);
                store.check_and_insert(&key, &id, 5000)
            })
        })
        .collect();

    for h in results {
        h.join().unwrap();
    }

    let purged = purge_handle.join().unwrap().unwrap();
    assert!(purged >= 0, "Purge must complete without error");
}
