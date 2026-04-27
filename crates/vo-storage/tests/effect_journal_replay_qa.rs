//! QA: Effect journal read and replay verification (ve-ohmmh)

#![allow(clippy::unwrap_used)]

use vo_storage::effect_journal::{EffectId, EffectJournal, InMemoryEffectJournal};
use vo_types::{EffectIntent, EffectKind, EffectRecord, InstanceId};

fn sample_instance() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

fn make_record(intent_id: &str, kind: EffectKind) -> EffectRecord {
    EffectRecord::new(
        intent_id.to_string(),
        kind,
        serde_json::json!({"param": intent_id}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap()
}

#[test]
fn list_pending_returns_all_prepared_effects_for_instance() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance();

    let _e1 = journal
        .prepare(&id, make_record("fx-1", EffectKind::HttpCall))
        .unwrap();
    let e2 = journal
        .prepare(&id, make_record("fx-2", EffectKind::SqlQuery))
        .unwrap();
    let _ = journal
        .prepare(&id, make_record("fx-3", EffectKind::BlobWrite))
        .unwrap();

    // Commit one, so only 2 should be pending
    journal.commit(&e2).unwrap();

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending.len(), 2);
    let ids: Vec<&str> = pending.iter().map(|r| r.intent_id()).collect();
    assert!(ids.contains(&"fx-1"));
    assert!(ids.contains(&"fx-3"));
}

#[test]
fn replay_reconstructs_instance_state_from_journal() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance();

    // Simulate a workflow's effect history
    let e1 = journal
        .prepare(&id, make_record("charge-card", EffectKind::HttpCall))
        .unwrap();
    let _e2 = journal
        .prepare(&id, make_record("reserve-inventory", EffectKind::SqlQuery))
        .unwrap();
    let _e3 = journal
        .prepare(&id, make_record("send-email", EffectKind::HttpCall))
        .unwrap();

    // Workflow commits some, crashes before committing others
    journal.commit(&e1).unwrap();
    // e2 and e3 remain Prepared (crash happened before commit)

    // Replay: discover pending effects and resolve them
    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending.len(), 2);

    // Compensate: rollback uncommitted effects
    for record in &pending {
        let effect_id = EffectId::new(&id, record.intent_id()).unwrap();
        journal.rollback(&effect_id).unwrap();
    }

    // After replay, no pending effects remain
    let final_pending = journal.list_pending(&id).unwrap();
    assert!(
        final_pending.is_empty(),
        "replay should resolve all pending effects"
    );
}
