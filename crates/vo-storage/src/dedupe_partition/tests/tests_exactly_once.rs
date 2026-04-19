#![allow(clippy::unwrap_used)]
//! Exactly-once delivery guarantee tests (ve-cg5hx).
//!
//! End-to-end tests verifying operations are executed exactly once despite
//! retries, network partitions, and process crashes. Uses the deterministic
//! test harness for time-controlled scenarios and InMemoryDedupeStore for
//! concurrent/realistic scenarios.

use super::*;
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

use crate::dedupe_partition::InMemoryDedupeStore;

fn iid(n: u8) -> InstanceId {
    InstanceId::from_bytes([n; 16])
}

fn key(name: &str) -> DedupeKey {
    DedupeKey::parse(name).unwrap()
}

// ---------------------------------------------------------------------------
// Deterministic store for time-controlled exactly-once tests
// ---------------------------------------------------------------------------

struct DeterministicStore {
    entries: std::cell::RefCell<HashMap<String, DedupeEntry>>,
    now_ms: Cell<u64>,
}

impl DeterministicStore {
    fn new() -> Self {
        Self {
            entries: std::cell::RefCell::new(HashMap::new()),
            now_ms: Cell::new(0),
        }
    }

    fn set_time(&self, t: u64) {
        self.now_ms.set(t);
    }
}

impl DedupeStore for DeterministicStore {
    fn check_and_insert(
        &self,
        k: &DedupeKey,
        instance_id: &InstanceId,
        ttl_ms: u64,
    ) -> Result<AdmissionResult, DedupeStoreError> {
        if ttl_ms == 0 {
            return Err(DedupeStoreError::InvalidArgument);
        }
        let key_str = k.as_str().to_string();
        let now = self.now_ms.get();
        {
            let entries = self.entries.borrow();
            if let Some(existing) = entries.get(&key_str) {
                if !existing.is_expired(now) {
                    return Ok(AdmissionResult::Duplicate {
                        instance_id: existing.instance_id().to_string(),
                    });
                }
            }
        }
        let expires_at = now.saturating_add(ttl_ms);
        let entry = DedupeEntry::new(key_str.clone(), instance_id.to_string(), expires_at)?;
        self.entries.borrow_mut().insert(key_str, entry);
        Ok(AdmissionResult::Admitted)
    }

    fn purge_expired(&self, now_ms: u64) -> Result<u64, DedupeStoreError> {
        let mut entries = self.entries.borrow_mut();
        let before = entries.len();
        entries.retain(|_, v| !v.is_expired(now_ms));
        Ok((before - entries.len()) as u64)
    }

    fn contains(&self, k: &DedupeKey) -> Result<bool, DedupeStoreError> {
        let entries = self.entries.borrow();
        let now = self.now_ms.get();
        Ok(entries.get(k.as_str()).is_some_and(|e| !e.is_expired(now)))
    }
}

// ---------------------------------------------------------------------------
// 1. Duplicate rejection: same key always returns Duplicate
// ---------------------------------------------------------------------------

#[test]
fn exactly_once_duplicate_rejected_on_retry() {
    let store = DeterministicStore::new();
    let k = key("order-12345");

    // First submission: admitted
    let r1 = store.check_and_insert(&k, &iid(1), 60_000).unwrap();
    assert_eq!(r1, AdmissionResult::Admitted);

    // Retry with same key: duplicate
    let r2 = store.check_and_insert(&k, &iid(2), 60_000).unwrap();
    assert!(matches!(r2, AdmissionResult::Duplicate { .. }));
}

#[test]
fn exactly_once_duplicate_returns_original_instance_id() {
    let store = DeterministicStore::new();
    let k = key("order-67890");

    store.check_and_insert(&k, &iid(0xAA), 60_000).unwrap();

    // Retry returns the ORIGINAL instance_id, not the retry's
    let r = store.check_and_insert(&k, &iid(0xBB), 60_000).unwrap();
    match r {
        AdmissionResult::Duplicate { instance_id } => {
            assert_eq!(instance_id, iid(0xAA).to_string());
            assert_ne!(instance_id, iid(0xBB).to_string());
        }
        AdmissionResult::Admitted => panic!("should be duplicate"),
    }
}

#[test]
fn exactly_once_multiple_retries_all_rejected() {
    let store = DeterministicStore::new();
    let k = key("tx-abc");

    store.check_and_insert(&k, &iid(1), 60_000).unwrap();

    // 10 retries, all must be rejected
    for i in 2..=11 {
        let r = store.check_and_insert(&k, &iid(i), 60_000).unwrap();
        assert!(
            matches!(r, AdmissionResult::Duplicate { .. }),
            "retry {i} should be duplicate"
        );
    }

    // Still contains the original
    assert!(store.contains(&k).unwrap());
}

// ---------------------------------------------------------------------------
// 2. Different keys are independent (per-key exactly-once)
// ---------------------------------------------------------------------------

#[test]
fn exactly_once_per_key_independence() {
    let store = DeterministicStore::new();
    let k1 = key("order-A");
    let k2 = key("order-B");
    let k3 = key("order-C");

    // Three different operations, all admitted
    assert_eq!(
        store.check_and_insert(&k1, &iid(1), 60_000).unwrap(),
        AdmissionResult::Admitted
    );
    assert_eq!(
        store.check_and_insert(&k2, &iid(2), 60_000).unwrap(),
        AdmissionResult::Admitted
    );
    assert_eq!(
        store.check_and_insert(&k3, &iid(3), 60_000).unwrap(),
        AdmissionResult::Admitted
    );

    // Retry k1: duplicate (k2 and k3 unaffected)
    assert!(matches!(
        store.check_and_insert(&k1, &iid(4), 60_000).unwrap(),
        AdmissionResult::Duplicate { .. }
    ));
    // k2 can still be retried (rejected)
    assert!(matches!(
        store.check_and_insert(&k2, &iid(5), 60_000).unwrap(),
        AdmissionResult::Duplicate { .. }
    ));
    // New key k4: admitted
    let k4 = key("order-D");
    assert_eq!(
        store.check_and_insert(&k4, &iid(6), 60_000).unwrap(),
        AdmissionResult::Admitted
    );
}

// ---------------------------------------------------------------------------
// 3. Network partition: two independent stores, partition then heal
// ---------------------------------------------------------------------------

#[test]
fn exactly_once_network_partition_stores_diverge_then_converge() {
    // Simulates two data center replicas with a network partition.
    // Each has its own store. During partition, both accept writes independently.
    // After partition heals, a client must handle divergence at the app layer.

    let store_a = DeterministicStore::new();
    let store_b = DeterministicStore::new();
    let k = key("global-order-1");

    // DC-A processes the order
    let r_a = store_a.check_and_insert(&k, &iid(1), 60_000).unwrap();
    assert_eq!(r_a, AdmissionResult::Admitted);

    // During partition, DC-B also receives the same order
    // (client retried against DC-B because DC-A was unreachable)
    let r_b = store_b.check_and_insert(&k, &iid(2), 60_000).unwrap();
    assert_eq!(r_b, AdmissionResult::Admitted);

    // Both stores independently enforce exactly-once for their own replica
    let r_a_retry = store_a.check_and_insert(&k, &iid(3), 60_000).unwrap();
    assert!(matches!(r_a_retry, AdmissionResult::Duplicate { .. }));

    let r_b_retry = store_b.check_and_insert(&k, &iid(4), 60_000).unwrap();
    assert!(matches!(r_b_retry, AdmissionResult::Duplicate { .. }));

    // The application layer detects divergence via instance_id mismatch
    // and reconciles (this is an application concern, not the store's)
}

#[test]
fn exactly_once_partition_isolated_still_enforces_locally() {
    // Single store in partition — retries still blocked
    let store = DeterministicStore::new();
    let k = key("partitioned-order");

    store.check_and_insert(&k, &iid(1), 60_000).unwrap();

    // Client retries many times during partition
    for _ in 0..50 {
        let r = store.check_and_insert(&k, &iid(2), 60_000).unwrap();
        assert!(matches!(r, AdmissionResult::Duplicate { .. }));
    }
}

// ---------------------------------------------------------------------------
// 4. Process crash recovery: state survives via persistence
// ---------------------------------------------------------------------------

#[test]
fn exactly_once_crash_recovery_in_memory_store() {
    // Simulates: process writes dedupe entry, crashes, restarts, retries
    let store = InMemoryDedupeStore::new();
    let k = key("crash-tx-1");

    // Before crash: operation recorded
    let r1 = store.check_and_insert(&k, &iid(1), 300_000).unwrap();
    assert_eq!(r1, AdmissionResult::Admitted);

    // Crash: process dies, but store state is "persisted" (in-memory simulates
    // what Fjall would do on disk). On restart, the client retries.

    let r2 = store.check_and_insert(&k, &iid(2), 300_000).unwrap();
    assert!(
        matches!(r2, AdmissionResult::Duplicate { .. }),
        "after crash recovery, retry must be rejected"
    );
}

#[test]
fn exactly_once_crash_before_response_client_retries() {
    // Simulates: store committed the entry but response was lost.
    // Client doesn't know if it succeeded and retries.
    let store = DeterministicStore::new();
    let k = key("crash-no-ack");

    // Server committed but crashed before sending response
    store.check_and_insert(&k, &iid(1), 60_000).unwrap();
    // (response lost — client doesn't know it succeeded)

    // Client retries with new instance_id — store correctly rejects
    let r = store.check_and_insert(&k, &iid(2), 60_000).unwrap();
    assert!(matches!(r, AdmissionResult::Duplicate { instance_id } if instance_id == iid(1).to_string()));
}

#[test]
fn exactly_once_crash_mid_batch_partial_commit() {
    // Simulates: batch of operations, crash after some committed
    let store = DeterministicStore::new();
    let keys: Vec<DedupeKey> = (0..5).map(|i| key(&format!("batch-{i}"))).collect();

    // First 3 operations committed before crash
    for i in 0..3 {
        let r = store.check_and_insert(&keys[i], &iid(i as u8), 60_000).unwrap();
        assert_eq!(r, AdmissionResult::Admitted);
    }
    // keys[3] and keys[4] never got committed

    // After restart, retry all 5:
    for i in 0..3 {
        let r = store.check_and_insert(&keys[i], &iid((i + 10) as u8), 60_000).unwrap();
        assert!(
            matches!(r, AdmissionResult::Duplicate { .. }),
            "key {} should be duplicate after crash",
            i
        );
    }
    for i in 3..5 {
        let r = store.check_and_insert(&keys[i], &iid(i as u8), 60_000).unwrap();
        assert_eq!(
            r,
            AdmissionResult::Admitted,
            "key {} should be admitted (never committed)",
            i
        );
    }
}

// ---------------------------------------------------------------------------
// 5. TTL expiry: expired entries allow re-admission
// ---------------------------------------------------------------------------

#[test]
fn exactly_once_expired_entry_allows_new_admission() {
    let store = DeterministicStore::new();
    let k = key("ttl-order");

    // Admit at time 0 with 1000ms TTL
    store.check_and_insert(&k, &iid(1), 1000).unwrap();
    assert!(store.contains(&k).unwrap());

    // Before expiry: duplicate
    store.set_time(500);
    let r = store.check_and_insert(&k, &iid(2), 1000).unwrap();
    assert!(matches!(r, AdmissionResult::Duplicate { .. }));

    // After expiry: new admission allowed
    store.set_time(1000);
    let r = store.check_and_insert(&k, &iid(3), 1000).unwrap();
    assert_eq!(r, AdmissionResult::Admitted);
}

#[test]
fn exactly_once_expiry_boundary_precise() {
    let store = DeterministicStore::new();
    let k = key("boundary-order");

    // Entry expires at exactly 500ms
    store.check_and_insert(&k, &iid(1), 500).unwrap();

    // At 499ms: still active
    store.set_time(499);
    assert!(store.contains(&k).unwrap());
    assert!(matches!(
        store.check_and_insert(&k, &iid(2), 500).unwrap(),
        AdmissionResult::Duplicate { .. }
    ));

    // At 500ms: expired (is_expired uses >=)
    store.set_time(500);
    assert!(!store.contains(&k).unwrap());
    let r = store.check_and_insert(&k, &iid(3), 500).unwrap();
    assert_eq!(r, AdmissionResult::Admitted);
}

// ---------------------------------------------------------------------------
// 6. Concurrent exactly-once: only one thread wins
// ---------------------------------------------------------------------------

#[test]
fn exactly_once_concurrent_only_one_admitted() {
    let store = Arc::new(InMemoryDedupeStore::new());
    let k = Arc::new(key("concurrent-order"));
    let num_threads = 8;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let store = Arc::clone(&store);
            let k = Arc::clone(&k);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let iid = InstanceId::from_bytes([i as u8; 16]);
                store.check_and_insert(&k, &iid, 60_000)
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let admitted = results.iter().filter(|r| matches!(r, Ok(AdmissionResult::Admitted))).count();
    let duplicates = results
        .iter()
        .filter(|r| matches!(r, Ok(AdmissionResult::Duplicate { .. })))
        .count();

    assert_eq!(admitted, 1, "exactly one thread must win admission");
    assert_eq!(duplicates, num_threads - 1);
}

// ---------------------------------------------------------------------------
// 7. End-to-end: simulate a full delivery lifecycle with crash + retry
// ---------------------------------------------------------------------------

#[test]
fn exactly_once_e2e_lifecycle_with_crash_and_retry() {
    let store = DeterministicStore::new();
    let k = key("e2e-order-lifecycle");

    // Phase 1: Initial submission at T=0
    store.set_time(0);
    let r1 = store.check_and_insert(&k, &iid(1), 10_000).unwrap();
    assert_eq!(r1, AdmissionResult::Admitted);

    // Phase 2: Network timeout — client retries at T=100
    store.set_time(100);
    let r2 = store.check_and_insert(&k, &iid(2), 10_000).unwrap();
    assert!(matches!(r2, AdmissionResult::Duplicate { .. }));

    // Phase 3: Process crash at T=5000 — restart, client retries at T=5500
    store.set_time(5500);
    let r3 = store.check_and_insert(&k, &iid(3), 10_000).unwrap();
    assert!(matches!(r3, AdmissionResult::Duplicate { .. }));

    // Phase 4: TTL expires at T=10000 — new submission allowed
    store.set_time(10_000);
    let r4 = store.check_and_insert(&k, &iid(4), 10_000).unwrap();
    assert_eq!(r4, AdmissionResult::Admitted);

    // Phase 5: New submission also rejects duplicates
    let r5 = store.check_and_insert(&k, &iid(5), 10_000).unwrap();
    assert!(matches!(r5, AdmissionResult::Duplicate { .. }));

    // Phase 6: Purge expired — old entry already replaced
    let purged = store.purge_expired(20_000).unwrap();
    assert_eq!(purged, 1, "the re-admitted entry should be expired");
    assert!(!store.contains(&k).unwrap());
}

// ---------------------------------------------------------------------------
// 8. Power loss: store recovered from persistent state
// ---------------------------------------------------------------------------

#[test]
fn exactly_once_power_loss_rehydrated_state_rejects_replay() {
    // Simulate: entries serialized to bytes, then deserialized into new store
    // after power loss

    let original = DeterministicStore::new();
    let k = key("power-loss-key");

    original.check_and_insert(&k, &iid(0x42), 60_000).unwrap();

    // Serialize entries (simulating Fjall persistence)
    let serialized: Vec<(String, Vec<u8>)> = {
        let entries = original.entries.borrow();
        entries
            .iter()
            .map(|(k, v)| (k.clone(), super::super::encode_dedupe_entry(v).unwrap()))
            .collect()
    };

    // Power loss — create new store and hydrate from serialized state
    let recovered = DeterministicStore::new();
    for (key_str, bytes) in &serialized {
        let entry = super::super::decode_dedupe_entry(bytes).unwrap();
        recovered.entries.borrow_mut().insert(key_str.clone(), entry);
    }

    // Retried operation must be rejected
    let r = recovered.check_and_insert(&k, &iid(0x99), 60_000).unwrap();
    assert!(
        matches!(r, AdmissionResult::Duplicate { .. }),
        "rehydrated store must reject replayed operations"
    );

    // Contains also works
    assert!(recovered.contains(&k).unwrap());
}

// ---------------------------------------------------------------------------
// 9. Stress: many operations, all exactly-once
// ---------------------------------------------------------------------------

#[test]
fn exactly_once_100_operations_all_unique() {
    let store = DeterministicStore::new();

    for i in 0..100u8 {
        let k = key(&format!("stress-{i}"));
        let r = store.check_and_insert(&k, &iid(i), 60_000).unwrap();
        assert_eq!(r, AdmissionResult::Admitted, "operation {i} should be admitted");
    }

    // All retries rejected
    for i in 0..100u8 {
        let k = key(&format!("stress-{i}"));
        let r = store.check_and_insert(&k, &iid(i.wrapping_add(1)), 60_000).unwrap();
        assert!(
            matches!(r, AdmissionResult::Duplicate { .. }),
            "retry {i} must be duplicate"
        );
    }
}

#[test]
fn exactly_once_50_operations_half_expired_retried() {
    let store = DeterministicStore::new();

    // Insert 50 operations with short TTL
    for i in 0..50u8 {
        let k = key(&format!("expire-stress-{i}"));
        store.check_and_insert(&k, &iid(i), 1000).unwrap();
    }

    // At T=500: none expired, all duplicates
    store.set_time(500);
    for i in 0..50u8 {
        let k = key(&format!("expire-stress-{i}"));
        let r = store.check_and_insert(&k, &iid(i.wrapping_add(1)), 1000).unwrap();
        assert!(matches!(r, AdmissionResult::Duplicate { .. }));
    }

    // At T=2000: all expired, all re-admitted
    store.set_time(2000);
    for i in 0..50u8 {
        let k = key(&format!("expire-stress-{i}"));
        let r = store.check_and_insert(&k, &iid(i.wrapping_add(2)), 1000).unwrap();
        assert_eq!(r, AdmissionResult::Admitted);
    }
}
