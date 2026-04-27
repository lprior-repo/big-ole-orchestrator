//! Red Queen tests — crash recovery after prepare without commit/rollback.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::super::{EffectId, EffectJournal, InMemoryEffectJournal, InstanceId};
use serde_json::json;
use vo_types::{EffectIntent, EffectKind};

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

#[test]
fn red_queen_crash_after_prepare_recovers_all_pending_effects() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    for i in 0..5u32 {
        let record = vo_types::EffectRecord::new(
            format!("fx-crash-{i}"),
            EffectKind::HttpCall,
            json!({"step": i}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        journal.prepare(&id, record).unwrap();
    }

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(
        pending.len(),
        5,
        "BUG: crash recovery lost prepared effects"
    );

    let mut recovered_ids: Vec<String> =
        pending.iter().map(|r| r.intent_id().to_string()).collect();
    recovered_ids.sort();
    for (i, name) in recovered_ids.iter().enumerate() {
        assert_eq!(
            name,
            &format!("fx-crash-{i}"),
            "BUG: recovered effect has wrong intent_id"
        );
    }
}

#[test]
fn red_queen_crash_recovery_can_commit_each_recovered_effect() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    for i in 0..3u32 {
        let record = vo_types::EffectRecord::new(
            format!("fx-recover-{i}"),
            EffectKind::SqlQuery,
            json!({"q": format!("SELECT {i}")}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        journal.prepare(&id, record).unwrap();
    }

    let pending = journal.list_pending(&id).unwrap();
    for record in &pending {
        let eid = EffectId::new(&id, record.intent_id()).unwrap();
        journal.commit(&eid).unwrap();
    }

    let after = journal.list_pending(&id).unwrap();
    assert!(
        after.is_empty(),
        "BUG: pending not empty after recovering all effects"
    );
}

#[test]
fn red_queen_crash_recovery_can_rollback_each_recovered_effect() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    for i in 0..3u32 {
        let record = vo_types::EffectRecord::new(
            format!("fx-rb-recover-{i}"),
            EffectKind::BlobWrite,
            json!({"key": format!("blob-{i}")}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        journal.prepare(&id, record).unwrap();
    }

    let pending = journal.list_pending(&id).unwrap();
    for record in &pending {
        let eid = EffectId::new(&id, record.intent_id()).unwrap();
        journal.rollback(&eid).unwrap();
    }

    let after = journal.list_pending(&id).unwrap();
    assert!(
        after.is_empty(),
        "BUG: pending not empty after rolling back all"
    );
}

#[test]
fn red_queen_partial_commit_then_crash_recovers_remaining() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    for i in 0..5u32 {
        let record = vo_types::EffectRecord::new(
            format!("fx-partial-{i}"),
            EffectKind::HttpCall,
            json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        journal.prepare(&id, record).unwrap();
    }

    journal
        .commit(&EffectId::new(&id, "fx-partial-0").unwrap())
        .unwrap();
    journal
        .commit(&EffectId::new(&id, "fx-partial-1").unwrap())
        .unwrap();

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(
        pending.len(),
        3,
        "BUG: partial commit recovery returned wrong count"
    );

    let double = journal.commit(&EffectId::new(&id, "fx-partial-0").unwrap());
    assert!(
        double.is_err(),
        "BUG: double-commit after crash recovery succeeded"
    );
}