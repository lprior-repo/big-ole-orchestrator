#![allow(clippy::unwrap_used)]
//! Red Queen adversarial tests: concurrent writes with thread-safe harness.
//!
//! Probes race conditions in duplicate detection under multi-threaded contention.

use super::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

// ========================================================================
// Thread-safe DedupeStore for concurrent tests
// ========================================================================

struct ConcurrentDedupeStore {
    entries: parking_lot::Mutex<HashMap<String, DedupeEntry>>,
    now_ms: AtomicU64,
}

impl ConcurrentDedupeStore {
    fn new() -> Self {
        Self {
            entries: parking_lot::Mutex::new(HashMap::new()),
            now_ms: AtomicU64::new(0),
        }
    }
}

impl DedupeStore for ConcurrentDedupeStore {
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
        let now = self.now_ms.load(Ordering::SeqCst);

        let mut entries = self.entries.lock();
        if let Some(existing) = entries.get(key_str.as_str()) {
            if !existing.is_expired(now) {
                return Ok(AdmissionResult::Duplicate {
                    instance_id: existing.instance_id().to_string(),
                });
            }
        }
        let entry = DedupeEntry::new(key_str.clone(), format!("{instance_id}"), ttl_ms)?;
        entries.insert(key_str, entry);
        Ok(AdmissionResult::Admitted)
    }

    fn purge_expired(&self, now_ms: u64) -> Result<u64, DedupeStoreError> {
        let mut entries = self.entries.lock();
        let before = entries.len();
        entries.retain(|_, v| !v.is_expired(now_ms));
        Ok((before - entries.len()) as u64)
    }

    fn contains(&self, key: &DedupeKey) -> Result<bool, DedupeStoreError> {
        let entries = self.entries.lock();
        let now = self.now_ms.load(Ordering::SeqCst);
        Ok(entries
            .get(key.as_str())
            .is_some_and(|entry| !entry.is_expired(now)))
    }
}

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([2u8; 16])
}

// ========================================================================
// DIMENSION: concurrent same-key — exactly one winner
// ========================================================================

#[test]
fn rq_concurrent_same_key_exactly_one_admitted() {
    let store = Arc::new(ConcurrentDedupeStore::new());
    let key = DedupeKey::parse("rq-concurrent-same-b2uv").unwrap();
    let num_threads: usize = 8;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let store = Arc::clone(&store);
            let key = key.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let iid = InstanceId::from_bytes([i as u8; 16]);
                store.check_and_insert(&key, &iid, 60_000)
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let admitted_count = results
        .iter()
        .filter(|r| *r == &Ok(AdmissionResult::Admitted))
        .count();
    let dup_count = results
        .iter()
        .filter(|r| matches!(r, Ok(AdmissionResult::Duplicate { .. })))
        .count();

    assert_eq!(
        admitted_count, 1,
        "Exactly one thread must win, got {admitted_count}"
    );
    assert_eq!(
        dup_count,
        num_threads - 1,
        "All others must be Duplicate, got {dup_count}"
    );
    assert_eq!(admitted_count + dup_count, num_threads);
}

// ========================================================================
// DIMENSION: concurrent distinct keys — all admitted
// ========================================================================

#[test]
fn rq_concurrent_distinct_keys_all_admitted() {
    let store = Arc::new(ConcurrentDedupeStore::new());
    let num_threads: usize = 16;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let key = DedupeKey::parse(&format!("rq-distinct-{i}-b2uv")).unwrap();
                let iid = InstanceId::from_bytes([i as u8; 16]);
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

// ========================================================================
// DIMENSION: concurrent duplicate preserves original instance_id
// ========================================================================

#[test]
fn rq_concurrent_duplicate_preserves_winner_instance_id() {
    let store = Arc::new(ConcurrentDedupeStore::new());
    let key = DedupeKey::parse("rq-iid-preserve-b2uv").unwrap();
    let winner_iid = InstanceId::from_bytes([0xAA; 16]);
    let num_threads: usize = 4;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads));

    store.check_and_insert(&key, &winner_iid, 60_000).unwrap();

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let store = Arc::clone(&store);
            let key = key.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let iid = InstanceId::from_bytes([i as u8; 16]);
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
// DIMENSION: concurrent purge + insert — no false duplicates
// ========================================================================

#[test]
fn rq_concurrent_purge_and_insert_no_false_duplicates() {
    let store = Arc::new(ConcurrentDedupeStore::new());
    let num_inserters: usize = 4;
    let num_purgers: usize = 2;
    let barrier = Arc::new(std::sync::Barrier::new(num_inserters + num_purgers));

    // Pre-populate entries expiring at time 100
    for i in 0..10usize {
        let key = DedupeKey::parse(&format!("rq-pre-pop-{i}-b2uv")).unwrap();
        let iid = InstanceId::from_bytes([i as u8; 16]);
        store.check_and_insert(&key, &iid, 100).unwrap();
    }

    let inserter_handles: Vec<_> = (0..num_inserters)
        .map(|t| {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let mut admitted = 0u64;
                let mut dupes = 0u64;
                for i in 0..10usize {
                    let key = DedupeKey::parse(&format!("rq-pre-pop-{i}-b2uv")).unwrap();
                    let iid = InstanceId::from_bytes([(t as u8).wrapping_add(i as u8); 16]);
                    match store.check_and_insert(&key, &iid, 60_000).unwrap() {
                        AdmissionResult::Admitted => admitted += 1,
                        AdmissionResult::Duplicate { .. } => dupes += 1,
                    }
                }
                (admitted, dupes)
            })
        })
        .collect();

    let purger_handles: Vec<_> = (0..num_purgers)
        .map(|_| {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                store.purge_expired(100).unwrap()
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

    let total_purged: u64 = purger_results.iter().sum();
    assert!(
        total_purged > 0,
        "At least some entries should have been purged"
    );

    let total_ops: u64 = inserter_results.iter().map(|(a, d)| a + d).sum();
    assert_eq!(
        total_ops,
        (num_inserters * 10) as u64,
        "Every insert attempt must yield a result"
    );
}
