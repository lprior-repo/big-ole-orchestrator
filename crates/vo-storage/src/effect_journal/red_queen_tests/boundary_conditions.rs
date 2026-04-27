//! Red Queen tests — boundary conditions.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::super::{decode_effect_key, encode_effect_key, EffectId, EffectJournal, InMemoryEffectJournal, InstanceId};
use serde_json::json;
use vo_types::{EffectIntent, EffectKind};

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

#[test]
fn red_queen_effectid_with_unicode_intent_id() {
    let id = sample_instance_id();
    let unicode_intents = vec!["café", "日本語", "emoji🦀", "مرحبا", "naïve", "ß"];

    for intent in unicode_intents {
        let eid = EffectId::new(&id, intent).unwrap();
        assert_eq!(eid.as_str(), format!("{id}::{intent}"));

        let bytes = encode_effect_key(&eid);
        let recovered = decode_effect_key(&bytes).unwrap();
        assert_eq!(
            recovered, eid,
            "BUG: unicode roundtrip failed for: {intent}"
        );
    }
}

#[test]
fn red_queen_effectid_with_very_long_intent_id() {
    let id = sample_instance_id();
    let long_intent: String = "x".repeat(10_000);
    let eid = EffectId::new(&id, &long_intent).unwrap();

    let bytes = encode_effect_key(&eid);
    assert_eq!(bytes.len(), long_intent.len() + id.to_string().len() + 2);
    let recovered = decode_effect_key(&bytes).unwrap();
    assert_eq!(recovered, eid);
}

#[test]
fn red_queen_prepare_with_all_effect_kinds() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();
    let kinds = [
        EffectKind::HttpCall,
        EffectKind::SqlQuery,
        EffectKind::BlobWrite,
    ];

    for kind in &kinds {
        let record = vo_types::EffectRecord::new(
            format!("fx-kind-{:?}", kind),
            *kind,
            json!({"kind_test": true}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let _eid = journal.prepare(&id, record).unwrap();

        let pending = journal.list_pending(&id).unwrap();
        let found = pending
            .iter()
            .find(|r| r.intent_id() == format!("fx-kind-{:?}", kind));
        assert!(
            found.is_some(),
            "BUG: effect with kind {:?} not found in pending",
            kind
        );
        assert_eq!(
            found.unwrap().kind(),
            *kind,
            "BUG: EffectKind not preserved"
        );
    }
}

#[test]
fn red_queen_record_codec_with_complex_json_params() {
    let complex_params = json!({
        "nested": {
            "deep": {
                "value": [1, 2, 3, null, true, false, "string"]
            }
        },
        "unicode_keys": {
            "日本語": "value",
            "emoji": "🦀"
        },
        "empty_struct": {},
        "number": -3.14e10,
        "big_int": 9_223_372_036_854_775_807_i64
    });

    let record = vo_types::EffectRecord::new(
        "fx-complex-params".to_string(),
        EffectKind::HttpCall,
        complex_params.clone(),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let bytes = super::super::encode_effect_record(&record).unwrap();
    let recovered = super::super::decode_effect_record(&bytes).unwrap();
    assert_eq!(recovered.params_json(), &complex_params);
}

#[test]
fn red_queen_effectid_with_delimiter_in_intent_id() {
    let id = sample_instance_id();
    let intent = "has::delimiters::inside";
    let eid = EffectId::new(&id, intent).unwrap();

    let as_str = eid.as_str();
    assert!(as_str.contains("has::delimiters::inside"));

    let bytes = encode_effect_key(&eid);
    let recovered = decode_effect_key(&bytes).unwrap();
    assert_eq!(recovered, eid);
}

#[test]
fn red_queen_list_pending_preserves_effect_metadata() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let record = vo_types::EffectRecord::new(
        "fx-metadata-check".to_string(),
        EffectKind::SqlQuery,
        json!({"query": "SELECT * FROM users WHERE id = $1", "params": [42]}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    journal.prepare(&id, record).unwrap();

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending.len(), 1);

    let r = &pending[0];
    assert_eq!(r.intent_id(), "fx-metadata-check");
    assert_eq!(r.kind(), EffectKind::SqlQuery);
    assert_eq!(r.status(), EffectIntent::Prepared);
    assert_eq!(
        r.params_json()["query"],
        "SELECT * FROM users WHERE id = $1"
    );
    assert_eq!(r.params_json()["params"][0], 42);
    assert!(r.committed_at().is_none());
}

#[test]
fn red_queen_commit_adds_timestamp_to_record() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let record = vo_types::EffectRecord::new(
        "fx-timestamp-check".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid = journal.prepare(&id, record).unwrap();

    let pending = journal.list_pending(&id).unwrap();
    assert!(pending[0].committed_at().is_none());

    journal.commit(&eid).unwrap();

    let after = journal.list_pending(&id).unwrap();
    assert!(after.is_empty());
}