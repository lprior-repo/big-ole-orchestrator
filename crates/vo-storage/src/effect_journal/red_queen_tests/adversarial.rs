//! Red Queen adversarial tests — crash recovery, isolation, data corruption, concurrency.
//!
//! These tests simulate hostile conditions:
//! - Crash recovery: prepare without commit/rollback leaves pending effects recoverable
//! - Cross-instance isolation: effects from one instance never leak into another
//! - Data corruption: corrupted keys and records are rejected gracefully
//! - Concurrent access: multiple threads hammer the same journal
//! - Idempotency stress: repeated prepare with different params preserves original
//! - Boundary conditions: unicode, very long intent_ids, special JSON shapes

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::super::{EffectJournal, InMemoryEffectJournal, InstanceId};
use serde_json::json;
use vo_types::{EffectIntent, EffectKind};

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

// ========================================================================
// DIMENSION: crash-recovery
// Contract: after prepare (simulate crash), list_pending recovers all Prepared effects
// ========================================================================

#[test]
fn red_queen_crash_after_prepare_recovers_all_pending_effects() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    // Simulate a workflow that prepared 5 effects then "crashed" (no commit/rollback)
    for i in 0..5u32 {
        let record = vo_types::EffectRecord::new(
            format!("fx-crash-{i}"),
            EffectKind::HttpCall,
            json!({"step": i}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        journal.prepare(&id, record).unwrap();
    }

    // Recovery: list_pending should find ALL 5
    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(
        pending.len(),
        5,
        "BUG: crash recovery lost prepared effects"
    );

    // Verify each effect can still be committed (not corrupted)
    let mut recovered_ids: Vec<String> =
        pending.iter().map(|r| r.intent_id().to_string()).collect();
    recovered_ids.sort();
    for (i, name) in recovered_ids.iter().enumerate() {
        assert_eq!(
            name,
            &format!("fx-crash-{i}"),
            "BUG: recovered effect has wrong intent_id"
        );
    }
}

#[test]
fn red_queen_crash_recovery_can_commit_each_recovered_effect() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    // Prepare 3 effects, simulate crash
    for i in 0..3u32 {
        let record = vo_types::EffectRecord::new(
            format!("fx-recover-{i}"),
            EffectKind::SqlQuery,
            json!({"q": format!("SELECT {i}")}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        journal.prepare(&id, record).unwrap();
    }

    // Recover and commit each one individually
    let pending = journal.list_pending(&id).unwrap();
    for record in &pending {
        let eid = super::super::EffectId::new(&id, record.intent_id()).unwrap();
        journal.commit(&eid).unwrap();
    }

    // After committing all, pending should be empty
    let after = journal.list_pending(&id).unwrap();
    assert!(
        after.is_empty(),
        "BUG: pending not empty after recovering all effects"
    );
}

#[test]
fn red_queen_crash_recovery_can_rollback_each_recovered_effect() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    // Prepare 3 effects, simulate crash
    for i in 0..3u32 {
        let record = vo_types::EffectRecord::new(
            format!("fx-rb-recover-{i}"),
            EffectKind::BlobWrite,
            json!({"key": format!("blob-{i}")}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        journal.prepare(&id, record).unwrap();
    }

    // Recover and rollback each one
    let pending = journal.list_pending(&id).unwrap();
    for record in &pending {
        let eid = super::super::EffectId::new(&id, record.intent_id()).unwrap();
        journal.rollback(&eid).unwrap();
    }

    let after = journal.list_pending(&id).unwrap();
    assert!(
        after.is_empty(),
        "BUG: pending not empty after rolling back all"
    );
}

#[test]
fn red_queen_partial_commit_then_crash_recovers_remaining() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    // Prepare 5 effects
    for i in 0..5u32 {
        let record = vo_types::EffectRecord::new(
            format!("fx-partial-{i}"),
            EffectKind::HttpCall,
            json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        journal.prepare(&id, record).unwrap();
    }

    // Commit 2, then "crash" (simulate partial completion)
    journal
        .commit(&super::super::EffectId::new(&id, "fx-partial-0").unwrap())
        .unwrap();
    journal
        .commit(&super::super::EffectId::new(&id, "fx-partial-1").unwrap())
        .unwrap();

    // Recovery: should find exactly 3 remaining
    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(
        pending.len(),
        3,
        "BUG: partial commit recovery returned wrong count"
    );

    // Already-committed effects should reject commit again
    let double = journal.commit(&super::super::EffectId::new(&id, "fx-partial-0").unwrap());
    assert!(
        double.is_err(),
        "BUG: double-commit after crash recovery succeeded"
    );
}

// ========================================================================
// DIMENSION: cross-instance-isolation
// Contract: effects from one instance NEVER appear in another's pending list
// ========================================================================

#[test]
fn red_queen_cross_instance_isolation_pending_lists() {
    let journal = InMemoryEffectJournal::new();
    let id_a = sample_instance_id();
    let id_b = InstanceId::from_bytes([2u8; 16]);

    // Prepare effects for instance A
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

    // Prepare effects for instance B
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

    // A's pending list must only contain A's effects
    let pending_a = journal.list_pending(&id_a).unwrap();
    assert_eq!(pending_a.len(), 3, "BUG: instance A pending count wrong");
    for r in &pending_a {
        assert!(
            r.intent_id().starts_with("fx-a-"),
            "BUG: instance B effect leaked into instance A's pending list: {}",
            r.intent_id()
        );
    }

    // B's pending list must only contain B's effects
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

// ========================================================================
// DIMENSION: idempotency-stress
// Contract: repeated prepare with different params/kinds preserves original record
// ========================================================================

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

// ========================================================================
// DIMENSION: data-corruption-codec
// Contract: corrupted keys and records are rejected with Codec errors
// ========================================================================

#[test]
fn red_queen_decode_key_rejects_truncated_utf8() {
    let truncated = vec![0xF0, 0x9F];
    let result = super::super::decode_effect_key(&truncated);
    assert!(result.is_err(), "BUG: accepted truncated UTF-8");
    assert!(
        matches!(
            result.unwrap_err(),
            super::super::EffectJournalError::Codec { .. }
        ),
        "BUG: wrong error variant for truncated UTF-8"
    );
}

#[test]
fn red_queen_decode_record_rejects_truncated_json() {
    let truncated = b"{\"intent_id\": \"fx-1\", \"kind";
    let result = super::super::decode_effect_record(truncated);
    assert!(result.is_err(), "BUG: accepted truncated JSON");
    assert!(
        matches!(
            result.unwrap_err(),
            super::super::EffectJournalError::Codec { .. }
        ),
        "BUG: wrong error variant for truncated JSON"
    );
}

#[test]
fn red_queen_decode_record_rejects_valid_json_wrong_type() {
    let wrong_type = b"42";
    let result = super::super::decode_effect_record(wrong_type);
    assert!(
        result.is_err(),
        "BUG: accepted JSON integer as EffectRecord"
    );
}

#[test]
fn red_queen_decode_record_rejects_json_array() {
    let arr = b"[1, 2, 3]";
    let result = super::super::decode_effect_record(arr);
    assert!(result.is_err(), "BUG: accepted JSON array as EffectRecord");
}

#[test]
fn red_queen_decode_record_rejects_empty_json_object() {
    let empty = b"{}";
    let result = super::super::decode_effect_record(empty);
    assert!(
        result.is_err(),
        "BUG: accepted empty JSON object as EffectRecord"
    );
}

#[test]
fn red_queen_decode_record_rejects_null_bytes() {
    let nulls = vec![0u8; 100];
    let result = super::super::decode_effect_record(&nulls);
    assert!(result.is_err(), "BUG: accepted null bytes as EffectRecord");
}

// ========================================================================
// DIMENSION: concurrent-access
// Contract: multiple threads can safely use the journal without data races
// ========================================================================

use std::sync::Arc;
use std::thread;

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

    let mut effect_ids: Vec<super::super::EffectId> =
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
                matches!(e, super::super::EffectJournalError::AlreadyTerminal { .. }),
                "BUG: concurrent race produced unexpected error: {:?}",
                e
            );
        }
    }
}

// ========================================================================
// DIMENSION: boundary-conditions
// Contract: unicode, long intent_ids, special JSON shapes, all EffectKinds
// ========================================================================

#[test]
fn red_queen_effectid_with_unicode_intent_id() {
    let id = sample_instance_id();
    let unicode_intents = vec!["café", "日本語", "emoji🦀", "مرحبا", "naïve", "ß"];

    for intent in unicode_intents {
        let eid = super::super::EffectId::new(&id, intent).unwrap();
        assert_eq!(eid.as_str(), format!("{id}::{intent}"));

        let bytes = super::super::encode_effect_key(&eid);
        let recovered = super::super::decode_effect_key(&bytes).unwrap();
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
    let eid = super::super::EffectId::new(&id, &long_intent).unwrap();

    let bytes = super::super::encode_effect_key(&eid);
    assert_eq!(bytes.len(), long_intent.len() + id.to_string().len() + 2);
    let recovered = super::super::decode_effect_key(&bytes).unwrap();
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
    let eid = super::super::EffectId::new(&id, intent).unwrap();

    let as_str = eid.as_str();
    assert!(as_str.contains("has::delimiters::inside"));

    let bytes = super::super::encode_effect_key(&eid);
    let recovered = super::super::decode_effect_key(&bytes).unwrap();
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

// ========================================================================
// DIMENSION: state-machine-exhaustive
// Contract: every invalid transition is rejected, every valid transition succeeds
// ========================================================================

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

    // Valid: Prepared -> Committed
    assert!(journal.commit(&eid).is_ok());

    // Invalid: Committed -> Committed
    assert!(matches!(
        journal.commit(&eid),
        Err(super::super::EffectJournalError::AlreadyTerminal { .. })
    ));

    // Invalid: Committed -> RolledBack
    assert!(matches!(
        journal.rollback(&eid),
        Err(super::super::EffectJournalError::AlreadyTerminal { .. })
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

    // Valid: Prepared -> RolledBack
    assert!(journal.rollback(&eid).is_ok());

    // Invalid: RolledBack -> Committed
    assert!(matches!(
        journal.commit(&eid),
        Err(super::super::EffectJournalError::AlreadyTerminal { .. })
    ));

    // Invalid: RolledBack -> RolledBack
    assert!(matches!(
        journal.rollback(&eid),
        Err(super::super::EffectJournalError::AlreadyTerminal { .. })
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

// ========================================================================
// DIMENSION: partition-constant-integrity
// Contract: EFFECTS_PARTITION is non-empty and used consistently
// ========================================================================

#[test]
fn red_queen_effects_partition_is_nonempty_utf8() {
    assert!(
        !super::super::EFFECTS_PARTITION.is_empty(),
        "BUG: EFFECTS_PARTITION is empty"
    );
    assert!(
        super::super::EFFECTS_PARTITION
            .chars()
            .all(|c| !c.is_control()),
        "BUG: EFFECTS_PARTITION contains control characters"
    );
}

#[test]
fn red_queen_effects_partition_no_leading_trailing_whitespace() {
    assert_eq!(
        super::super::EFFECTS_PARTITION,
        super::super::EFFECTS_PARTITION.trim(),
        "BUG: EFFECTS_PARTITION has leading/trailing whitespace"
    );
}

// ========================================================================
// DIMENSION: compact
// Contract: compact removes ONLY terminal Committed effects with committed_at < threshold.
// RolledBack effects (committed_at = None) are NEVER compacted.
// Prepared effects are NEVER compacted.
// ========================================================================

fn ts(n: u64) -> vo_types::TimestampMs {
    vo_types::TimestampMs::parse(&n.to_string()).unwrap()
}

#[test]
fn red_queen_compact_removes_committed_effects_older_than_threshold() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    // Create 5 committed effects directly with known timestamp (100)
    for i in 0..5u32 {
        let record = vo_types::EffectRecord::new(
            format!("fx-compact-{i}"),
            EffectKind::HttpCall,
            json!({}),
            EffectIntent::Committed,
            Some(ts(100)),
        )
        .unwrap();
        journal.prepare(&id, record).unwrap();
    }

    // Compact with threshold > 100 should remove all 5
    let removed = journal.compact(ts(200)).unwrap();
    assert_eq!(
        removed, 5,
        "BUG: compact should remove all 5 committed effects"
    );

    // Verify pending is empty (no effects remain)
    let pending = journal.list_pending(&id).unwrap();
    assert!(pending.is_empty(), "BUG: compact left pending effects");
}

#[test]
fn red_queen_compact_preserves_prepared_effects() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    // Prepare 3 effects, commit none
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

    // Compact with very high threshold — should remove nothing
    let removed = journal.compact(ts(99999)).unwrap();
    assert_eq!(removed, 0, "BUG: compact removed prepared effects");

    // All 3 should still be pending
    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending.len(), 3, "BUG: compact destroyed prepared effects");
}

#[test]
fn red_queen_compact_preserves_committed_effects_newer_than_threshold() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    // Prepare and commit effects (committed_at = 100)
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

    // Compact with threshold < 100 — nothing should be removed
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

    // Prepare and rollback 3 effects
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

    // Compact with very high threshold — RolledBack has committed_at = None, so never compacted
    let removed = journal.compact(ts(99999)).unwrap();
    assert_eq!(
        removed, 0,
        "BUG: compact removed rolledback effects (committed_at is None)"
    );

    // Verify rolledback effects reject commit (still exist and are terminal)
    let eid = super::super::EffectId::new(&id, "fx-rollback-0").unwrap();
    let result = journal.commit(&eid);
    assert!(
        result.is_err(),
        "BUG: rolledback effect was destroyed by compact"
    );
    assert!(
        matches!(
            result.unwrap_err(),
            super::super::EffectJournalError::AlreadyTerminal { .. }
        ),
        "BUG: wrong error after compact touched rolledback effect"
    );
}

#[test]
fn red_queen_compact_mixed_states_removes_only_committed() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    // 2 Prepared, 2 Committed (with timestamp 100), 2 RolledBack
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
            EffectIntent::Committed,
            Some(ts(100)),
        )
        .unwrap();
        journal.prepare(&id, record).unwrap();
    }
    for i in 0..2u32 {
        let record = vo_types::EffectRecord::new(
            format!("fx-mixed-rolledback-{i}"),
            EffectKind::BlobWrite,
            json!({}),
            EffectIntent::RolledBack,
            None,
        )
        .unwrap();
        journal.prepare(&id, record).unwrap();
    }

    let removed = journal.compact(ts(200)).unwrap();
    assert_eq!(
        removed, 2,
        "BUG: compact should remove exactly the 2 committed effects"
    );

    // Verify: 2 prepared still pending
    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending.len(), 2, "BUG: compact destroyed prepared effects");

    // Verify: 2 rolledback still exist and are terminal
    let rb0 = journal.rollback(&super::super::EffectId::new(&id, "fx-mixed-rolledback-0").unwrap());
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
        EffectIntent::Committed,
        Some(ts(100)),
    )
    .unwrap();
    journal.prepare(&id, record).unwrap();

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

    // Instance A: prepare + commit
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

    // Instance B: prepare (no commit)
    let record_b = vo_types::EffectRecord::new(
        "fx-b-no-compact".to_string(),
        EffectKind::SqlQuery,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    journal.prepare(&id_b, record_b).unwrap();

    // Compact — should only remove A's committed effect
    journal.compact(ts(200)).unwrap();

    // B's effect should still be pending
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

    // Create committed effect directly with timestamp 100
    let record = vo_types::EffectRecord::new(
        "fx-boundary".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Committed,
        Some(ts(100)),
    )
    .unwrap();
    journal.prepare(&id, record).unwrap();

    // Compact with threshold exactly equal to committed_at — contract is strict less-than
    let removed = journal.compact(ts(100)).unwrap();
    assert_eq!(
        removed, 0,
        "BUG: compact with threshold == committed_at removed effect (should be strict <)"
    );

    // Compact with threshold one above — should remove
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

    // Prepare 3 effects
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

    // Compact (should remove nothing — all prepared)
    journal.compact(ts(99999)).unwrap();

    // "Crash" recovery: all 3 should still be recoverable
    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(
        pending.len(),
        3,
        "BUG: compact destroyed pending effects before crash recovery"
    );

    // Each should still be committable
    for record in &pending {
        let eid = super::super::EffectId::new(&id, record.intent_id()).unwrap();
        journal.commit(&eid).unwrap();
    }
}

#[test]
fn red_queen_concurrent_compact_and_prepare() {
    use std::sync::Arc;
    use std::thread;

    let journal = Arc::new(InMemoryEffectJournal::new());
    let id = sample_instance_id();

    // Pre-populate some committed effects
    for i in 0..10u32 {
        let record = vo_types::EffectRecord::new(
            format!("fx-conc-compact-{i}"),
            EffectKind::HttpCall,
            json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let eid = journal.prepare(&id, record).unwrap();
        journal.commit(&eid).unwrap();
    }

    let mut handles: Vec<thread::JoinHandle<()>> = Vec::new();

    // Thread 1: compact
    {
        let j = Arc::clone(&journal);
        handles.push(thread::spawn(move || {
            j.compact(ts(200)).unwrap();
        }));
    }

    // Threads 2-5: prepare new effects
    for i in 0..4u32 {
        let j = Arc::clone(&journal);
        let id = id.clone();
        handles.push(thread::spawn(move || {
            let record = vo_types::EffectRecord::new(
                format!("fx-conc-new-{i}"),
                EffectKind::SqlQuery,
                json!({}),
                EffectIntent::Prepared,
                None,
            )
            .unwrap();
            j.prepare(&id, record).unwrap();
        }));
    }

    // Wait for all
    for h in handles {
        h.join().unwrap();
    }

    // New effects should still be pending
    let pending = journal.list_pending(&id).unwrap();
    let new_pending: Vec<_> = pending
        .iter()
        .filter(|r| r.intent_id().starts_with("fx-conc-new-"))
        .collect();
    assert_eq!(
        new_pending.len(),
        4,
        "BUG: concurrent compact removed newly prepared effects"
    );
}
