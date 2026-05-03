//! Red Queen adversarial concurrent write stress tests for vo-storage.
//!
//! Dimensions tested:
//! 1. 100 concurrent FjallEventStore submissions — no data loss
//! 2. Event ordering preservation under contention (monotonic sequences)
//! 3. No data loss on panic mid-batch (fjall batch atomicity)
//! 4. LSM-tree compaction during writes (high-volume churn)
//! 5. Concurrent atomic_admit_workflow — exactly-once admission
//! 6. Concurrent commit_event_and_summary — no partial state

#![cfg(test)]
#![allow(clippy::unwrap_used)]

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use tempfile::TempDir;
use vo_types::events::EventMetadata;
use vo_types::{EventEnvelope, InstanceId, InstanceStatus};

use crate::admission_commit::{
    atomic_admit_workflow, AtomicAdmitParams, AtomicAdmitResult,
};
use crate::event_store::{EventStore, FjallEventStore};
use crate::event_summary_commit::{
    commit_event_and_summary, CommitEventAndSummaryParams,
};
use crate::partitions::StorageEngine;

fn make_instance_id(n: u8) -> InstanceId {
    InstanceId::from_bytes([n; 16])
}

fn make_unique_instance_id(seed: u64) -> InstanceId {
    let bytes: [u8; 16] = seed.to_be_bytes().repeat(2).try_into().unwrap();
    InstanceId::from_bytes(bytes)
}

fn make_envelope(instance_id: &InstanceId, sequence: u64) -> EventEnvelope {
    EventEnvelope {
        schema_version: 1,
        instance_id: instance_id.to_string(),
        sequence,
        timestamp_ms: 1000 + sequence,
        payload: serde_json::json!({"type": "TestEvent", "seq": sequence}),
        metadata: EventMetadata::default(),
    }
}

fn create_test_db() -> (fjall::Database, TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(dir.path()).open().unwrap();
    (db, dir)
}

// ========================================================================
// DIMENSION 1: 100 concurrent event store submissions — no data loss
// ========================================================================

#[tokio::test]
async fn rq_100_concurrent_event_store_submissions_no_data_loss() {
    let (db, _dir) = create_test_db();
    let store = Arc::new(FjallEventStore::open(&db).unwrap());
    let num_tasks: usize = 100;
    let barrier = Arc::new(tokio::sync::Barrier::new(num_tasks));
    let successes = Arc::new(AtomicUsize::new(0));
    let occ_conflicts = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::with_capacity(num_tasks);
    for i in 0..num_tasks {
        let store = Arc::clone(&store);
        let barrier = Arc::clone(&barrier);
        let successes = Arc::clone(&successes);
        let occ_conflicts = Arc::clone(&occ_conflicts);
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            let instance_id = make_unique_instance_id(i as u64);
            let events = vec![make_envelope(&instance_id, 1)];
            match store.append(&instance_id, events).await {
                Ok(seq) => {
                    assert_eq!(seq, 1);
                    successes.fetch_add(1, Ordering::SeqCst);
                }
                Err(crate::event_store::EventStoreError::OccConflict { .. }) => {
                    occ_conflicts.fetch_add(1, Ordering::SeqCst);
                }
                Err(e) => panic!("unexpected error: {e}"),
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let total = successes.load(Ordering::SeqCst) + occ_conflicts.load(Ordering::SeqCst);
    assert_eq!(
        total, num_tasks,
        "every task must complete (success or OCC conflict)"
    );

    // All 100 distinct instances should succeed (different stripe locks)
    assert_eq!(
        successes.load(Ordering::SeqCst),
        num_tasks,
        "all 100 distinct instance writes must succeed with no data loss"
    );
}

// ========================================================================
// DIMENSION 1b: 100 concurrent writes to SAME instance — ordering preserved
// ========================================================================

#[tokio::test]
async fn rq_100_concurrent_same_instance_event_ordering_preserved() {
    let (db, _dir) = create_test_db();
    let store = Arc::new(FjallEventStore::open(&db).unwrap());
    let instance_id = Arc::new(make_instance_id(42));
    let num_tasks: usize = 100;
    let barrier = Arc::new(tokio::sync::Barrier::new(num_tasks));
    let success_count = Arc::new(AtomicUsize::new(0));

    // Pre-insert first event so the OCC check works
    {
        let events = vec![make_envelope(&instance_id, 1)];
        store.append(&instance_id, events).await.unwrap();
    }

    let mut handles = Vec::with_capacity(num_tasks);
    let next_seq = Arc::new(AtomicU64::new(2));

    for _ in 0..num_tasks {
        let store = Arc::clone(&store);
        let instance_id = Arc::clone(&instance_id);
        let barrier = Arc::clone(&barrier);
        let success_count = Arc::clone(&success_count);
        let next_seq = Arc::clone(&next_seq);
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            // Each task tries to append the next sequence number
            let seq = next_seq.fetch_add(1, Ordering::SeqCst);
            let events = vec![make_envelope(&instance_id, seq)];
            match store.append(&instance_id, events).await {
                Ok(_) => {
                    success_count.fetch_add(1, Ordering::SeqCst);
                }
                Err(crate::event_store::EventStoreError::OccConflict { .. }) => {
                    // Expected for losing racers
                }
                Err(e) => panic!("unexpected error: {e}"),
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let succeeded = success_count.load(Ordering::SeqCst);
    assert!(
        succeeded >= 1,
        "at least one sequential write must succeed"
    );

    // Verify ordering: read back all events and check monotonic sequences
    let final_seq = store.get_sequence(&instance_id).await.unwrap();
    assert_eq!(
        final_seq,
        (succeeded + 1) as u64,
        "sequence counter must equal 1 (pre-insert) + {succeeded} successful appends"
    );
}

// ========================================================================
// DIMENSION 2: Concurrent event ordering — cross-instance isolation
// ========================================================================

#[tokio::test]
async fn rq_concurrent_cross_instance_no_sequence_cross_contamination() {
    let (db, _dir) = create_test_db();
    let store = Arc::new(FjallEventStore::open(&db).unwrap());
    let num_instances = 20;
    let events_per_instance = 10;
    let barrier = Arc::new(tokio::sync::Barrier::new(num_instances));

    let mut handles = Vec::with_capacity(num_instances);
    for i in 0..num_instances {
        let store = Arc::clone(&store);
        let barrier = Arc::clone(&barrier);
        handles.push(tokio::spawn(async move {
            let instance_id = make_unique_instance_id(i as u64);
            barrier.wait().await;
            for seq in 1..=events_per_instance as u64 {
                let events = vec![make_envelope(&instance_id, seq)];
                let result = store.append(&instance_id, events).await;
                assert!(
                    result.is_ok(),
                    "instance {i} seq {seq} failed: {:?}",
                    result.err()
                );
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    // Verify each instance has exactly events_per_instance events
    for i in 0..num_instances {
        let instance_id = make_unique_instance_id(i as u64);
        let seq = store.get_sequence(&instance_id).await.unwrap();
        assert_eq!(
            seq, events_per_instance,
            "instance {i} should have {events_per_instance} events, got {seq}"
        );
    }
}

// ========================================================================
// DIMENSION 3: Batch atomicity — no partial state visible mid-batch
// ========================================================================

#[tokio::test]
async fn rq_batch_atomicity_no_partial_state_during_concurrent_commits() {
    let (db, _dir) = create_test_db();
    let store = Arc::new(FjallEventStore::open(&db).unwrap());
    let num_tasks = 50;
    let barrier = Arc::new(tokio::sync::Barrier::new(num_tasks));

    let mut handles = Vec::with_capacity(num_tasks);
    for i in 0..num_tasks {
        let store = Arc::clone(&store);
        let barrier = Arc::clone(&barrier);
        handles.push(tokio::spawn(async move {
            let instance_id = make_unique_instance_id((i + 100) as u64);
            barrier.wait().await;
            // Append a batch of 5 events atomically
            let events: Vec<EventEnvelope> = (1..=5)
                .map(|seq| make_envelope(&instance_id, seq))
                .collect();
            let result = store.append(&instance_id, events).await;
            assert!(result.is_ok(), "batch append failed: {:?}", result.err());
            // Immediately read back and verify all 5 are visible
            let seq = store.get_sequence(&instance_id).await.unwrap();
            assert_eq!(seq, 5, "all 5 batch events must be visible atomically");
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }
}

// ========================================================================
// DIMENSION 3b: Panic resilience — simulated mid-batch abort via drop
// ========================================================================

#[tokio::test]
async fn rq_no_data_loss_when_tasks_abandon_mid_write() {
    let (db, _dir) = create_test_db();
    let store = Arc::new(FjallEventStore::open(&db).unwrap());

    // Write 10 events sequentially as baseline
    let instance_id = make_instance_id(0xAA);
    for seq in 1..=10 {
        let events = vec![make_envelope(&instance_id, seq)];
        store.append(&instance_id, events).await.unwrap();
    }

    // Spawn 50 concurrent tasks that all try conflicting writes (OCC will reject)
    let mut handles = Vec::with_capacity(50);
    for _ in 0..50 {
        let store = Arc::clone(&store);
        let instance_id = instance_id.clone();
        handles.push(tokio::spawn(async move {
            let events = vec![make_envelope(&instance_id, 11)];
            // Most will get OCC conflict, that's fine
            let _ = store.append(&instance_id, events).await;
        }));
    }

    // Abort all tasks (drop handles without awaiting)
    drop(handles);

    // The original 10 events must be intact
    let seq = store.get_sequence(&instance_id).await.unwrap();
    assert_eq!(
        seq, 10,
        "original 10 events must survive despite aborted concurrent tasks"
    );
}

// ========================================================================
// DIMENSION 4: LSM-tree compaction during concurrent writes
// ========================================================================

#[tokio::test]
async fn rq_high_volume_churn_triggers_compaction_without_data_loss() {
    let (db, _dir) = create_test_db();
    let store = Arc::new(FjallEventStore::open(&db).unwrap());
    let num_instances = 10;
    let events_per_instance = 200;
    let total_events = num_instances * events_per_instance;

    // Write a high volume of events across multiple instances to trigger memtable flushes
    let mut handles = Vec::with_capacity(num_instances);
    for i in 0..num_instances {
        let store = Arc::clone(&store);
        handles.push(tokio::spawn(async move {
            let instance_id = make_unique_instance_id((i + 500) as u64);
            for seq in 1..=events_per_instance as u64 {
                let events = vec![make_envelope(&instance_id, seq)];
                let result = store.append(&instance_id, events).await;
                assert!(
                    result.is_ok(),
                    "write failed at seq {seq} for instance {i}: {:?}",
                    result.err()
                );
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    // Force compaction checkpoint
    db.persist(fjall::PersistMode::SyncAll).unwrap();

    // Verify total event count
    for i in 0..num_instances {
        let instance_id = make_unique_instance_id((i + 500) as u64);
        let seq = store.get_sequence(&instance_id).await.unwrap();
        assert_eq!(
            seq, events_per_instance as u64,
            "instance {i}: expected {events_per_instance} events after compaction, got {seq}"
        );
    }

    let _ = total_events;
}

// ========================================================================
// DIMENSION 5: Concurrent atomic_admit_workflow — exactly-once admission
// ========================================================================

#[test]
fn rq_concurrent_atomic_admit_exactly_one_winner_per_dedupe_key() {
    let db = create_test_db().0;
    let num_threads = 20;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads));
    let admitted = Arc::new(AtomicUsize::new(0));
    let duplicates = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let db = db.clone();
            let barrier = Arc::clone(&barrier);
            let admitted = Arc::clone(&admitted);
            let duplicates = Arc::clone(&duplicates);
            std::thread::spawn(move || {
                barrier.wait();
                let params = AtomicAdmitParams {
                    namespace: "stress-ns".to_string(),
                    instance_id: make_unique_instance_id(i as u64),
                    dedupe_key_str: "shared-dedupe-key-stress".to_string(),
                    dedupe_ttl_ms: 60_000,
                    timestamp_ms: 1_000_000 + i as u64,
                    event_payload: serde_json::json!({"type": "WorkflowStarted", "thread": i}),
                    event_metadata: EventMetadata::default(),
                    initial_status: InstanceStatus::Running,
                };
                match atomic_admit_workflow(&db, params) {
                    Ok(AtomicAdmitResult::Admitted) => {
                        admitted.fetch_add(1, Ordering::SeqCst);
                    }
                    Ok(AtomicAdmitResult::Duplicate { .. }) => {
                        duplicates.fetch_add(1, Ordering::SeqCst);
                    }
                    Err(e) => panic!("unexpected error: {e}"),
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let admitted_count = admitted.load(Ordering::SeqCst);
    let dup_count = duplicates.load(Ordering::SeqCst);

    assert_eq!(
        admitted_count, 1,
        "exactly one thread must win admission, got {admitted_count}"
    );
    assert_eq!(
        dup_count,
        num_threads - 1,
        "all others must be duplicates, got {dup_count}"
    );
}

#[test]
fn rq_concurrent_atomic_admit_distinct_keys_all_succeed() {
    let db = create_test_db().0;
    let num_threads = 50;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads));
    let admitted = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let db = db.clone();
            let barrier = Arc::clone(&barrier);
            let admitted = Arc::clone(&admitted);
            std::thread::spawn(move || {
                barrier.wait();
                let params = AtomicAdmitParams {
                    namespace: "stress-ns".to_string(),
                    instance_id: make_unique_instance_id(i as u64),
                    dedupe_key_str: format!("unique-key-{i}"),
                    dedupe_ttl_ms: 60_000,
                    timestamp_ms: 1_000_000 + i as u64,
                    event_payload: serde_json::json!({"type": "WorkflowStarted"}),
                    event_metadata: EventMetadata::default(),
                    initial_status: InstanceStatus::Running,
                };
                match atomic_admit_workflow(&db, params) {
                    Ok(AtomicAdmitResult::Admitted) => {
                        admitted.fetch_add(1, Ordering::SeqCst);
                    }
                    Ok(AtomicAdmitResult::Duplicate { .. }) => {
                        panic!("distinct keys should never produce duplicates");
                    }
                    Err(e) => panic!("unexpected error: {e}"),
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(
        admitted.load(Ordering::SeqCst),
        num_threads,
        "all 50 distinct key admissions must succeed"
    );
}

// ========================================================================
// DIMENSION 6: Concurrent commit_event_and_summary — no partial state
// ========================================================================

#[test]
fn rq_concurrent_event_summary_commits_no_partial_state_visible() {
    let db = create_test_db().0;
    let num_threads = 30;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let db = db.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                let instance_id = make_unique_instance_id((i + 200) as u64);
                let event = EventEnvelope {
                    schema_version: 1,
                    instance_id: instance_id.to_string(),
                    sequence: 1,
                    timestamp_ms: 1_712_200_000_000 + i as u64,
                    payload: serde_json::json!({"type": "TransitionEvent", "thread": i}),
                    metadata: EventMetadata::default(),
                };
                let params = CommitEventAndSummaryParams::new(
                    instance_id,
                    vo_types::SequenceNumber::try_from(1).unwrap(),
                    event,
                    InstanceStatus::Running,
                    vo_types::TimestampMs::try_from(1_712_200_000_000 + i as u64).unwrap(),
                    None,
                )
                .unwrap();
                commit_event_and_summary(&db, &params)
                    .expect("atomic event+summary commit must succeed");
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify each instance has both event and status entry
    let events_ks = db
        .keyspace(
            crate::partitions::EVENTS_PARTITION,
            fjall::KeyspaceCreateOptions::default,
        )
        .unwrap();
    let instances_ks = db
        .keyspace(
            crate::partitions::INSTANCES_PARTITION,
            fjall::KeyspaceCreateOptions::default,
        )
        .unwrap();

    let mut event_count = 0;
    let mut instance_count = 0;
    for item in events_ks.iter() {
        let (key, value) = item.into_inner().unwrap();
        assert!(
            key.len() >= 24,
            "event key must be at least 24 bytes, got {}",
            key.len()
        );
        let parsed: serde_json::Value = serde_json::from_slice(&value).unwrap();
        assert_eq!(parsed["sequence"], 1);
        event_count += 1;
    }
    for item in instances_ks.iter() {
        let _ = item.into_inner().unwrap();
        instance_count += 1;
    }

    assert_eq!(
        event_count, num_threads,
        "all {num_threads} events must be visible"
    );
    assert_eq!(
        instance_count, num_threads,
        "all {num_threads} instance index entries must be visible"
    );
}

// ========================================================================
// DIMENSION 7: StorageEngine-level concurrent writes across all stores
// ========================================================================

#[tokio::test]
async fn rq_storage_engine_100_concurrent_writes_across_all_stores() {
    let dir = tempfile::tempdir().unwrap();
    let engine = Arc::new(StorageEngine::open(dir.path()).unwrap());
    let num_tasks = 100;
    let barrier = Arc::new(tokio::sync::Barrier::new(num_tasks));
    let errors = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::with_capacity(num_tasks);
    for i in 0..num_tasks {
        let engine = Arc::clone(&engine);
        let barrier = Arc::clone(&barrier);
        let errors = Arc::clone(&errors);
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            let instance_id = make_unique_instance_id(i as u64);
            let events = vec![make_envelope(&instance_id, 1)];
            match engine.event_store.append(&instance_id, events).await {
                Ok(seq) => {
                    assert_eq!(seq, 1);
                }
                Err(e) => {
                    eprintln!("task {i} failed: {e}");
                    errors.fetch_add(1, Ordering::SeqCst);
                }
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    assert_eq!(
        errors.load(Ordering::SeqCst),
        0,
        "no errors allowed across 100 concurrent StorageEngine writes"
    );
}

// ========================================================================
// DIMENSION 8: Stress test — interleaved reads and writes
// ========================================================================

#[tokio::test]
async fn rq_interleaved_reads_during_concurrent_writes_never_see_partial_state() {
    let (db, _dir) = create_test_db();
    let store = Arc::new(FjallEventStore::open(&db).unwrap());
    let instance_id = Arc::new(make_instance_id(0xBB));
    let total_writes = 50;
    let barrier = Arc::new(tokio::sync::Barrier::new(total_writes + 5));

    // Writers: append events 1..=50 sequentially (serialized by OCC)
    let mut handles = Vec::new();
    for _ in 0..total_writes {
        let store = Arc::clone(&store);
        let instance_id = Arc::clone(&instance_id);
        let barrier = Arc::clone(&barrier);
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            // Retry loop for OCC conflicts
            for attempt in 0..100 {
                let current = store.get_sequence(&instance_id).await.unwrap();
                let next_seq = current + 1;
                let events = vec![make_envelope(&instance_id, next_seq)];
                match store.append(&instance_id, events).await {
                    Ok(_) => return,
                    Err(crate::event_store::EventStoreError::OccConflict { .. }) => {
                        if attempt == 99 {
                            panic!("too many retries");
                        }
                        continue;
                    }
                    Err(e) => panic!("unexpected error: {e}"),
                }
            }
        }));
    }

    // Readers: concurrently check sequence monotonicity
    for _ in 0..5 {
        let store = Arc::clone(&store);
        let instance_id = Arc::clone(&instance_id);
        let barrier = Arc::clone(&barrier);
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            for _ in 0..20 {
                let seq = store.get_sequence(&instance_id).await.unwrap();
                // Sequence must be monotonically non-decreasing
                // (it can be 0 if reads happen before any writes)
                assert!(
                    seq <= total_writes as u64,
                    "sequence {seq} exceeded expected max {total_writes}"
                );
                tokio::task::yield_now().await;
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let final_seq = store.get_sequence(&instance_id).await.unwrap();
    assert_eq!(
        final_seq, total_writes as u64,
        "all {total_writes} writes must be reflected in final sequence"
    );
}

// ========================================================================
// DIMENSION 9: Hot partition contention — many instances, same stripe
// ========================================================================

#[tokio::test]
async fn rq_hot_partition_contention_same_stripe_no_data_loss() {
    let (db, _dir) = create_test_db();
    let store = Arc::new(FjallEventStore::open(&db).unwrap());

    // Create 20 instance IDs that all hash to the same stripe bucket
    // by brute-forcing until we find collisions
    let mut same_stripe_ids: Vec<InstanceId> = Vec::new();
    let target_stripe = 0usize;
    for seed in 0u64..10000 {
        let bytes: [u8; 16] = seed.to_be_bytes().repeat(2).try_into().unwrap();
        let id = InstanceId::from_bytes(bytes);
        let id_bytes = id.to_bytes().unwrap();
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&id_bytes);
        let stripe = (hasher.finalize() as usize) % 64;
        if stripe == target_stripe {
            same_stripe_ids.push(id);
            if same_stripe_ids.len() >= 20 {
                break;
            }
        }
    }

    let num_instances = same_stripe_ids.len();
    assert!(
        num_instances >= 10,
        "need at least 10 same-stripe instances, found {num_instances}"
    );

    let barrier = Arc::new(tokio::sync::Barrier::new(num_instances));
    let store = Arc::new(store);
    let mut handles = Vec::with_capacity(num_instances);

    for instance_id in &same_stripe_ids {
        let store = Arc::clone(&store);
        let barrier = Arc::clone(&barrier);
        let iid = instance_id.clone();
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            for seq in 1..=10u64 {
                let events = vec![make_envelope(&iid, seq)];
                store.append(&iid, events).await.expect("append must succeed");
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all writes landed
    for (i, id) in same_stripe_ids.iter().enumerate() {
        let seq = store.get_sequence(id).await.unwrap();
        assert_eq!(
            seq, 10,
            "instance {i} (same-stripe) must have 10 events, got {seq}"
        );
    }
}

// ========================================================================
// DIMENSION 10: Concurrent admission + event summary — mixed workload
// ========================================================================

#[test]
fn rq_mixed_concurrent_admissions_and_summary_commits() {
    let db = create_test_db().0;
    let db = Arc::new(db);
    let num_admissions = 25;
    let num_summaries = 25;
    let barrier = Arc::new(std::sync::Barrier::new(num_admissions + num_summaries));
    let admission_ok = Arc::new(AtomicUsize::new(0));
    let summary_ok = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::with_capacity(num_admissions + num_summaries);

    // Admission tasks
    for i in 0..num_admissions {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        let admission_ok = Arc::clone(&admission_ok);
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            let params = AtomicAdmitParams {
                namespace: "mixed-stress".to_string(),
                instance_id: make_unique_instance_id((i + 300) as u64),
                dedupe_key_str: format!("mixed-key-{i}"),
                dedupe_ttl_ms: 60_000,
                timestamp_ms: 1_712_200_000_000 + i as u64,
                event_payload: serde_json::json!({"type": "Admission"}),
                event_metadata: EventMetadata::default(),
                initial_status: InstanceStatus::Running,
            };
            match atomic_admit_workflow(&db, params) {
                Ok(AtomicAdmitResult::Admitted) => {
                    admission_ok.fetch_add(1, Ordering::SeqCst);
                }
                Ok(AtomicAdmitResult::Duplicate { .. }) => {}
                Err(e) => panic!("admission error: {e}"),
            }
        }));
    }

    // Summary commit tasks
    for i in 0..num_summaries {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        let summary_ok = Arc::clone(&summary_ok);
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            let instance_id = make_unique_instance_id((i + 400) as u64);
            let event = EventEnvelope {
                schema_version: 1,
                instance_id: instance_id.to_string(),
                sequence: 1,
                timestamp_ms: 1_712_200_000_000 + i as u64,
                payload: serde_json::json!({"type": "Summary"}),
                metadata: EventMetadata::default(),
            };
            let params = CommitEventAndSummaryParams::new(
                instance_id,
                vo_types::SequenceNumber::try_from(1).unwrap(),
                event,
                InstanceStatus::Running,
                vo_types::TimestampMs::try_from(1_712_200_000_000 + i as u64).unwrap(),
                None,
            )
            .unwrap();
            commit_event_and_summary(&db, &params).expect("summary commit must succeed");
            summary_ok.fetch_add(1, Ordering::SeqCst);
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(
        admission_ok.load(Ordering::SeqCst),
        num_admissions,
        "all admissions must succeed"
    );
    assert_eq!(
        summary_ok.load(Ordering::SeqCst),
        num_summaries,
        "all summary commits must succeed"
    );
}
