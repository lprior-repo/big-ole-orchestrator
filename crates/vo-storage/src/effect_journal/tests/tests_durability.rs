//! Effect journal durability tests — write durability, read-after-write consistency,
//! journal replay correctness (ADR-030).
//!
//! These tests exercise the `FjallEffectJournal` (production backend) to verify:
//! - Writes survive keyspace reopen (durability)
//! - Read-after-write consistency (freshly written record is immediately visible)
//! - Journal replay: after crash, pending effects are recoverable
//! - Idempotency: duplicate prepare with same `intent_id` returns same `effect_id`
//! - Compact correctness: only terminal + old effects are removed

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::super::*;
use serde_json::json;
use vo_types::{EffectIntent, EffectKind};

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

fn http_record(intent_id: &str, params: serde_json::Value) -> EffectRecord {
    EffectRecord::new(
        intent_id.to_string(),
        EffectKind::HttpCall,
        params,
        EffectIntent::Prepared,
        None,
    )
    .unwrap()
}

fn open_journal(dir: &std::path::Path) -> FjallEffectJournal {
    let keyspace = fjall::Database::builder(dir).open().unwrap();
    FjallEffectJournal::open(&keyspace).unwrap()
}

#[test]
fn fjall_write_survives_keyspace_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();

    // Phase 1: write
    {
        let journal = open_journal(dir.path());
        let eid = journal
            .prepare(
                &id,
                http_record("fx-durable", json!({"url": "https://api.example.com"})),
            )
            .unwrap();
        assert_eq!(eid.as_str(), format!("{id}::fx-durable"));
    } // keyspace dropped — simulates process exit

    // Phase 2: reopen and verify
    {
        let journal = open_journal(dir.path());
        let pending = journal.list_pending(&id).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].intent_id(), "fx-durable");
        assert_eq!(pending[0].kind(), EffectKind::HttpCall);
    }
}

#[test]
fn fjall_committed_state_survives_keyspace_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();

    // Phase 1: prepare + commit
    let eid = {
        let journal = open_journal(dir.path());
        let eid = journal
            .prepare(&id, http_record("fx-committed-durable", json!({})))
            .unwrap();
        journal.commit(&eid).unwrap();
        eid
    }; // keyspace dropped

    // Phase 2: reopen — committed effect must not appear as pending
    {
        let journal = open_journal(dir.path());
        let pending = journal.list_pending(&id).unwrap();
        assert!(
            pending.is_empty(),
            "committed effect must not appear as pending after reopen"
        );

        // Attempting to commit again must fail (AlreadyTerminal)
        let result = journal.commit(&eid);
        assert!(matches!(
            result,
            Err(EffectJournalError::AlreadyTerminal { .. })
        ));
    }
}

#[test]
fn fjall_rolledback_state_survives_keyspace_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();

    let eid = {
        let journal = open_journal(dir.path());
        let eid = journal
            .prepare(
                &id,
                EffectRecord::new(
                    "fx-rb-durable".to_string(),
                    EffectKind::SqlQuery,
                    json!({"query": "SELECT 1"}),
                    EffectIntent::Prepared,
                    None,
                )
                .unwrap(),
            )
            .unwrap();
        journal.rollback(&eid).unwrap();
        eid
    };

    {
        let journal = open_journal(dir.path());
        let pending = journal.list_pending(&id).unwrap();
        assert!(pending.is_empty());
        let result = journal.rollback(&eid);
        assert!(matches!(
            result,
            Err(EffectJournalError::AlreadyTerminal { .. })
        ));
    }
}

#[test]
fn fjall_read_after_write_pending_visible_immediately() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();
    let journal = open_journal(dir.path());

    let _eid = journal
        .prepare(&id, http_record("fx-raw", json!({})))
        .unwrap();

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].intent_id(), "fx-raw");
    assert_eq!(pending[0].status(), EffectIntent::Prepared);
}

#[test]
fn fjall_read_after_write_preserves_params_json() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();
    let journal = open_journal(dir.path());

    let params = json!({
        "url": "https://api.stripe.com/v1/charges",
        "method": "POST",
        "idempotency_key": "ch_12345"
    });
    journal
        .prepare(&id, http_record("fx-params", params.clone()))
        .unwrap();

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending[0].params_json(), &params);
}

#[test]
fn fjall_replay_discovers_pending_effects_after_crash() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();

    // Phase 1: prepare 5 effects, commit 2, then "crash"
    {
        let journal = open_journal(dir.path());
        for i in 0..5 {
            journal
                .prepare(&id, http_record(&format!("fx-replay-{i}"), json!({})))
                .unwrap();
        }
        for i in 0..2 {
            let eid = EffectId::new(&id, &format!("fx-replay-{i}")).unwrap();
            journal.commit(&eid).unwrap();
        }
    }

    // Phase 2: reopen — 3 pending effects must be discoverable
    {
        let journal = open_journal(dir.path());
        let pending = journal.list_pending(&id).unwrap();
        assert_eq!(pending.len(), 3, "3 effects should be pending after crash");

        // Replay: rollback all pending (compensation for crash recovery)
        for record in &pending {
            let eid = EffectId::new(&id, record.intent_id()).unwrap();
            journal.rollback(&eid).unwrap();
        }
        let pending_after = journal.list_pending(&id).unwrap();
        assert!(pending_after.is_empty());
    }
}

#[test]
fn fjall_replay_isolated_per_instance() {
    let dir = tempfile::tempdir().unwrap();
    let id1 = sample_instance_id();
    let id2 = InstanceId::from_bytes([2u8; 16]);

    {
        let journal = open_journal(dir.path());
        journal
            .prepare(&id1, http_record("fx-inst1-a", json!({})))
            .unwrap();
        journal
            .prepare(&id1, http_record("fx-inst1-b", json!({})))
            .unwrap();
        journal
            .prepare(&id2, http_record("fx-inst2-a", json!({})))
            .unwrap();
    }

    {
        let journal = open_journal(dir.path());
        assert_eq!(journal.list_pending(&id1).unwrap().len(), 2);
        assert_eq!(journal.list_pending(&id2).unwrap().len(), 1);

        for record in journal.list_pending(&id1).unwrap() {
            let eid = EffectId::new(&id1, record.intent_id()).unwrap();
            journal.rollback(&eid).unwrap();
        }
        assert_eq!(journal.list_pending(&id1).unwrap().len(), 0);
        assert_eq!(journal.list_pending(&id2).unwrap().len(), 1);
    }
}

#[test]
fn fjall_idempotent_prepare_after_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();

    {
        let journal = open_journal(dir.path());
        let first = journal
            .prepare(&id, http_record("fx-idem", json!({})))
            .unwrap();
        let second = journal
            .prepare(&id, http_record("fx-idem", json!({"updated": true})))
            .unwrap();
        assert_eq!(first, second, "same intent_id must return same effect_id");
    }

    {
        let journal = open_journal(dir.path());
        let third = journal
            .prepare(&id, http_record("fx-idem", json!({"again": true})))
            .unwrap();
        let expected = EffectId::new(&id, "fx-idem").unwrap();
        assert_eq!(third, expected);
        assert_eq!(journal.list_pending(&id).unwrap().len(), 1);
    }
}

#[test]
fn fjall_compact_after_reopen_removes_correct_effects() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();

    {
        let journal = open_journal(dir.path());
        let committed_eid = journal
            .prepare(&id, http_record("fx-compact-old", json!({})))
            .unwrap();
        journal.commit(&committed_eid).unwrap();
        journal
            .prepare(&id, http_record("fx-compact-still-pending", json!({})))
            .unwrap();
    }

    {
        let journal = open_journal(dir.path());
        let old_ts = vo_types::TimestampMs::parse("150").unwrap();
        let removed = journal.compact(old_ts).unwrap();
        assert_eq!(removed, 1, "only the committed effect should be removed");

        let pending = journal.list_pending(&id).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].intent_id(), "fx-compact-still-pending");
    }
}
