//! Crash injection tests for effect journal exactly-once commit guarantees (ADR-030).
//!
//! These tests simulate crash scenarios between prepare and commit, verifying that
//! managed effects commit exactly once under crash injection.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::super::*;
use serde_json::json;
use vo_types::{EffectIntent, EffectKind};

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

fn create_keyspace() -> (tempfile::TempDir, fjall::Keyspace) {
    let dir = tempfile::tempdir().expect("tempdir");
    let ks = fjall::Config::new(dir.path()).open().expect("keyspace");
    (dir, ks)
}

// ========================================================================
// Crash Scenario 1: Prepare succeeds, crash before commit
// On recovery, the effect must still be in Prepared state and commitable
// ========================================================================

#[test]
fn crash_after_prepare_effect_remains_prepared_on_recovery() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.path().to_path_buf();
    let id = sample_instance_id();

    let effect_id;
    {
        let keyspace = fjall::Config::new(&dir_path).open().expect("keyspace");
        let journal = FjallEffectJournal::open(&keyspace).expect("journal");

        let record = EffectRecord::new(
            "fx-crash-before-commit".to_string(),
            EffectKind::HttpCall,
            json!({"url": "https://api.stripe.com/charge"}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();

        effect_id = journal.prepare(&id, record).expect("prepare");

        // Simulate crash: keyspace dropped without commit
    }

    // Recovery: reopen keyspace
    let keyspace = fjall::Config::new(&dir_path)
        .open()
        .expect("keyspace reopen");
    let journal = FjallEffectJournal::open(&keyspace).expect("journal reopen");

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending.len(), 1, "crashed effect must remain pending");
    assert_eq!(pending[0].intent_id(), "fx-crash-before-commit");

    // Effect must still be commitable after crash
    journal
        .commit(&effect_id)
        .expect("commit after crash recovery");

    let pending_after = journal.list_pending(&id).unwrap();
    assert!(
        pending_after.is_empty(),
        "committed effect must not appear pending"
    );
}

#[test]
fn crash_after_prepare_rollback_still_works_on_recovery() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.path().to_path_buf();
    let id = sample_instance_id();

    let effect_id;
    {
        let keyspace = fjall::Config::new(&dir_path).open().expect("keyspace");
        let journal = FjallEffectJournal::open(&keyspace).expect("journal");

        let record = EffectRecord::new(
            "fx-crash-rollback".to_string(),
            EffectKind::SqlQuery,
            json!({"query": "INSERT INTO t VALUES (1)"}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();

        effect_id = journal.prepare(&id, record).expect("prepare");
    }

    let keyspace = fjall::Config::new(&dir_path)
        .open()
        .expect("keyspace reopen");
    let journal = FjallEffectJournal::open(&keyspace).expect("journal reopen");

    journal
        .rollback(&effect_id)
        .expect("rollback after crash recovery");
    let pending = journal.list_pending(&id).unwrap();
    assert!(pending.is_empty());
}

// ========================================================================
// Crash Scenario 2: Commit succeeds, crash before next operation
// On recovery, re-commit must fail with AlreadyTerminal (exactly-once)
// ========================================================================

#[test]
fn crash_after_commit_re_commit_returns_already_terminal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.path().to_path_buf();
    let id = sample_instance_id();

    let effect_id;
    {
        let keyspace = fjall::Config::new(&dir_path).open().expect("keyspace");
        let journal = FjallEffectJournal::open(&keyspace).expect("journal");

        let record = EffectRecord::new(
            "fx-crash-after-commit".to_string(),
            EffectKind::HttpCall,
            json!({"url": "https://api.stripe.com"}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();

        effect_id = journal.prepare(&id, record).expect("prepare");
        journal.commit(&effect_id).expect("commit");

        // Crash after commit
    }

    let keyspace = fjall::Config::new(&dir_path)
        .open()
        .expect("keyspace reopen");
    let journal = FjallEffectJournal::open(&keyspace).expect("journal reopen");

    let result = journal.commit(&effect_id);
    assert!(
        matches!(result, Err(EffectJournalError::AlreadyTerminal { .. })),
        "re-commit after crash must return AlreadyTerminal"
    );

    let pending = journal.list_pending(&id).unwrap();
    assert!(
        pending.is_empty(),
        "already-committed effect must not be pending"
    );
}

#[test]
fn crash_after_commit_rollback_returns_already_terminal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.path().to_path_buf();
    let id = sample_instance_id();

    let effect_id;
    {
        let keyspace = fjall::Config::new(&dir_path).open().expect("keyspace");
        let journal = FjallEffectJournal::open(&keyspace).expect("journal");

        let record = EffectRecord::new(
            "fx-crash-after-commit-rb".to_string(),
            EffectKind::BlobWrite,
            json!({"key": "test"}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();

        effect_id = journal.prepare(&id, record).expect("prepare");
        journal.commit(&effect_id).expect("commit");
    }

    let keyspace = fjall::Config::new(&dir_path)
        .open()
        .expect("keyspace reopen");
    let journal = FjallEffectJournal::open(&keyspace).expect("journal reopen");

    let result = journal.rollback(&effect_id);
    assert!(
        matches!(result, Err(EffectJournalError::AlreadyTerminal { .. })),
        "rollback after committed crash must return AlreadyTerminal"
    );
}

// ========================================================================
// Crash Scenario 3: Rollback succeeds, crash before next operation
// On recovery, re-rollback must fail (exactly-once rollback)
// ========================================================================

#[test]
fn crash_after_rollback_re_rollback_returns_already_terminal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.path().to_path_buf();
    let id = sample_instance_id();

    let effect_id;
    {
        let keyspace = fjall::Config::new(&dir_path).open().expect("keyspace");
        let journal = FjallEffectJournal::open(&keyspace).expect("journal");

        let record = EffectRecord::new(
            "fx-crash-after-rollback".to_string(),
            EffectKind::HttpCall,
            json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();

        effect_id = journal.prepare(&id, record).expect("prepare");
        journal.rollback(&effect_id).expect("rollback");
    }

    let keyspace = fjall::Config::new(&dir_path)
        .open()
        .expect("keyspace reopen");
    let journal = FjallEffectJournal::open(&keyspace).expect("journal reopen");

    let result = journal.rollback(&effect_id);
    assert!(
        matches!(result, Err(EffectJournalError::AlreadyTerminal { .. })),
        "re-rollback after crash must return AlreadyTerminal"
    );

    let result2 = journal.commit(&effect_id);
    assert!(
        matches!(result2, Err(EffectJournalError::AlreadyTerminal { .. })),
        "commit after rolled-back crash must return AlreadyTerminal"
    );
}

// ========================================================================
// Crash Scenario 4: Multiple effects, crash mid-batch
// Recovery must find exactly the uncommitted effects
// ========================================================================

#[test]
fn crash_mid_batch_replays_uncommitted_effects() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.path().to_path_buf();
    let id = sample_instance_id();

    let committed_ids;
    {
        let keyspace = fjall::Config::new(&dir_path).open().expect("keyspace");
        let journal = FjallEffectJournal::open(&keyspace).expect("journal");

        let mut committed = Vec::new();
        for i in 0..10 {
            let record = EffectRecord::new(
                format!("fx-batch-crash-{i}"),
                EffectKind::HttpCall,
                json!({"i": i}),
                EffectIntent::Prepared,
                None,
            )
            .unwrap();

            let eid = journal.prepare(&id, record).unwrap();

            if i < 5 {
                journal.commit(&eid).unwrap();
                committed.push(eid);
            }
            // Effects 5-9: prepared but NOT committed (crash simulation)
        }

        committed_ids = committed;
    }

    let keyspace = fjall::Config::new(&dir_path)
        .open()
        .expect("keyspace reopen");
    let journal = FjallEffectJournal::open(&keyspace).expect("journal reopen");

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(
        pending.len(),
        5,
        "exactly 5 uncommitted effects after crash"
    );

    let intent_ids: Vec<&str> = pending.iter().map(|r| r.intent_id()).collect();
    for i in 5..10 {
        assert!(
            intent_ids.contains(&format!("fx-batch-crash-{i}").as_str()),
            "effect fx-batch-crash-{i} must be pending"
        );
    }

    // Verify committed effects cannot be re-committed
    for eid in &committed_ids {
        let result = journal.commit(eid);
        assert!(
            matches!(result, Err(EffectJournalError::AlreadyTerminal { .. })),
            "already-committed effect must not be re-committable"
        );
    }
}

// ========================================================================
// Crash Scenario 5: Idempotent prepare after crash
// Re-preparing the same effect must not duplicate
// ========================================================================

#[test]
fn crash_after_prepare_idempotent_reprepare_no_duplicate() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.path().to_path_buf();
    let id = sample_instance_id();

    {
        let keyspace = fjall::Config::new(&dir_path).open().expect("keyspace");
        let journal = FjallEffectJournal::open(&keyspace).expect("journal");

        let record = EffectRecord::new(
            "fx-idempotent-crash".to_string(),
            EffectKind::HttpCall,
            json!({"url": "https://api.example.com"}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();

        journal.prepare(&id, record).expect("first prepare");
        // Crash: engine retries by re-preparing
    }

    let keyspace = fjall::Config::new(&dir_path)
        .open()
        .expect("keyspace reopen");
    let journal = FjallEffectJournal::open(&keyspace).expect("journal reopen");

    let record2 = EffectRecord::new(
        "fx-idempotent-crash".to_string(),
        EffectKind::HttpCall,
        json!({"url": "https://api.example.com", "retry": true}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let eid = journal
        .prepare(&id, record2)
        .expect("idempotent re-prepare");
    assert_eq!(eid.as_str().contains("fx-idempotent-crash"), true);

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending.len(), 1, "idempotent re-prepare must not duplicate");
}

// ========================================================================
// Property: Exactly-once across multiple crash-recovery cycles
// ========================================================================

#[test]
fn exactly_once_across_multiple_crash_recovery_cycles() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.path().to_path_buf();
    let id = sample_instance_id();

    let effect_id = EffectId::new(&id, "fx-multi-cycle").unwrap();

    // Cycle 1: Prepare, crash
    {
        let keyspace = fjall::Config::new(&dir_path).open().expect("keyspace");
        let journal = FjallEffectJournal::open(&keyspace).expect("journal");

        let record = EffectRecord::new(
            "fx-multi-cycle".to_string(),
            EffectKind::HttpCall,
            json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();

        let eid = journal.prepare(&id, record).unwrap();
        assert_eq!(eid, effect_id);
    }

    // Cycle 2: Recover, attempt commit, crash
    {
        let keyspace = fjall::Config::new(&dir_path).open().expect("keyspace");
        let journal = FjallEffectJournal::open(&keyspace).expect("journal");

        let pending = journal.list_pending(&id).unwrap();
        assert_eq!(pending.len(), 1);

        journal.commit(&effect_id).expect("commit on recovery");
    }

    // Cycle 3: Recover, attempt re-commit (must fail)
    {
        let keyspace = fjall::Config::new(&dir_path).open().expect("keyspace");
        let journal = FjallEffectJournal::open(&keyspace).expect("journal");

        let result = journal.commit(&effect_id);
        assert!(matches!(
            result,
            Err(EffectJournalError::AlreadyTerminal { .. })
        ));

        let pending = journal.list_pending(&id).unwrap();
        assert!(pending.is_empty());

        let result_rb = journal.rollback(&effect_id);
        assert!(matches!(
            result_rb,
            Err(EffectJournalError::AlreadyTerminal { .. })
        ));
    }
}

// ========================================================================
// Property: Cross-instance crash isolation
// ========================================================================

#[test]
fn crash_isolation_between_instances() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.path().to_path_buf();
    let id1 = InstanceId::from_bytes([1u8; 16]);
    let id2 = InstanceId::from_bytes([2u8; 16]);

    {
        let keyspace = fjall::Config::new(&dir_path).open().expect("keyspace");
        let journal = FjallEffectJournal::open(&keyspace).expect("journal");

        let r1 = EffectRecord::new(
            "fx-iso-1".to_string(),
            EffectKind::HttpCall,
            json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let r2 = EffectRecord::new(
            "fx-iso-2".to_string(),
            EffectKind::SqlQuery,
            json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();

        let eid1 = journal.prepare(&id1, r1).unwrap();
        journal.commit(&eid1).unwrap();

        let eid2 = journal.prepare(&id2, r2).unwrap();
        // Instance 2: prepared but NOT committed (crash)
    }

    let keyspace = fjall::Config::new(&dir_path)
        .open()
        .expect("keyspace reopen");
    let journal = FjallEffectJournal::open(&keyspace).expect("journal reopen");

    let pending1 = journal.list_pending(&id1).unwrap();
    assert!(
        pending1.is_empty(),
        "instance 1 committed effect must not be pending"
    );

    let pending2 = journal.list_pending(&id2).unwrap();
    assert_eq!(
        pending2.len(),
        1,
        "instance 2 crashed effect must be pending"
    );
    assert_eq!(pending2[0].intent_id(), "fx-iso-2");
}
