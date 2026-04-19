//! Effect journal write durability, read-after-write consistency, and replay tests (ADR-030).

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::super::*;
use serde_json::json;
use vo_types::{EffectIntent, EffectKind};

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

fn create_keyspace() -> (tempfile::TempDir, fjall::Database) {
    let dir = tempfile::tempdir().expect("tempdir");
    let ks = fjall::Database::builder(dir.path())
        .open()
        .expect("keyspace");
    (dir, ks)
}

// ========================================================================
// Write Durability — Fjall-backed persistence survives flush
// ========================================================================

#[test]
fn fjall_prepare_survives_keyspace_flush() {
    let (_dir, keyspace) = create_keyspace();
    let journal = FjallEffectJournal::open(&keyspace).unwrap();
    let id = sample_instance_id();

    let record = EffectRecord::new(
        "fx-durability-1".to_string(),
        EffectKind::HttpCall,
        json!({"url": "https://api.stripe.com/v1/charges"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let eid = journal.prepare(&id, record).unwrap();

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].intent_id(), "fx-durability-1");

    let commit_result = journal.commit(&eid);
    assert_eq!(commit_result, Ok(()));
}

#[test]
fn fjall_commit_survives_keyspace_flush() {
    let (_dir, keyspace) = create_keyspace();
    let journal = FjallEffectJournal::open(&keyspace).unwrap();
    let id = sample_instance_id();

    let record = EffectRecord::new(
        "fx-durability-2".to_string(),
        EffectKind::SqlQuery,
        json!({"query": "INSERT INTO payments VALUES (1)"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let eid = journal.prepare(&id, record).unwrap();
    journal.commit(&eid).unwrap();

    let pending = journal.list_pending(&id).unwrap();
    assert!(
        pending.is_empty(),
        "committed effect must not appear in pending after flush"
    );

    let second_commit = journal.commit(&eid);
    assert!(matches!(
        second_commit,
        Err(EffectJournalError::AlreadyTerminal { .. })
    ));
}

#[test]
fn fjall_rollback_survives_keyspace_flush() {
    let (_dir, keyspace) = create_keyspace();
    let journal = FjallEffectJournal::open(&keyspace).unwrap();
    let id = sample_instance_id();

    let record = EffectRecord::new(
        "fx-durability-3".to_string(),
        EffectKind::BlobWrite,
        json!({"bucket": "s3://data", "key": "obj"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let eid = journal.prepare(&id, record).unwrap();
    journal.rollback(&eid).unwrap();

    let pending = journal.list_pending(&id).unwrap();
    assert!(pending.is_empty());

    let second_rollback = journal.rollback(&eid);
    assert!(matches!(
        second_rollback,
        Err(EffectJournalError::AlreadyTerminal { .. })
    ));
}

#[test]
fn fjall_multiple_effects_durability_across_flush() {
    let (_dir, keyspace) = create_keyspace();
    let journal = FjallEffectJournal::open(&keyspace).unwrap();
    let id = sample_instance_id();

    for i in 0..10 {
        let record = EffectRecord::new(
            format!("fx-batch-{i}"),
            EffectKind::HttpCall,
            json!({"i": i}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        journal.prepare(&id, record).unwrap();
    }

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending.len(), 10);

    for (idx, p) in pending.iter().enumerate() {
        assert_eq!(p.intent_id(), format!("fx-batch-{idx}"));
    }
}

// ========================================================================
// Read-After-Write Consistency
// ========================================================================

#[test]
fn fjall_read_after_write_consistency_single_effect() {
    let (_dir, keyspace) = create_keyspace();
    let journal = FjallEffectJournal::open(&keyspace).unwrap();
    let id = sample_instance_id();

    let record = EffectRecord::new(
        "fx-raw-1".to_string(),
        EffectKind::HttpCall,
        json!({"url": "https://example.com/api"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let eid = journal.prepare(&id, record.clone()).unwrap();

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].intent_id(), record.intent_id());
    assert_eq!(pending[0].kind(), record.kind());
    assert_eq!(pending[0].status(), EffectIntent::Prepared);
    assert_eq!(eid.as_str().contains("fx-raw-1"), true);
}

#[test]
fn fjall_read_after_write_consistency_after_commit() {
    let (_dir, keyspace) = create_keyspace();
    let journal = FjallEffectJournal::open(&keyspace).unwrap();
    let id = sample_instance_id();

    let record = EffectRecord::new(
        "fx-raw-2".to_string(),
        EffectKind::SqlQuery,
        json!({"sql": "SELECT 1"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let eid = journal.prepare(&id, record).unwrap();
    journal.commit(&eid).unwrap();

    let pending = journal.list_pending(&id).unwrap();
    assert!(
        pending.is_empty(),
        "read-after-write: committed effect must not appear in pending"
    );
}

#[test]
fn fjall_read_after_write_consistency_after_rollback() {
    let (_dir, keyspace) = create_keyspace();
    let journal = FjallEffectJournal::open(&keyspace).unwrap();
    let id = sample_instance_id();

    let record = EffectRecord::new(
        "fx-raw-3".to_string(),
        EffectKind::BlobWrite,
        json!({"key": "test"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let eid = journal.prepare(&id, record).unwrap();
    journal.rollback(&eid).unwrap();

    let pending = journal.list_pending(&id).unwrap();
    assert!(
        pending.is_empty(),
        "read-after-write: rolled back effect must not appear in pending"
    );
}

#[test]
fn fjall_read_after_write_interleaved_operations() {
    let (_dir, keyspace) = create_keyspace();
    let journal = FjallEffectJournal::open(&keyspace).unwrap();
    let id = sample_instance_id();

    let r1 = EffectRecord::new(
        "fx-interleave-1".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let r2 = EffectRecord::new(
        "fx-interleave-2".to_string(),
        EffectKind::SqlQuery,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let r3 = EffectRecord::new(
        "fx-interleave-3".to_string(),
        EffectKind::BlobWrite,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let eid1 = journal.prepare(&id, r1).unwrap();
    let eid2 = journal.prepare(&id, r2).unwrap();
    let eid3 = journal.prepare(&id, r3).unwrap();

    assert_eq!(journal.list_pending(&id).unwrap().len(), 3);

    journal.commit(&eid2).unwrap();
    assert_eq!(journal.list_pending(&id).unwrap().len(), 2);

    journal.rollback(&eid1).unwrap();
    assert_eq!(journal.list_pending(&id).unwrap().len(), 1);

    journal.commit(&eid3).unwrap();
    assert_eq!(journal.list_pending(&id).unwrap().len(), 0);
}

#[test]
fn fjall_read_after_write_cross_instance_isolation() {
    let (_dir, keyspace) = create_keyspace();
    let journal = FjallEffectJournal::open(&keyspace).unwrap();
    let id1 = InstanceId::from_bytes([1u8; 16]);
    let id2 = InstanceId::from_bytes([2u8; 16]);

    let r1 = EffectRecord::new(
        "fx-isolated".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let r2 = EffectRecord::new(
        "fx-isolated".to_string(),
        EffectKind::SqlQuery,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    journal.prepare(&id1, r1).unwrap();
    journal.prepare(&id2, r2).unwrap();

    let pending1 = journal.list_pending(&id1).unwrap();
    let pending2 = journal.list_pending(&id2).unwrap();

    assert_eq!(pending1.len(), 1);
    assert_eq!(pending2.len(), 1);
    assert_eq!(pending1[0].kind(), EffectKind::HttpCall);
    assert_eq!(pending2[0].kind(), EffectKind::SqlQuery);
}

// ========================================================================
// Journal Replay Correctness — survive keyspace reopen
// ========================================================================

#[test]
fn fjall_replay_survives_keyspace_reopen_prepared_effects() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.path().to_path_buf();

    let eid_prepared;
    let id = sample_instance_id();

    {
        let keyspace = fjall::Database::builder(&dir_path)
            .open()
            .expect("keyspace");
        let journal = FjallEffectJournal::open(&keyspace).expect("journal");

        let record = EffectRecord::new(
            "fx-replay-pending".to_string(),
            EffectKind::HttpCall,
            json!({"url": "https://api.example.com"}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();

        eid_prepared = journal.prepare(&id, record).expect("prepare");
    }

    let keyspace = fjall::Database::builder(&dir_path)
        .open()
        .expect("keyspace reopen");
    let journal = FjallEffectJournal::open(&keyspace).expect("journal reopen");

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].intent_id(), "fx-replay-pending");

    journal.commit(&eid_prepared).expect("commit after replay");
    assert_eq!(journal.list_pending(&id).unwrap().len(), 0);
}

#[test]
fn fjall_replay_survives_keyspace_reopen_committed_effects() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.path().to_path_buf();
    let id = sample_instance_id();

    {
        let keyspace = fjall::Database::builder(&dir_path)
            .open()
            .expect("keyspace");
        let journal = FjallEffectJournal::open(&keyspace).expect("journal");

        let record = EffectRecord::new(
            "fx-replay-committed".to_string(),
            EffectKind::SqlQuery,
            json!({"sql": "INSERT INTO t VALUES (1)"}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();

        let eid = journal.prepare(&id, record).expect("prepare");
        journal.commit(&eid).expect("commit");
    }

    let keyspace = fjall::Database::builder(&dir_path)
        .open()
        .expect("keyspace reopen");
    let journal = FjallEffectJournal::open(&keyspace).expect("journal reopen");

    let pending = journal.list_pending(&id).unwrap();
    assert!(
        pending.is_empty(),
        "committed effect must not replay as pending"
    );

    let eid = EffectId::new(&id, "fx-replay-committed").unwrap();
    let commit_again = journal.commit(&eid);
    assert!(
        commit_again.is_err(),
        "re-commit after replay must fail with AlreadyTerminal"
    );
}

#[test]
fn fjall_replay_survives_keyspace_reopen_rolledback_effects() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.path().to_path_buf();
    let id = sample_instance_id();

    {
        let keyspace = fjall::Database::builder(&dir_path)
            .open()
            .expect("keyspace");
        let journal = FjallEffectJournal::open(&keyspace).expect("journal");

        let record = EffectRecord::new(
            "fx-replay-rolledback".to_string(),
            EffectKind::BlobWrite,
            json!({"key": "replay-test"}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();

        let eid = journal.prepare(&id, record).expect("prepare");
        journal.rollback(&eid).expect("rollback");
    }

    let keyspace = fjall::Database::builder(&dir_path)
        .open()
        .expect("keyspace reopen");
    let journal = FjallEffectJournal::open(&keyspace).expect("journal reopen");

    let pending = journal.list_pending(&id).unwrap();
    assert!(
        pending.is_empty(),
        "rolled back effect must not replay as pending"
    );

    let eid = EffectId::new(&id, "fx-replay-rolledback").unwrap();
    let rb_again = journal.rollback(&eid);
    assert!(
        rb_again.is_err(),
        "re-rollback after replay must fail with AlreadyTerminal"
    );
}

#[test]
fn fjall_replay_mixed_effects_survive_keyspace_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.path().to_path_buf();
    let id = sample_instance_id();

    {
        let keyspace = fjall::Database::builder(&dir_path)
            .open()
            .expect("keyspace");
        let journal = FjallEffectJournal::open(&keyspace).expect("journal");

        for i in 0..6 {
            let record = EffectRecord::new(
                format!("fx-mixed-{i}"),
                if i % 2 == 0 {
                    EffectKind::HttpCall
                } else {
                    EffectKind::SqlQuery
                },
                json!({"i": i}),
                EffectIntent::Prepared,
                None,
            )
            .unwrap();

            let eid = journal.prepare(&id, record).unwrap();
            match i % 3 {
                0 => journal.commit(&eid).unwrap(),
                1 => journal.rollback(&eid).unwrap(),
                _ => {}
            }
        }
    }

    let keyspace = fjall::Database::builder(&dir_path)
        .open()
        .expect("keyspace reopen");
    let journal = FjallEffectJournal::open(&keyspace).expect("journal reopen");

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(
        pending.len(),
        2,
        "only uncommitted effects should survive replay"
    );
    let intent_ids: Vec<&str> = pending.iter().map(|r| r.intent_id()).collect();
    assert!(intent_ids.contains(&"fx-mixed-2"));
    assert!(intent_ids.contains(&"fx-mixed-5"));
}

#[test]
fn fjall_replay_idempotent_prepare_after_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.path().to_path_buf();
    let id = sample_instance_id();

    {
        let keyspace = fjall::Database::builder(&dir_path)
            .open()
            .expect("keyspace");
        let journal = FjallEffectJournal::open(&keyspace).expect("journal");

        let record = EffectRecord::new(
            "fx-idempotent-replay".to_string(),
            EffectKind::HttpCall,
            json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();

        journal.prepare(&id, record).expect("first prepare");
    }

    let keyspace = fjall::Database::builder(&dir_path)
        .open()
        .expect("keyspace reopen");
    let journal = FjallEffectJournal::open(&keyspace).expect("journal reopen");

    let record2 = EffectRecord::new(
        "fx-idempotent-replay".to_string(),
        EffectKind::HttpCall,
        json!({"updated": true}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let eid = journal
        .prepare(&id, record2)
        .expect("idempotent replay prepare");
    assert_eq!(eid.as_str().contains("fx-idempotent-replay"), true);

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending.len(), 1, "idempotent replay must not duplicate");
}

#[test]
fn fjall_replay_compact_removes_terminal_after_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.path().to_path_buf();
    let id = sample_instance_id();

    {
        let keyspace = fjall::Database::builder(&dir_path)
            .open()
            .expect("keyspace");
        let journal = FjallEffectJournal::open(&keyspace).expect("journal");

        // Create 3 committed effects directly with known old timestamp
        for i in 0..3 {
            let record = EffectRecord::new(
                format!("fx-compact-{i}"),
                EffectKind::HttpCall,
                json!({}),
                EffectIntent::Committed,
                Some(vo_types::TimestampMs::parse("100").unwrap()),
            )
            .unwrap();
            journal.prepare(&id, record).unwrap();
        }
    }

    let keyspace = fjall::Database::builder(&dir_path)
        .open()
        .expect("keyspace reopen");
    let journal = FjallEffectJournal::open(&keyspace).expect("journal reopen");

    let ts = vo_types::TimestampMs::parse("1000").unwrap();
    let removed = journal.compact(ts).expect("compact after replay");
    assert_eq!(
        removed, 3,
        "all committed effects should be compacted after replay"
    );
}
