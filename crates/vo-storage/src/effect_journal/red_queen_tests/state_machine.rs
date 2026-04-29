//! Red Queen tests — state machine exhaustive transitions.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::super::{
    EffectId, EffectJournal, EffectJournalError, InMemoryEffectJournal, InstanceId,
};
use serde_json::json;
use vo_types::{EffectIntent, EffectKind};

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

#[test]
fn red_queen_state_machine_prepared_to_committed_all_invalid_rejected() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let record = vo_types::EffectRecord::new(
        "fx-sm".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid = journal.prepare(&id, record).unwrap();

    assert!(journal.commit(&eid).is_ok());

    assert!(matches!(
        journal.commit(&eid),
        Err(EffectJournalError::AlreadyTerminal { .. })
    ));

    assert!(matches!(
        journal.rollback(&eid),
        Err(EffectJournalError::AlreadyTerminal { .. })
    ));
}

#[test]
fn red_queen_state_machine_prepared_to_rollback_all_invalid_rejected() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let record = vo_types::EffectRecord::new(
        "fx-sm-rb".to_string(),
        EffectKind::SqlQuery,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid = journal.prepare(&id, record).unwrap();

    assert!(journal.rollback(&eid).is_ok());

    assert!(matches!(
        journal.commit(&eid),
        Err(EffectJournalError::AlreadyTerminal { .. })
    ));

    assert!(matches!(
        journal.rollback(&eid),
        Err(EffectJournalError::AlreadyTerminal { .. })
    ));
}

#[test]
fn red_queen_error_display_contains_actionable_info() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let record = vo_types::EffectRecord::new(
        "fx-err-display".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid = journal.prepare(&id, record).unwrap();
    journal.commit(&eid).unwrap();

    let err = journal.rollback(&eid).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains(&eid.as_str()),
        "BUG: AlreadyTerminal error doesn't contain effect_id"
    );
    assert!(
        msg.contains("Committed"),
        "BUG: AlreadyTerminal error doesn't contain current status"
    );
}
