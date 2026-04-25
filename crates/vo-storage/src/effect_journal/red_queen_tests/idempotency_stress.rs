//! Red Queen tests — idempotency stress.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::super::{EffectJournal, InMemoryEffectJournal, InstanceId};
use serde_json::json;
use vo_types::{EffectIntent, EffectKind};

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

#[test]
fn red_queen_prepare_idempotent_different_kind_preserves_original() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let record_http = vo_types::EffectRecord::new(
        "fx-kind-switch".to_string(),
        EffectKind::HttpCall,
        json!({"url": "https://example.com"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid = journal.prepare(&id, record_http).unwrap();

    let record_sql = vo_types::EffectRecord::new(
        "fx-kind-switch".to_string(),
        EffectKind::SqlQuery,
        json!({"q": "DROP TABLE users"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid2 = journal.prepare(&id, record_sql).unwrap();

    assert_eq!(
        eid, eid2,
        "BUG: idempotent prepare returned different EffectId"
    );

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(
        pending[0].kind(),
        EffectKind::HttpCall,
        "BUG: idempotent prepare overwrote original EffectKind"
    );
    assert_eq!(
        pending[0].params_json()["url"],
        "https://example.com",
        "BUG: idempotent prepare overwrote original params"
    );
}

#[test]
fn red_queen_prepare_idempotent_different_params_preserves_original() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let record_v1 = vo_types::EffectRecord::new(
        "fx-params-v1".to_string(),
        EffectKind::HttpCall,
        json!({"amount": 100, "currency": "USD"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid = journal.prepare(&id, record_v1).unwrap();

    let record_v2 = vo_types::EffectRecord::new(
        "fx-params-v1".to_string(),
        EffectKind::HttpCall,
        json!({"amount": 99999, "currency": "BTC"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid2 = journal.prepare(&id, record_v2).unwrap();

    assert_eq!(eid, eid2);

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending[0].params_json()["amount"], 100);
    assert_eq!(pending[0].params_json()["currency"], "USD");
}

#[test]
fn red_queen_prepare_100_times_same_intent_id_produces_single_record() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    for i in 0..100u32 {
        let record = vo_types::EffectRecord::new(
            "fx-stress-idempotent".to_string(),
            EffectKind::HttpCall,
            json!({"attempt": i}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let eid = journal.prepare(&id, record).unwrap();
        assert_eq!(
            eid.as_str(),
            format!("{id}::fx-stress-idempotent"),
            "BUG: EffectId changed on iteration {i}"
        );
    }

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(
        pending.len(),
        1,
        "BUG: 100 idempotent prepares produced {} records instead of 1",
        pending.len()
    );
}