//! Red Queen tests — cross-instance isolation.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::super::{EffectId, EffectJournal, InMemoryEffectJournal, InstanceId};
use serde_json::json;
use vo_types::{EffectIntent, EffectKind};

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

#[test]
fn red_queen_cross_instance_isolation_pending_lists() {
    let journal = InMemoryEffectJournal::new();
    let id_a = sample_instance_id();
    let id_b = InstanceId::from_bytes([2u8; 16]);

    for i in 0..3u32 {
        let record = vo_types::EffectRecord::new(
            format!("fx-a-{i}"),
            EffectKind::HttpCall,
            json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        journal.prepare(&id_a, record).unwrap();
    }

    for i in 0..2u32 {
        let record = vo_types::EffectRecord::new(
            format!("fx-b-{i}"),
            EffectKind::SqlQuery,
            json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        journal.prepare(&id_b, record).unwrap();
    }

    let pending_a = journal.list_pending(&id_a).unwrap();
    assert_eq!(pending_a.len(), 3, "BUG: instance A pending count wrong");
    for r in &pending_a {
        assert!(
            r.intent_id().starts_with("fx-a-"),
            "BUG: instance B effect leaked into instance A's pending list: {}",
            r.intent_id()
        );
    }

    let pending_b = journal.list_pending(&id_b).unwrap();
    assert_eq!(pending_b.len(), 2, "BUG: instance B pending count wrong");
    for r in &pending_b {
        assert!(
            r.intent_id().starts_with("fx-b-"),
            "BUG: instance A effect leaked into instance B's pending list: {}",
            r.intent_id()
        );
    }
}

#[test]
fn red_queen_cross_instance_commit_does_not_affect_other_instance() {
    let journal = InMemoryEffectJournal::new();
    let id_a = sample_instance_id();
    let id_b = InstanceId::from_bytes([2u8; 16]);

    let record_a = vo_types::EffectRecord::new(
        "shared-intent".to_string(),
        EffectKind::HttpCall,
        json!({"instance": "A"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let record_b = vo_types::EffectRecord::new(
        "shared-intent".to_string(),
        EffectKind::SqlQuery,
        json!({"instance": "B"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let eid_a = journal.prepare(&id_a, record_a).unwrap();
    let eid_b = journal.prepare(&id_b, record_b).unwrap();

    journal.commit(&eid_a).unwrap();

    let pending_b = journal.list_pending(&id_b).unwrap();
    assert_eq!(
        pending_b.len(),
        1,
        "BUG: committing A's effect affected B's pending list"
    );

    let commit_b = journal.commit(&eid_b);
    assert!(
        commit_b.is_ok(),
        "BUG: B's effect became uncommittable after A committed"
    );
}

#[test]
fn red_queen_list_pending_for_empty_instance_returns_empty() {
    let journal = InMemoryEffectJournal::new();
    let id_a = sample_instance_id();
    let id_b = InstanceId::from_bytes([2u8; 16]);

    let record = vo_types::EffectRecord::new(
        "fx-only-a".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    journal.prepare(&id_a, record).unwrap();

    let pending_b = journal.list_pending(&id_b).unwrap();
    assert!(
        pending_b.is_empty(),
        "BUG: empty instance returned non-empty pending list"
    );
}