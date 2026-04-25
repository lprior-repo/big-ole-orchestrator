//! Red Queen tests — compact functionality (non-concurrent).

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::super::{EffectId, EffectJournal, EffectJournalError, InMemoryEffectJournal, InstanceId};
use serde_json::json;
use vo_types::{EffectIntent, EffectKind};

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

fn ts(n: u64) -> vo_types::TimestampMs {
    vo_types::TimestampMs::parse(&n.to_string()).unwrap()
}

#[test]
fn red_queen_compact_removes_committed_effects_older_than_threshold() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    for i in 0..5u32 {
        let record = vo_types::EffectRecord::new(
            format!("fx-compact-{i}"),
            EffectKind::HttpCall,
            json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let eid = journal.prepare(&id, record).unwrap();
        journal.commit(&eid).unwrap();
    }

    let removed = journal.compact(ts(200)).unwrap();
    assert_eq!(
        removed, 5,
        "BUG: compact should remove all 5 committed effects"
    );

    let pending = journal.list_pending(&id).unwrap();
    assert!(pending.is_empty(), "BUG: compact left pending effects");
}

#[test]
fn red_queen_compact_preserves_prepared_effects() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    for i in 0..3u32 {
        let record = vo_types::EffectRecord::new(
            format!("fx-prepared-{i}"),
            EffectKind::HttpCall,
            json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        journal.prepare(&id, record).unwrap();
    }

    let removed = journal.compact(ts(99999)).unwrap();
    assert_eq!(removed, 0, "BUG: compact removed prepared effects");

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending.len(), 3, "BUG: compact destroyed prepared effects");
}

#[test]
fn red_queen_compact_preserves_committed_effects_newer_than_threshold() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    for i in 0..3u32 {
        let record = vo_types::EffectRecord::new(
            format!("fx-newer-{i}"),
            EffectKind::HttpCall,
            json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let eid = journal.prepare(&id, record).unwrap();
        journal.commit(&eid).unwrap();
    }

    let removed = journal.compact(ts(50)).unwrap();
    assert_eq!(
        removed, 0,
        "BUG: compact removed effects newer than threshold"
    );
}

#[test]
fn red_queen_compact_never_removes_rolledback_effects() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    for i in 0..3u32 {
        let record = vo_types::EffectRecord::new(
            format!("fx-rollback-{i}"),
            EffectKind::HttpCall,
            json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let eid = journal.prepare(&id, record).unwrap();
        journal.rollback(&eid).unwrap();
    }

    let removed = journal.compact(ts(99999)).unwrap();
    assert_eq!(
        removed, 0,
        "BUG: compact removed rolledback effects (committed_at is None)"
    );

    let eid = EffectId::new(&id, "fx-rollback-0").unwrap();
    let result = journal.commit(&eid);
    assert!(
        result.is_err(),
        "BUG: rolledback effect was destroyed by compact"
    );
    assert!(
        matches!(
            result.unwrap_err(),
            EffectJournalError::AlreadyTerminal { .. }
        ),
        "BUG: wrong error after compact touched rolledback effect"
    );
}

#[test]
fn red_queen_compact_mixed_states_removes_only_committed() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    for i in 0..2u32 {
        let record = vo_types::EffectRecord::new(
            format!("fx-mixed-prepared-{i}"),
            EffectKind::HttpCall,
            json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        journal.prepare(&id, record).unwrap();
    }
    for i in 0..2u32 {
        let record = vo_types::EffectRecord::new(
            format!("fx-mixed-committed-{i}"),
            EffectKind::SqlQuery,
            json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let eid = journal.prepare(&id, record).unwrap();
        journal.commit(&eid).unwrap();
    }
    for i in 0..2u32 {
        let record = vo_types::EffectRecord::new(
            format!("fx-mixed-rolledback-{i}"),
            EffectKind::BlobWrite,
            json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let eid = journal.prepare(&id, record).unwrap();
        journal.rollback(&eid).unwrap();
    }

    let removed = journal.compact(ts(200)).unwrap();
    assert_eq!(
        removed, 2,
        "BUG: compact should remove exactly the 2 committed effects"
    );

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending.len(), 2, "BUG: compact destroyed prepared effects");

    let rb0 = journal.rollback(&EffectId::new(&id, "fx-mixed-rolledback-0").unwrap());
    assert!(
        rb0.is_err(),
        "BUG: rolledback effect was removed by compact"
    );
}

#[test]
fn red_queen_compact_idempotent_double_compact_removes_zero_second_time() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let record = vo_types::EffectRecord::new(
        "fx-dbl-compact".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid = journal.prepare(&id, record).unwrap();
    journal.commit(&eid).unwrap();

    let first = journal.compact(ts(200)).unwrap();
    assert_eq!(first, 1);

    let second = journal.compact(ts(200)).unwrap();
    assert_eq!(
        second, 0,
        "BUG: second compact removed records that were already removed"
    );
}

#[test]
fn red_queen_compact_does_not_affect_other_instance() {
    let journal = InMemoryEffectJournal::new();
    let id_a = sample_instance_id();
    let id_b = InstanceId::from_bytes([2u8; 16]);

    let record_a = vo_types::EffectRecord::new(
        "fx-a-compact".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid_a = journal.prepare(&id_a, record_a).unwrap();
    journal.commit(&eid_a).unwrap();

    let record_b = vo_types::EffectRecord::new(
        "fx-b-no-compact".to_string(),
        EffectKind::SqlQuery,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    journal.prepare(&id_b, record_b).unwrap();

    journal.compact(ts(200)).unwrap();

    let pending_b = journal.list_pending(&id_b).unwrap();
    assert_eq!(
        pending_b.len(),
        1,
        "BUG: compact removed effects from wrong instance"
    );
}

#[test]
fn red_queen_compact_empty_journal_returns_zero() {
    let journal = InMemoryEffectJournal::new();
    let removed = journal.compact(ts(1000)).unwrap();
    assert_eq!(
        removed, 0,
        "BUG: compact on empty journal returned non-zero"
    );
}

#[test]
fn red_queen_compact_boundary_exact_timestamp() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let record = vo_types::EffectRecord::new(
        "fx-boundary".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid = journal.prepare(&id, record).unwrap();
    journal.commit(&eid).unwrap();

    let removed = journal.compact(ts(100)).unwrap();
    assert_eq!(
        removed, 0,
        "BUG: compact with threshold == committed_at removed effect (should be strict <)"
    );

    let removed = journal.compact(ts(101)).unwrap();
    assert_eq!(
        removed, 1,
        "BUG: compact with threshold > committed_at did not remove effect"
    );
}

#[test]
fn red_queen_compact_then_crash_recovery_preserves_pending() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    for i in 0..3u32 {
        let record = vo_types::EffectRecord::new(
            format!("fx-compact-crash-{i}"),
            EffectKind::HttpCall,
            json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        journal.prepare(&id, record).unwrap();
    }

    journal.compact(ts(99999)).unwrap();

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(
        pending.len(),
        3,
        "BUG: compact destroyed pending effects before crash recovery"
    );

    for record in &pending {
        let eid = EffectId::new(&id, record.intent_id()).unwrap();
        journal.commit(&eid).unwrap();
    }
}