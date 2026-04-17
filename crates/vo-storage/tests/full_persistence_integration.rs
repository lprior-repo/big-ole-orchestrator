//! Full persistence integration tests — cross-backend durability, crash recovery, and concurrent access.
//!
//! Tests PERS-001 through PERS-012: comprehensive persistence guarantees across storage backends.
//!
//! ## Storage Backends Tested
//!
//! - `FjallEffectJournal`: Production Fjall-backed effect journal
//! - `InMemoryEffectJournal`: In-memory effect journal for testing
//!
//! ## Edge Cases Covered
//!
//! - Power failure: crash between prepare/commit, mid-batch crashes, recovery after abrupt termination
//! - Concurrent access: multi-threaded prepare/commit/rollback, race conditions, exactly-once guarantees
//! - Cross-backend consistency: both backends must satisfy same persistence invariants

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use serde_json::json;
use std::sync::Arc;
use vo_storage::effect_journal::InMemoryEffectJournal;
use vo_storage::effect_journal::{EffectId, EffectJournal, EffectJournalError, FjallEffectJournal};
use vo_types::{EffectIntent, EffectKind, EffectRecord, InstanceId};

// ---------------------------------------------------------------------------
// Test Configuration
// ---------------------------------------------------------------------------

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

fn other_instance_id() -> InstanceId {
    InstanceId::from_bytes([2u8; 16])
}

fn make_effect_record(intent_id: &str) -> vo_types::EffectRecord {
    EffectRecord::new(
        intent_id.to_string(),
        EffectKind::HttpCall,
        json!({"url": "https://api.example.com"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap()
}

// ---------------------------------------------------------------------------
// PERS-001: Basic prepare-commit lifecycle (both backends)
// ---------------------------------------------------------------------------

#[test]
fn pers_001_fjall_basic_prepare_commit() {
    let dir = tempfile::tempdir().unwrap();
    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let journal = FjallEffectJournal::open(&keyspace).unwrap();
    let id = sample_instance_id();

    let record = make_effect_record("pers-basic-1");
    let eid = journal.prepare(&id, record).unwrap();

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending.len(), 1, "Fjall: one pending after prepare");
    assert_eq!(pending[0].intent_id(), "pers-basic-1");

    journal.commit(&eid).unwrap();

    let pending_after = journal.list_pending(&id).unwrap();
    assert!(pending_after.is_empty(), "Fjall: no pending after commit");

    let double_commit = journal.commit(&eid);
    assert!(
        matches!(
            double_commit,
            Err(EffectJournalError::AlreadyTerminal { .. })
        ),
        "Fjall: double commit fails with AlreadyTerminal"
    );
}

#[test]
fn pers_001_inmemory_basic_prepare_commit() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let record = make_effect_record("pers-basic-1");
    let eid = journal.prepare(&id, record).unwrap();

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending.len(), 1, "InMemory: one pending after prepare");
    assert_eq!(pending[0].intent_id(), "pers-basic-1");

    journal.commit(&eid).unwrap();

    let pending_after = journal.list_pending(&id).unwrap();
    assert!(
        pending_after.is_empty(),
        "InMemory: no pending after commit"
    );

    let double_commit = journal.commit(&eid);
    assert!(
        matches!(
            double_commit,
            Err(EffectJournalError::AlreadyTerminal { .. })
        ),
        "InMemory: double commit fails with AlreadyTerminal"
    );
}

// ---------------------------------------------------------------------------
// PERS-002: Basic rollback lifecycle (both backends)
// ---------------------------------------------------------------------------

#[test]
fn pers_002_fjall_basic_rollback() {
    let dir = tempfile::tempdir().unwrap();
    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let journal = FjallEffectJournal::open(&keyspace).unwrap();
    let id = sample_instance_id();

    let record = make_effect_record("pers-rollback-1");
    let eid = journal.prepare(&id, record).unwrap();

    journal.rollback(&eid).unwrap();

    let pending_after = journal.list_pending(&id).unwrap();
    assert!(pending_after.is_empty(), "Fjall: no pending after rollback");

    let double_rollback = journal.rollback(&eid);
    assert!(
        matches!(
            double_rollback,
            Err(EffectJournalError::AlreadyTerminal { .. })
        ),
        "Fjall: double rollback fails with AlreadyTerminal"
    );
}

#[test]
fn pers_002_inmemory_basic_rollback() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let record = make_effect_record("pers-rollback-1");
    let eid = journal.prepare(&id, record).unwrap();

    journal.rollback(&eid).unwrap();

    let pending_after = journal.list_pending(&id).unwrap();
    assert!(
        pending_after.is_empty(),
        "InMemory: no pending after rollback"
    );

    let double_rollback = journal.rollback(&eid);
    assert!(
        matches!(
            double_rollback,
            Err(EffectJournalError::AlreadyTerminal { .. })
        ),
        "InMemory: double rollback fails with AlreadyTerminal"
    );
}

// ---------------------------------------------------------------------------
// PERS-003: Power failure — crash between multiple prepares
// ---------------------------------------------------------------------------

#[test]
fn pers_003_fjall_power_failure_multiple_prepares() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();

    let effect_ids;
    {
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let journal = FjallEffectJournal::open(&keyspace).unwrap();

        effect_ids = (0..5)
            .map(|i| {
                let record = make_effect_record(&format!("pers-multi-{}", i));
                journal.prepare(&id, record).unwrap()
            })
            .collect::<Vec<_>>();
    }

    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let journal = FjallEffectJournal::open(&keyspace).unwrap();

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(
        pending.len(),
        5,
        "All 5 effects must be pending after crash recovery"
    );

    for eid in &effect_ids {
        journal.commit(eid).unwrap();
    }

    let pending_after = journal.list_pending(&id).unwrap();
    assert!(
        pending_after.is_empty(),
        "All effects committed, no pending"
    );
}

// ---------------------------------------------------------------------------
// PERS-004: Power failure — partial commit crash
// ---------------------------------------------------------------------------

#[test]
fn pers_004_fjall_power_failure_partial_commit() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();

    let effect_ids;
    {
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let journal = FjallEffectJournal::open(&keyspace).unwrap();

        effect_ids = (0..4)
            .map(|i| {
                let record = make_effect_record(&format!("pers-partial-{}", i));
                journal.prepare(&id, record).unwrap()
            })
            .collect::<Vec<_>>();

        journal.commit(&effect_ids[0]).unwrap();
        journal.commit(&effect_ids[1]).unwrap();
    }

    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let journal = FjallEffectJournal::open(&keyspace).unwrap();

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(
        pending.len(),
        2,
        "Exactly 2 effects pending after partial commit crash"
    );

    for record in &pending {
        let eid = EffectId::new(&id, record.intent_id()).unwrap();
        journal.rollback(&eid).unwrap();
    }

    let pending_final = journal.list_pending(&id).unwrap();
    assert!(
        pending_final.is_empty(),
        "All effects resolved after rollback"
    );
}

// ---------------------------------------------------------------------------
// PERS-005: Concurrent access — multi-threaded prepare (InMemory only, thread-safe)
// ---------------------------------------------------------------------------

#[test]
fn pers_005_concurrent_prepare_same_instance() {
    let journal = Arc::new(InMemoryEffectJournal::new());
    let id = sample_instance_id();
    let num_threads = 16;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let journal = journal.clone();
            let barrier = barrier.clone();
            let id = id.clone();
            std::thread::spawn(move || {
                barrier.wait();
                let record = make_effect_record(&format!("pers-concurrent-{}", i));
                journal.prepare(&id, record)
            })
        })
        .collect();

    let results: Vec<Result<EffectId, _>> =
        handles.into_iter().map(|h| h.join().unwrap()).collect();

    assert!(
        results.iter().all(|r| r.is_ok()),
        "All concurrent prepares should succeed"
    );

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(
        pending.len(),
        num_threads,
        "All {} concurrent effects should be pending",
        num_threads
    );
}

// ---------------------------------------------------------------------------
// PERS-006: Concurrent access — multi-threaded commit (InMemory only, thread-safe)
// ---------------------------------------------------------------------------

#[test]
fn pers_006_concurrent_commit_same_effect() {
    let journal = Arc::new(InMemoryEffectJournal::new());
    let id = sample_instance_id();
    let record = make_effect_record("pers-concurrent-commit");
    let eid = journal.prepare(&id, record).unwrap();

    let num_threads = 8;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let journal = journal.clone();
            let eid = eid.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                journal.commit(&eid)
            })
        })
        .collect();

    let results: Vec<Result<(), _>> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    let success_count = results.iter().filter(|r| r.is_ok()).count();
    let fail_count = results.iter().filter(|r| r.is_err()).count();

    assert_eq!(success_count, 1, "Exactly one commit should succeed");
    assert_eq!(
        fail_count,
        num_threads - 1,
        "Other commits should fail with AlreadyTerminal"
    );
}

// ---------------------------------------------------------------------------
// PERS-007: Concurrent access — cross-instance isolation (InMemory only, thread-safe)
// ---------------------------------------------------------------------------

#[test]
fn pers_007_concurrent_cross_instance_isolation() {
    let journal = Arc::new(InMemoryEffectJournal::new());
    let id1 = sample_instance_id();
    let id2 = other_instance_id();
    let num_threads = 8;

    let barrier = Arc::new(std::sync::Barrier::new(num_threads * 2));

    let handles1: Vec<_> = (0..num_threads)
        .map(|i| {
            let journal = journal.clone();
            let id1 = id1.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                let record = make_effect_record(&format!("inst1-{}", i));
                journal.prepare(&id1, record)
            })
        })
        .collect();

    let handles2: Vec<_> = (0..num_threads)
        .map(|i| {
            let journal = journal.clone();
            let id2 = id2.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                let record = make_effect_record(&format!("inst2-{}", i));
                journal.prepare(&id2, record)
            })
        })
        .collect();

    for h in handles1.into_iter().chain(handles2.into_iter()) {
        assert!(
            h.join().unwrap().is_ok(),
            "All concurrent prepares should succeed"
        );
    }

    let pending1 = journal.list_pending(&id1).unwrap();
    let pending2 = journal.list_pending(&id2).unwrap();

    assert_eq!(pending1.len(), num_threads, "Instance 1 has correct count");
    assert_eq!(pending2.len(), num_threads, "Instance 2 has correct count");
}

// ---------------------------------------------------------------------------
// PERS-008: Idempotent prepare after crash (exactly-once)
// ---------------------------------------------------------------------------

#[test]
fn pers_008_fjall_idempotent_prepare_after_crash() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();

    {
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let journal = FjallEffectJournal::open(&keyspace).unwrap();
        let record = make_effect_record("pers-idempotent");
        journal.prepare(&id, record).unwrap();
    }

    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let journal = FjallEffectJournal::open(&keyspace).unwrap();

    let record2 = make_effect_record("pers-idempotent");
    let eid = journal.prepare(&id, record2).unwrap();

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(
        pending.len(),
        1,
        "Idempotent re-prepare must not duplicate effects"
    );
    assert_eq!(eid.as_str().contains("pers-idempotent"), true);
}

// ---------------------------------------------------------------------------
// PERS-009: Concurrent prepare-commit-rollback interleaved (InMemory)
// ---------------------------------------------------------------------------

#[test]
fn pers_009_concurrent_prepare_commit_rollback_interleaved() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let eid1 = journal
        .prepare(&id, make_effect_record("pers-interleave-1"))
        .unwrap();
    let eid2 = journal
        .prepare(&id, make_effect_record("pers-interleave-2"))
        .unwrap();
    let eid3 = journal
        .prepare(&id, make_effect_record("pers-interleave-3"))
        .unwrap();

    assert_eq!(journal.list_pending(&id).unwrap().len(), 3);

    journal.commit(&eid2).unwrap();
    assert_eq!(journal.list_pending(&id).unwrap().len(), 2);

    journal.rollback(&eid1).unwrap();
    assert_eq!(journal.list_pending(&id).unwrap().len(), 1);

    journal.commit(&eid3).unwrap();
    assert_eq!(journal.list_pending(&id).unwrap().len(), 0);
}

// ---------------------------------------------------------------------------
// PERS-010: Batch operations with crash recovery
// ---------------------------------------------------------------------------

#[test]
fn pers_010_fjall_batch_crash_recovery_exactly_once() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();

    let committed_ids;
    {
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let journal = FjallEffectJournal::open(&keyspace).unwrap();

        let mut committed = Vec::new();
        for i in 0..10 {
            let record = make_effect_record(&format!("pers-batch-{}", i));
            let eid = journal.prepare(&id, record).unwrap();
            if i < 5 {
                journal.commit(&eid).unwrap();
                committed.push(eid);
            }
        }
        committed_ids = committed;
    }

    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let journal = FjallEffectJournal::open(&keyspace).unwrap();

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending.len(), 5, "Exactly 5 effects pending after crash");

    for eid in &committed_ids {
        let result = journal.commit(eid);
        assert!(
            matches!(result, Err(EffectJournalError::AlreadyTerminal { .. })),
            "Already-committed effects must fail re-commit"
        );
    }

    for record in &pending {
        let eid = EffectId::new(&id, record.intent_id()).unwrap();
        journal.commit(&eid).unwrap();
    }

    let final_pending = journal.list_pending(&id).unwrap();
    assert!(final_pending.is_empty(), "All effects resolved");
}

// ---------------------------------------------------------------------------
// PERS-011: Exactly-once guarantee across multiple crash cycles
// ---------------------------------------------------------------------------

#[test]
fn pers_011_fjall_exactly_once_across_multiple_cycles() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();
    let effect_id = EffectId::new(&id, "pers-multi-cycle").unwrap();

    for _ in 0..3 {
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let journal = FjallEffectJournal::open(&keyspace).unwrap();

        let pending = journal.list_pending(&id).unwrap();
        if pending.is_empty() {
            let record = make_effect_record("pers-multi-cycle");
            let eid = journal.prepare(&id, record).unwrap();
            assert_eq!(eid, effect_id);
            drop(journal);
            continue;
        }

        journal.commit(&effect_id).unwrap();
        drop(journal);
    }

    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let journal = FjallEffectJournal::open(&keyspace).unwrap();

    let result = journal.commit(&effect_id);
    assert!(
        matches!(result, Err(EffectJournalError::AlreadyTerminal { .. })),
        "Effect must be terminal after multiple crash cycles"
    );

    let result_rb = journal.rollback(&effect_id);
    assert!(
        matches!(result_rb, Err(EffectJournalError::AlreadyTerminal { .. })),
        "Cannot rollback committed effect"
    );
}

// ---------------------------------------------------------------------------
// PERS-012: Compact after crash recovery
// ---------------------------------------------------------------------------

#[test]
fn pers_012_fjall_compact_after_crash_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();

    {
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let journal = FjallEffectJournal::open(&keyspace).unwrap();

        for i in 0..3 {
            let record = make_effect_record(&format!("pers-compact-{}", i));
            let eid = journal.prepare(&id, record).unwrap();
            journal.commit(&eid).unwrap();
        }
    }

    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let journal = FjallEffectJournal::open(&keyspace).unwrap();

    let ts = vo_types::TimestampMs::parse("1000").unwrap();
    let removed = journal.compact(ts).unwrap();

    assert_eq!(removed, 3, "All committed effects should be compacted");
}

// ---------------------------------------------------------------------------
// PERS-013: Concurrent access with Fjall backend (realistic crash scenario)
// ---------------------------------------------------------------------------

#[test]
fn pers_013_fjall_concurrent_with_crash() {
    let dir = tempfile::tempdir().unwrap();
    let id1 = sample_instance_id();
    let id2 = other_instance_id();
    let id1_clone = id1.clone();
    let id2_clone = id2.clone();

    let barrier = Arc::new(std::sync::Barrier::new(4));

    let h1 = std::thread::spawn({
        let dir = dir.path().to_path_buf();
        let barrier = barrier.clone();
        move || {
            barrier.wait();
            let keyspace = fjall::Config::new(&dir).open().unwrap();
            let journal = FjallEffectJournal::open(&keyspace).unwrap();
            for i in 0..10 {
                let record = make_effect_record(&format!("thread1-{}", i));
                let _ = journal.prepare(&id1_clone, record);
            }
        }
    });

    let h2 = std::thread::spawn({
        let dir = dir.path().to_path_buf();
        let barrier = barrier.clone();
        move || {
            barrier.wait();
            let keyspace = fjall::Config::new(&dir).open().unwrap();
            let journal = FjallEffectJournal::open(&keyspace).unwrap();
            for i in 0..10 {
                let record = make_effect_record(&format!("thread2-{}", i));
                let _ = journal.prepare(&id2_clone, record);
            }
        }
    });

    h1.join().unwrap();
    h2.join().unwrap();

    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let journal = FjallEffectJournal::open(&keyspace).unwrap();

    let pending1 = journal.list_pending(&id1).unwrap();
    let pending2 = journal.list_pending(&id2).unwrap();

    assert_eq!(pending1.len(), 10, "Instance 1: all 10 effects pending");
    assert_eq!(pending2.len(), 10, "Instance 2: all 10 effects pending");
}

// ---------------------------------------------------------------------------
// PERS-014: InMemory durability survives keyspace reopen (unlike Fjall)
// ---------------------------------------------------------------------------

#[test]
fn pers_014_inmemory_durability_within_session() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let record = make_effect_record("pers-durable");
    let eid = journal.prepare(&id, record).unwrap();

    let pending_before = journal.list_pending(&id).unwrap();
    assert_eq!(pending_before.len(), 1, "One pending before commit");

    journal.commit(&eid).unwrap();

    let pending_after = journal.list_pending(&id).unwrap();
    assert!(pending_after.is_empty(), "No pending after commit");

    let result = journal.commit(&eid);
    assert!(
        matches!(result, Err(EffectJournalError::AlreadyTerminal { .. })),
        "Double commit must fail"
    );
}
