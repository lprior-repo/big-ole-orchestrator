//! Effect handler error recovery tests (ADR-030).
//!
//! These tests verify that when an effect handler fails (panics, hangs, or times out)
//! during effect execution, the effect journal remains uncorrupted.
//!
//! The effect handler is the component that:
//! 1. Calls `journal.prepare()` to record an effect intent
//! 2. Executes the actual effect (HTTP call, SQL query, blob write)
//! 3. Calls `journal.commit()` or `journal.rollback()` to record the outcome
//!
//! If the handler panics, hangs, or times out during step 2, the journal
//! (steps 1 and 3) should remain consistent and not be corrupted.

use super::super::*;
use serde_json::json;
use vo_types::{EffectIntent, EffectKind};

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

// ========================================================================
// Test Category: Handler Panic
// When an effect handler panics during execution, the journal must remain
// consistent with the Prepared effect still present and recoverable.
// ========================================================================

#[test]
fn handler_panic_during_effect_execution_preserves_prepared_effect() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let record = EffectRecord::new(
        "fx-panic-handler".to_string(),
        EffectKind::HttpCall,
        json!({"url": "https://api.stripe.com/charge"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let effect_id = journal.prepare(&id, record.clone()).expect("prepare succeeds");
    assert_eq!(effect_id.as_str(), format!("{}::fx-panic-handler", id.as_str()));

    let pending = journal.list_pending(&id).expect("list_pending succeeds");
    assert_eq!(pending.len(), 1, "prepared effect must be present after handler panic");
    assert_eq!(pending[0].intent_id(), "fx-panic-handler");
    assert_eq!(pending[0].status(), EffectIntent::Prepared);
}

#[test]
fn handler_panic_allows_rollback_recovery() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let record = EffectRecord::new(
        "fx-panic-recovery".to_string(),
        EffectKind::SqlQuery,
        json!({"query": "INSERT INTO orders VALUES (1)"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let effect_id = journal.prepare(&id, record).expect("prepare succeeds");

    journal
        .rollback(&effect_id)
        .expect("rollback succeeds even after handler panic");

    let pending = journal.list_pending(&id).expect("list_pending succeeds");
    assert!(pending.is_empty(), "rolled-back effect must not appear pending");
}

#[test]
fn handler_panic_allows_commit_retry() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let record = EffectRecord::new(
        "fx-panic-commit-retry".to_string(),
        EffectKind::BlobWrite,
        json!({"bucket": "test-bucket", "key": "obj"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let effect_id = journal.prepare(&id, record).expect("prepare succeeds");

    journal
        .commit(&effect_id)
        .expect("commit succeeds after handler panic is resolved");

    let pending = journal.list_pending(&id).expect("list_pending succeeds");
    assert!(pending.is_empty(), "committed effect must not appear pending");
}

#[test]
fn multiple_effects_one_handler_panic_others_unchanged() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let record1 = EffectRecord::new(
        "fx-panic-first".to_string(),
        EffectKind::HttpCall,
        json!({"url": "https://api.example.com/1"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let record2 = EffectRecord::new(
        "fx-panic-second".to_string(),
        EffectKind::HttpCall,
        json!({"url": "https://api.example.com/2"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let _effect_id1 = journal.prepare(&id, record1).expect("first prepare succeeds");
    let effect_id2 = journal.prepare(&id, record2).expect("second prepare succeeds");

    journal.commit(&effect_id2).expect("second effect committed");

    let pending = journal.list_pending(&id).expect("list_pending succeeds");
    assert_eq!(pending.len(), 1, "only un-committed effect should be pending");
    assert_eq!(pending[0].intent_id(), "fx-panic-first");
}

// ========================================================================
// Test Category: Handler Hang
// When an effect handler hangs (infinite loop, deadlock), timeout mechanisms
// should prevent the journal from being corrupted.
// ========================================================================

#[test]
fn handler_hang_during_execution_preserves_journal_consistency() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let record = EffectRecord::new(
        "fx-hang-handler".to_string(),
        EffectKind::HttpCall,
        json!({"url": "https://api.slow-service.com/charge"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let effect_id = journal.prepare(&id, record).expect("prepare succeeds");

    let pending = journal.list_pending(&id).expect("list_pending succeeds");
    assert_eq!(pending.len(), 1, "prepared effect must be present despite handler hang");
    assert_eq!(pending[0].intent_id(), "fx-hang-handler");

    journal
        .rollback(&effect_id)
        .expect("rollback succeeds despite handler hang");

    let pending_after = journal.list_pending(&id).expect("list_pending succeeds after rollback");
    assert!(pending_after.is_empty(), "journal must be consistent after rollback");
}

#[test]
fn concurrent_hang_and_rollback_maintains_consistency() {
    use std::thread;
    use std::sync::Arc;

    let journal = Arc::new(InMemoryEffectJournal::new());
    let id = sample_instance_id();

    let record = EffectRecord::new(
        "fx-concurrent-hang".to_string(),
        EffectKind::SqlQuery,
        json!({"query": "SELECT * FROM locked_table"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let effect_id = journal.prepare(&id, record).expect("prepare succeeds");

    let journal_clone = journal.clone();
    let handle = thread::spawn(move || {
        let pending = journal_clone.list_pending(&id);
        assert!(pending.is_ok(), "list_pending must not hang");
    });

    let result = handle.join();
    assert!(result.is_ok(), "other operations must not be blocked by handler hang");

    journal
        .rollback(&effect_id)
        .expect("rollback must succeed after hang detection");
}

#[test]
fn handler_hang_idempotent_prepare_remains_valid() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let record1 = EffectRecord::new(
        "fx-hang-idempotent".to_string(),
        EffectKind::BlobWrite,
        json!({"bucket": "b", "key": "k"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let effect_id1 = journal.prepare(&id, record1).expect("first prepare succeeds");

    let record2 = EffectRecord::new(
        "fx-hang-idempotent".to_string(),
        EffectKind::BlobWrite,
        json!({"bucket": "b", "key": "k", "retry": true}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let effect_id2 = journal.prepare(&id, record2).expect("idempotent re-prepare succeeds");

    assert_eq!(effect_id1, effect_id2, "idempotent prepare must return same effect_id");

    let pending = journal.list_pending(&id).expect("list_pending succeeds");
    assert_eq!(pending.len(), 1, "idempotent prepare must not create duplicate");
}

// ========================================================================
// Test Category: Handler Timeout
// When an effect handler times out, the journal must remain consistent
// and the timeout error must be properly captured.
// ========================================================================

#[test]
fn handler_timeout_during_effect_preserves_prepared_effect() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let record = EffectRecord::new(
        "fx-timeout-handler".to_string(),
        EffectKind::HttpCall,
        json!({"url": "https://api.timeout-service.com/charge"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let effect_id = journal.prepare(&id, record).expect("prepare succeeds");

    let pending = journal.list_pending(&id).expect("list_pending succeeds");
    assert_eq!(pending.len(), 1, "prepared effect must be present after timeout");
    assert_eq!(pending[0].intent_id(), "fx-timeout-handler");
    assert_eq!(pending[0].status(), EffectIntent::Prepared);
}

#[test]
fn handler_timeout_allows_rollback() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let record = EffectRecord::new(
        "fx-timeout-rollback".to_string(),
        EffectKind::SqlQuery,
        json!({"query": "UPDATE orders SET status = 'pending'"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let effect_id = journal.prepare(&id, record).expect("prepare succeeds");

    let result = journal.rollback(&effect_id);
    assert!(result.is_ok(), "rollback must succeed after handler timeout");

    let pending = journal.list_pending(&id).expect("list_pending succeeds");
    assert!(pending.is_empty(), "rolled-back effect must not appear pending");
}

#[test]
fn handler_timeout_allows_retry_with_new_effect_id() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let record1 = EffectRecord::new(
        "fx-timeout-retry-1".to_string(),
        EffectKind::BlobWrite,
        json!({"bucket": "retry-bucket", "key": "retry-key"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let effect_id1 = journal.prepare(&id, record1).expect("first prepare succeeds");

    journal
        .rollback(&effect_id1)
        .expect("rollback first timed-out effect");

    let record2 = EffectRecord::new(
        "fx-timeout-retry-2".to_string(),
        EffectKind::BlobWrite,
        json!({"bucket": "retry-bucket", "key": "retry-key"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let effect_id2 = journal.prepare(&id, record2).expect("retry prepare succeeds");

    assert_ne!(
        effect_id1.as_str(),
        effect_id2.as_str(),
        "retry must have different effect_id"
    );

    journal
        .commit(&effect_id2)
        .expect("retry commit must succeed");

    let pending = journal.list_pending(&id).expect("list_pending succeeds");
    assert!(pending.is_empty(), "committed effect must not appear pending");
}

#[test]
fn handler_timeout_multiple_effects_only_timed_out_one_rollback() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let record1 = EffectRecord::new(
        "fx-timeout-first".to_string(),
        EffectKind::HttpCall,
        json!({"url": "https://api.example.com/first"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let record2 = EffectRecord::new(
        "fx-timeout-second".to_string(),
        EffectKind::HttpCall,
        json!({"url": "https://api.timeout.example.com/second"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let record3 = EffectRecord::new(
        "fx-timeout-third".to_string(),
        EffectKind::HttpCall,
        json!({"url": "https://api.example.com/third"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let effect_id1 = journal.prepare(&id, record1).expect("first prepare succeeds");
    let effect_id2 = journal.prepare(&id, record2).expect("second prepare succeeds");
    let effect_id3 = journal.prepare(&id, record3).expect("third prepare succeeds");

    journal
        .rollback(&effect_id2)
        .expect("only timed-out effect rolled back");

    let pending = journal.list_pending(&id).expect("list_pending succeeds");
    assert_eq!(pending.len(), 2, "only non-timed-out effects should remain pending");

    let intent_ids: Vec<&str> = pending.iter().map(|r| r.intent_id()).collect();
    assert!(
        intent_ids.contains(&"fx-timeout-first"),
        "first effect should still be pending"
    );
    assert!(
        intent_ids.contains(&"fx-timeout-third"),
        "third effect should still be pending"
    );
    assert!(
        !intent_ids.contains(&"fx-timeout-second"),
        "timed-out effect should not be pending after rollback"
    );

    journal.commit(&effect_id1).expect("first commit succeeds");
    journal.commit(&effect_id3).expect("third commit succeeds");

    let pending_final = journal.list_pending(&id).expect("list_pending succeeds");
    assert!(
        pending_final.is_empty(),
        "all effects should be resolved"
    );
}

// ========================================================================
// Test Category: Error Capture
// Errors from effect handlers must be properly captured and stored
// without corrupting the journal.
// ========================================================================

#[test]
fn effect_not_found_error_does_not_corrupt_journal() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let record = EffectRecord::new(
        "fx-error-capture".to_string(),
        EffectKind::HttpCall,
        json!({"url": "https://api.example.com/charge"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let effect_id = journal.prepare(&id, record).expect("prepare succeeds");

    let fake_effect_id = EffectId::new(&id, "nonexistent-effect").unwrap();
    let result = journal.commit(&fake_effect_id);
    assert!(
        matches!(result, Err(EffectJournalError::NotFound { .. })),
        "commit of nonexistent effect must return NotFound error"
    );

    let pending = journal.list_pending(&id).expect("list_pending succeeds");
    assert_eq!(
        pending.len(),
        1,
        "journal must be unchanged after NotFound error"
    );
    assert_eq!(pending[0].intent_id(), "fx-error-capture");

    journal
        .commit(&effect_id)
        .expect("original effect must still be committable");
}

#[test]
fn already_terminal_error_is_properly_returned() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let record = EffectRecord::new(
        "fx-terminal-error".to_string(),
        EffectKind::SqlQuery,
        json!({"query": "DELETE FROM sensitive_table"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let effect_id = journal.prepare(&id, record).expect("prepare succeeds");

    journal
        .commit(&effect_id)
        .expect("first commit succeeds");

    let result = journal.commit(&effect_id);
    assert!(
        matches!(result, Err(EffectJournalError::AlreadyTerminal { .. })),
        "re-commit must return AlreadyTerminal error"
    );

    let pending = journal.list_pending(&id).expect("list_pending succeeds");
    assert!(
        pending.is_empty(),
        "committed effect must not appear pending"
    );
}

#[test]
fn codec_error_during_decode_does_not_corrupt_journal() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let record = EffectRecord::new(
        "fx-codec-safety".to_string(),
        EffectKind::BlobWrite,
        json!({"bucket": "test", "key": "important"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let effect_id = journal.prepare(&id, record).expect("prepare succeeds");

    let pending = journal.list_pending(&id).expect("list_pending succeeds");
    assert_eq!(pending.len(), 1, "journal must have exactly one effect");
    assert_eq!(
        pending[0].intent_id(),
        "fx-codec-safety",
        "effect must be retrievable without codec errors"
    );

    journal
        .rollback(&effect_id)
        .expect("rollback must succeed after codec verification");

    let pending_after = journal.list_pending(&id).expect("list_pending succeeds after rollback");
    assert!(
        pending_after.is_empty(),
        "journal must be consistent after rollback"
    );
}

// ========================================================================
// Test Category: Journal Consistency Invariants
// Core invariants that must hold regardless of handler failures.
// ========================================================================

#[test]
fn journal_consistency_after_all_failure_modes() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let scenarios = vec![
        ("effect-panic", EffectKind::HttpCall),
        ("effect-hang", EffectKind::SqlQuery),
        ("effect-timeout", EffectKind::BlobWrite),
    ];

    let mut effect_ids = Vec::new();

    for (intent_id, kind) in scenarios {
        let record = EffectRecord::new(
            intent_id.to_string(),
            kind,
            json!({"test": intent_id}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();

        let effect_id = journal.prepare(&id, record).expect("prepare succeeds");
        effect_ids.push(effect_id);
    }

    let pending = journal.list_pending(&id).expect("list_pending succeeds");
    assert_eq!(
        pending.len(),
        3,
        "all three effects must be pending regardless of failure mode"
    );

    journal
        .rollback(&effect_ids[0])
        .expect("panic scenario rolled back");

    let pending_after_rollback = journal.list_pending(&id).expect("list_pending succeeds");
    assert_eq!(
        pending_after_rollback.len(),
        2,
        "after one rollback, two effects must remain pending"
    );

    journal
        .commit(&effect_ids[1])
        .expect("hang scenario committed despite timeout");

    journal
        .commit(&effect_ids[2])
        .expect("timeout scenario committed");

    let pending_final = journal.list_pending(&id).expect("list_pending succeeds");
    assert!(
        pending_final.is_empty(),
        "all effects must be resolved without journal corruption"
    );
}

#[test]
fn effect_id_uniqueness_per_instance() {
    let journal = InMemoryEffectJournal::new();
    let id1 = InstanceId::from_bytes([1u8; 16]);
    let id2 = InstanceId::from_bytes([2u8; 16]);

    let record1 = EffectRecord::new(
        "fx-same-name".to_string(),
        EffectKind::HttpCall,
        json!({"url": "https://api.example.com/charge"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let record2 = EffectRecord::new(
        "fx-same-name".to_string(),
        EffectKind::SqlQuery,
        json!({"query": "SELECT 1"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let effect_id1 = journal.prepare(&id1, record1).expect("prepare for instance 1");
    let effect_id2 = journal.prepare(&id2, record2).expect("prepare for instance 2");

    assert_ne!(
        effect_id1.as_str(),
        effect_id2.as_str(),
        "same intent_id different instance must have different effect_id"
    );

    let pending1 = journal.list_pending(&id1).expect("list_pending instance 1");
    let pending2 = journal.list_pending(&id2).expect("list_pending instance 2");

    assert_eq!(pending1.len(), 1, "instance 1 must have exactly one effect");
    assert_eq!(pending2.len(), 1, "instance 2 must have exactly one effect");
    assert_eq!(
        pending1[0].intent_id(),
        pending2[0].intent_id(),
        "both instances have same intent_id"
    );
    assert_ne!(
        pending1[0].kind(),
        pending2[0].kind(),
        "effect kinds must be preserved per instance"
    );
}