//! `EffectJournal` trait integration tests — prepare, commit, rollback, `list_pending`.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
use super::super::*;
use serde_json::json;
use vo_types::{EffectIntent, EffectKind};

// Helper: valid InstanceId for tests
fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

// ========================================================================
// Trait Integration — via InMemoryEffectJournal
// ========================================================================

#[test]
fn prepare_returns_effect_id_for_new_intent() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();
    let record = EffectRecord::new(
        "fx-1".to_string(),
        EffectKind::HttpCall,
        json!({"url": "https://api.stripe.com"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let result = journal.prepare(&id, record);
    let expected = EffectId::new(&id, "fx-1").unwrap();
    assert_eq!(result, Ok(expected.clone()));
    assert_eq!(result.unwrap().as_str(), expected.as_str());
}

#[test]
fn prepare_is_idempotent_for_same_intent_id() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();
    let record = EffectRecord::new(
        "fx-1".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let first = journal.prepare(&id, record.clone()).unwrap();
    let second = journal.prepare(&id, record).unwrap();
    assert_eq!(first, second);
    // Verify internal state: only ONE record exists (idempotent, not duplicated)
    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(
        pending.len(),
        1,
        "idempotent prepare must not duplicate records"
    );
}

#[test]
fn prepare_idempotent_returns_existing_when_status_differs() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();
    // First record with Prepared status
    let record_prepared = EffectRecord::new(
        "fx-status-diff".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let effect_id = journal.prepare(&id, record_prepared).unwrap();
    // Second record with same intent_id but different status (Committed)
    let record_committed = EffectRecord::new(
        "fx-status-diff".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Committed,
        None,
    )
    .unwrap();
    let second = journal.prepare(&id, record_committed).unwrap();
    // Must return the same EffectId (idempotent for same intent_id)
    assert_eq!(
        effect_id, second,
        "prepare must be idempotent for same intent_id regardless of status"
    );
    // Verify the stored record still has Prepared status by committing it.
    let commit_result = journal.commit(&effect_id);
    assert!(
        commit_result.is_ok(),
        "commit must succeed if original Prepared status was preserved"
    );
}

#[test]
fn commit_transitions_prepared_to_committed() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();
    let record = EffectRecord::new(
        "fx-commit".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid = journal.prepare(&id, record).unwrap();
    let result = journal.commit(&eid);
    assert_eq!(result, Ok(()));
    // Verify via list_pending: committed effects should not appear
    let pending = journal.list_pending(&id).unwrap();
    assert!(pending.is_empty());
}

#[test]
fn rollback_transitions_prepared_to_rolledback() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();
    let record = EffectRecord::new(
        "fx-rollback".to_string(),
        EffectKind::SqlQuery,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid = journal.prepare(&id, record).unwrap();
    let result = journal.rollback(&eid);
    assert_eq!(result, Ok(()));
    // Verify via list_pending: rolled back effects should not appear
    let pending = journal.list_pending(&id).unwrap();
    assert!(pending.is_empty());
}

#[test]
fn list_pending_returns_only_prepared_effects() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let r1 = EffectRecord::new(
        "fx-pending".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let r2 = EffectRecord::new(
        "fx-committed".to_string(),
        EffectKind::SqlQuery,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let r3 = EffectRecord::new(
        "fx-rolledback".to_string(),
        EffectKind::BlobWrite,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let eid2 = journal.prepare(&id, r2).unwrap();
    let eid1 = journal.prepare(&id, r1).unwrap();
    let eid3 = journal.prepare(&id, r3).unwrap();

    assert_eq!(eid1, EffectId::new(&id, "fx-pending").unwrap());

    journal.commit(&eid2).unwrap();
    journal.rollback(&eid3).unwrap();

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].intent_id(), "fx-pending");
}
