//! Effect journal append correctness tests.
//!
//! Verifies that all effect records are correctly persisted via prepare,
//! covering single append, batch append, and concurrent append scenarios.
//!
//! bead_id: ve-u3gwi

use super::super::*;
use serde_json::json;
use std::sync::Arc;
use vo_types::{EffectIntent, EffectKind, InstanceId};

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

#[test]
fn single_prepare_record_is_persisted_and_retrievable() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();
    let record = EffectRecord::new(
        "fx-single".to_string(),
        EffectKind::HttpCall,
        json!({"url": "https://api.example.com/charge"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let effect_id = journal.prepare(&id, record.clone()).unwrap();

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending.len(), 1, "Single prepare should yield exactly one pending record");
    assert_eq!(pending[0].intent_id(), "fx-single");
    assert_eq!(pending[0].kind(), EffectKind::HttpCall);
    assert_eq!(pending[0].status(), EffectIntent::Prepared);
    assert_eq!(pending[0].params_json(), &json!({"url": "https://api.example.com/charge"}));
    assert_eq!(effect_id.as_str(), format!("{}::fx-single", id.as_str()));
}

#[test]
fn batch_prepare_all_records_persisted() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();
    let batch_size = 100;

    let mut expected_intent_ids = Vec::with_capacity(batch_size);
    for i in 0..batch_size {
        let intent_id = format!("fx-batch-{i}");
        expected_intent_ids.push(intent_id.clone());
        let record = EffectRecord::new(
            intent_id,
            EffectKind::HttpCall,
            json!({"index": i}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let result = journal.prepare(&id, record);
        assert!(result.is_ok(), "prepare should succeed for record {i}");
    }

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(
        pending.len(),
        batch_size,
        "All {} batch records should be persisted",
        batch_size
    );

    let mut actual_intent_ids: Vec<String> =
        pending.iter().map(|r| r.intent_id().to_string()).collect();
    actual_intent_ids.sort();
    expected_intent_ids.sort();
    assert_eq!(
        actual_intent_ids, expected_intent_ids,
        "All intent IDs must match"
    );
}

#[test]
fn concurrent_prepare_all_records_persisted_without_loss() {
    use std::thread;

    let journal = Arc::new(InMemoryEffectJournal::new());
    let id = sample_instance_id();
    let thread_count = 8;
    let records_per_thread = 50;

    let handles: Vec<_> = (0..thread_count)
        .map(|t| {
            let j = journal.clone();
            let inst = id.clone();
            thread::spawn(move || {
                for i in 0..records_per_thread {
                    let intent_id = format!("fx-t{t}-r{i}");
                    let record = EffectRecord::new(
                        intent_id,
                        EffectKind::SqlQuery,
                        json!({"thread": t, "index": i}),
                        EffectIntent::Prepared,
                        None,
                    )
                    .unwrap();
                    let result = j.prepare(&inst, record);
                    assert!(result.is_ok(), "concurrent prepare failed for t{t}-r{i}");
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread panicked");
    }

    let pending = journal.list_pending(&id).unwrap();
    let expected_total = thread_count * records_per_thread;
    assert_eq!(
        pending.len(),
        expected_total,
        "All {} concurrent records must be persisted without loss",
        expected_total
    );
}
