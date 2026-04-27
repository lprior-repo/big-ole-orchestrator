//! Red Queen tests — concurrent access.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::super::{decode_effect_key, EffectId, EffectJournal, EffectJournalError, InMemoryEffectJournal, InstanceId};
use serde_json::json;
use vo_types::{EffectIntent, EffectKind};
use std::sync::Arc;
use std::thread;

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

#[test]
fn red_queen_concurrent_prepare_same_intent_id_is_idempotent() {
    let journal = Arc::new(InMemoryEffectJournal::new());
    let id = sample_instance_id();
    let num_threads = 8;

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let j = Arc::clone(&journal);
            let id = id.clone();
            thread::spawn(move || {
                let record = vo_types::EffectRecord::new(
                    "fx-concurrent-same".to_string(),
                    EffectKind::HttpCall,
                    json!({"thread": i}),
                    EffectIntent::Prepared,
                    None,
                )
                .unwrap();
                j.prepare(&id, record).unwrap()
            })
        })
        .collect();

    let mut effect_ids: Vec<EffectId> =
        handles.into_iter().map(|h| h.join().unwrap()).collect();

    let first = effect_ids.pop().unwrap();
    for eid in &effect_ids {
        assert_eq!(
            *eid, first,
            "BUG: concurrent prepare returned different EffectId for same intent_id"
        );
    }

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(
        pending.len(),
        1,
        "BUG: concurrent idempotent prepare produced {} records",
        pending.len()
    );
}

#[test]
fn red_queen_concurrent_prepare_different_intent_ids_all_succeed() {
    let journal = Arc::new(InMemoryEffectJournal::new());
    let id = sample_instance_id();
    let num_threads = 16;

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let j = Arc::clone(&journal);
            let id = id.clone();
            thread::spawn(move || {
                let record = vo_types::EffectRecord::new(
                    format!("fx-concurrent-{i}"),
                    EffectKind::HttpCall,
                    json!({}),
                    EffectIntent::Prepared,
                    None,
                )
                .unwrap();
                j.prepare(&id, record).unwrap()
            })
        })
        .collect();

    let effect_ids: std::collections::HashSet<_> =
        handles.into_iter().map(|h| h.join().unwrap()).collect();

    assert_eq!(
        effect_ids.len(),
        num_threads as usize,
        "BUG: concurrent unique prepares produced duplicate EffectIds"
    );

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending.len(), num_threads as usize);
}

#[test]
fn red_queen_concurrent_commit_rollback_on_same_effect_one_wins() {
    let journal = Arc::new(InMemoryEffectJournal::new());
    let id = sample_instance_id();

    let record = vo_types::EffectRecord::new(
        "fx-race-commit-rb".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid = journal.prepare(&id, record).unwrap();
    let eid = Arc::new(eid);

    let mut handles = Vec::new();
    for _ in 0..4 {
        let j = Arc::clone(&journal);
        let e = Arc::clone(&eid);
        handles.push(thread::spawn(move || j.commit(&e)));
    }
    for _ in 0..4 {
        let j = Arc::clone(&journal);
        let e = Arc::clone(&eid);
        handles.push(thread::spawn(move || j.rollback(&e)));
    }

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    let ok_count = results.iter().filter(|r| r.is_ok()).count();
    let err_count = results.iter().filter(|r| r.is_err()).count();
    assert_eq!(
        ok_count, 1,
        "BUG: expected exactly 1 successful transition, got {ok_count}"
    );
    assert_eq!(
        err_count, 7,
        "BUG: expected exactly 7 AlreadyTerminal errors, got {err_count}"
    );

    for result in &results {
        if let Err(e) = result {
            assert!(
                matches!(e, EffectJournalError::AlreadyTerminal { .. }),
                "BUG: concurrent race produced unexpected error: {:?}",
                e
            );
        }
    }
}