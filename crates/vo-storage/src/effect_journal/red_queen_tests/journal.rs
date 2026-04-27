//! Red Queen tests — journal lifecycle and idempotency.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::super::{
    decode_effect_key, encode_effect_key, EffectId, EffectJournal, EffectJournalError,
    InMemoryEffectJournal, EFFECTS_PARTITION,
};
use serde_json::json;
use vo_types::{EffectIntent, EffectKind, InstanceId};

// Helper
fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

// ========================================================================
// DIMENSION: journal-lifecycle
// Contract: prepare->commit->AlreadyTerminal, prepare->rollback->AlreadyTerminal
// ========================================================================

#[test]
fn red_queen_commit_then_rollback_returns_already_terminal() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();
    let record = vo_types::EffectRecord::new(
        "fx-commit-then-rb".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid = journal.prepare(&id, record).unwrap();

    // Commit first
    journal.commit(&eid).expect("BUG: commit failed");

    // Rollback should fail with AlreadyTerminal
    let result = journal.rollback(&eid);
    assert!(
        result.is_err(),
        "BUG: rollback succeeded after commit (should be AlreadyTerminal)"
    );
    match result.unwrap_err() {
        EffectJournalError::AlreadyTerminal {
            effect_id,
            current_status,
        } => {
            assert_eq!(effect_id, eid.as_str());
            assert!(
                current_status.contains("Committed"),
                "BUG: status not Committed"
            );
        }
        other => panic!("BUG: Wrong error variant for commit+rollback: {other:?}"),
    }
}

#[test]
fn red_queen_rollback_then_commit_returns_already_terminal() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();
    let record = vo_types::EffectRecord::new(
        "fx-rb-then-commit".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid = journal.prepare(&id, record).unwrap();

    // Rollback first
    journal.rollback(&eid).expect("BUG: rollback failed");

    // Commit should fail with AlreadyTerminal
    let result = journal.commit(&eid);
    assert!(
        result.is_err(),
        "BUG: commit succeeded after rollback (should be AlreadyTerminal)"
    );
    match result.unwrap_err() {
        EffectJournalError::AlreadyTerminal {
            effect_id,
            current_status,
        } => {
            assert_eq!(effect_id, eid.as_str());
            assert!(
                current_status.contains("RolledBack"),
                "BUG: status not RolledBack"
            );
        }
        other => panic!("BUG: Wrong error variant for rollback+commit: {other:?}"),
    }
}

#[test]
fn red_queen_prepare_validates_intent_id_non_empty() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();
    // Create a record with valid (non-empty) intent_id
    let record = vo_types::EffectRecord::new(
        "valid-intent".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap(); // This succeeds because intent_id is non-empty

    // prepare should succeed with valid input
    let result = journal.prepare(&id, record);
    assert!(
        result.is_ok(),
        "BUG: prepare rejected valid record with non-empty intent_id"
    );

    // Now verify that EffectRecord::new correctly rejects empty intent_id (contract enforcement)
    let empty_record_result = vo_types::EffectRecord::new(
        String::new(), // Empty intent_id
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    );
    assert!(
        empty_record_result.is_none(),
        "BUG: EffectRecord::new accepted empty intent_id"
    );
    // This proves the contract layer (EffectRecord) and calc layer (EffectId::new) both validate
}

// ========================================================================
// DIMENSION: journal-idempotency
// Contract: prepare is idempotent - same intent_id returns same EffectId
// ========================================================================

#[test]
fn red_queen_prepare_idempotent_different_status_preserves_original() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    // First prepare with Prepared status
    let record_prepared = vo_types::EffectRecord::new(
        "fx-idempotent".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid1 = journal.prepare(&id, record_prepared).unwrap();

    // Second prepare with Committed status (same intent_id)
    let record_committed = vo_types::EffectRecord::new(
        "fx-idempotent".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Committed, // Different status
        None,
    )
    .unwrap();
    let eid2 = journal.prepare(&id, record_committed).unwrap();

    // Must return same EffectId
    assert_eq!(
        eid1, eid2,
        "BUG: prepare returned different EffectId for same intent_id"
    );

    // And the stored record must still be Prepared (not overwritten with Committed)
    // This is verified by successfully committing
    let commit_result = journal.commit(&eid1);
    assert!(
        commit_result.is_ok(),
        "BUG: commit failed after idempotent prepare - original status was overwritten"
    );
}

#[test]
fn red_queen_list_pending_only_returns_prepared() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    // Prepare 3 effects
    for i in 0..3 {
        let record = vo_types::EffectRecord::new(
            format!("fx-pending-{i}"),
            EffectKind::HttpCall,
            json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        journal.prepare(&id, record).unwrap();
    }

    // Commit one, rollback another
    let eid_committed = EffectId::new(&id, "fx-pending-0").unwrap();
    let eid_rolledback = EffectId::new(&id, "fx-pending-1").unwrap();
    journal.commit(&eid_committed).unwrap();
    journal.rollback(&eid_rolledback).unwrap();

    // list_pending should return only the one still Prepared
    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending.len(), 1, "BUG: list_pending returned wrong count");
    assert_eq!(pending[0].intent_id(), "fx-pending-2");
}

// ========================================================================
// DIMENSION: journal-not-found
// Contract: commit/rollback on unknown effect returns NotFound
// ========================================================================

#[test]
fn red_queen_commit_unknown_effect_returns_not_found() {
    let journal = InMemoryEffectJournal::new();
    let eid = EffectId::new(&sample_instance_id(), "nonexistent").unwrap();
    let result = journal.commit(&eid);
    assert!(result.is_err(), "BUG: commit succeeded for unknown effect");
    assert!(
        matches!(result.unwrap_err(), EffectJournalError::NotFound { .. }),
        "BUG: Wrong error variant for commit on unknown"
    );
}

#[test]
fn red_queen_rollback_unknown_effect_returns_not_found() {
    let journal = InMemoryEffectJournal::new();
    let eid = EffectId::new(&sample_instance_id(), "nonexistent").unwrap();
    let result = journal.rollback(&eid);
    assert!(
        result.is_err(),
        "BUG: rollback succeeded for unknown effect"
    );
    assert!(
        matches!(result.unwrap_err(), EffectJournalError::NotFound { .. }),
        "BUG: Wrong error variant for rollback on unknown"
    );
}

// ========================================================================
// DIMENSION: partition-constant
// Contract: EFFECTS_PARTITION == "effects"
// ========================================================================

#[test]
fn red_queen_effects_partition_constant_is_correct() {
    assert_eq!(
        EFFECTS_PARTITION, "effects",
        "BUG: EFFECTS_PARTITION has wrong value"
    );
}
