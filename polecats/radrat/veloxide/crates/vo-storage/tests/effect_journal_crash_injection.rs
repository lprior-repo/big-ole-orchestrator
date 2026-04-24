//! Crash injection tests — managed effects commit exactly once under simulated crash
//! scenarios (ADR-030/ADR-034).
//!
//! These tests simulate process crashes by dropping keyspaces without graceful
//! shutdown, then verify recovery invariants.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use vo_storage::effect_journal::{EffectJournal, FjallEffectJournal};
use vo_types::{EffectIntent, EffectKind, InstanceId};

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

fn http_record(intent_id: &str) -> vo_types::EffectRecord {
    vo_types::EffectRecord::new(
        intent_id.to_string(),
        EffectKind::HttpCall,
        serde_json::json!({"url": "https://api.example.com"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap()
}

fn open_journal(dir: &std::path::Path) -> FjallEffectJournal {
    let keyspace = fjall::Config::new(dir).open().unwrap();
    FjallEffectJournal::open(&keyspace).unwrap()
}

#[test]
fn exactly_once_commit_survives_crash_between_prepare_and_commit() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();

    // Phase 1: prepare but crash before commit
    let eid = {
        let journal = open_journal(dir.path());
        journal
            .prepare(&id, http_record("fx-exactly-once"))
            .unwrap()
    };

    // Phase 2: reopen and commit
    {
        let journal = open_journal(dir.path());
        journal.commit(&eid).unwrap();
    }

    // Phase 3: verify idempotent — double commit fails
    {
        let journal = open_journal(dir.path());
        let result = journal.commit(&eid);
        assert!(
            result.is_err(),
            "double commit after crash must fail (already terminal)"
        );
    }
}

#[test]
fn rollback_after_crash_prevents_double_rollback() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();

    let eid = {
        let journal = open_journal(dir.path());
        journal.prepare(&id, http_record("fx-rb-crash")).unwrap()
        // Crash: no rollback
    };

    {
        let journal = open_journal(dir.path());
        journal.rollback(&eid).unwrap();
        let result = journal.rollback(&eid);
        assert!(
            result.is_err(),
            "double rollback after crash must fail (already terminal)"
        );
    }
}

#[test]
fn crash_between_multiple_prepares_recovers_all_as_pending() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();

    // Prepare 5 effects, crash before any commit
    {
        let journal = open_journal(dir.path());
        for i in 0..5 {
            journal
                .prepare(&id, http_record(&format!("fx-batch-{i}")))
                .unwrap();
        }
    }

    // Reopen: all 5 must be pending
    {
        let journal = open_journal(dir.path());
        let pending = journal.list_pending(&id).unwrap();
        assert_eq!(
            pending.len(),
            5,
            "all 5 effects must be recovered as pending"
        );

        // Commit all to complete the saga
        for record in &pending {
            let eid = vo_storage::effect_journal::EffectId::new(&id, record.intent_id()).unwrap();
            journal.commit(&eid).unwrap();
        }
        assert!(journal.list_pending(&id).unwrap().is_empty());
    }
}

#[test]
fn partial_commit_crash_recovers_uncommitted_and_preserves_committed() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();

    // Prepare 4, commit 2, crash
    {
        let journal = open_journal(dir.path());
        for i in 0..4 {
            journal
                .prepare(&id, http_record(&format!("fx-partial-{i}")))
                .unwrap();
        }
        for i in 0..2 {
            let eid =
                vo_storage::effect_journal::EffectId::new(&id, &format!("fx-partial-{i}")).unwrap();
            journal.commit(&eid).unwrap();
        }
    }

    // Reopen: 2 pending, 2 committed
    {
        let journal = open_journal(dir.path());
        let pending = journal.list_pending(&id).unwrap();
        assert_eq!(pending.len(), 2);

        // Re-committing already-committed must fail
        for i in 0..2 {
            let eid =
                vo_storage::effect_journal::EffectId::new(&id, &format!("fx-partial-{i}")).unwrap();
            assert!(journal.commit(&eid).is_err(), "re-commit must fail");
        }

        // Rollback remaining pending
        for record in &pending {
            let eid = vo_storage::effect_journal::EffectId::new(&id, record.intent_id()).unwrap();
            journal.rollback(&eid).unwrap();
        }
        assert!(journal.list_pending(&id).unwrap().is_empty());
    }
}
