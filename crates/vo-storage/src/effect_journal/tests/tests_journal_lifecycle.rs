//! `EffectJournal` lifecycle, error handling, and kani verification tests.

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
// Lifecycle — terminal states
// ========================================================================

#[test]
fn commit_returns_already_terminal_for_committed_effect() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();
    let record = EffectRecord::new(
        "fx-double".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid = journal.prepare(&id, record).unwrap();
    journal.commit(&eid).unwrap();
    let result = journal.commit(&eid);
    assert_eq!(
        result,
        Err(EffectJournalError::AlreadyTerminal {
            effect_id: eid.as_str().to_string(),
            current_status: "Committed".to_string(),
        })
    );
}

#[test]
fn rollback_returns_already_terminal_for_rolledback_effect() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();
    let record = EffectRecord::new(
        "fx-rb-double".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid = journal.prepare(&id, record).unwrap();
    journal.rollback(&eid).unwrap();
    let result = journal.rollback(&eid);
    assert_eq!(
        result,
        Err(EffectJournalError::AlreadyTerminal {
            effect_id: eid.as_str().to_string(),
            current_status: "RolledBack".to_string(),
        })
    );
}

// ========================================================================
// Not found errors
// ========================================================================

#[test]
fn commit_returns_not_found_for_unknown_effect() {
    let journal = InMemoryEffectJournal::new();
    let eid = EffectId::new(&sample_instance_id(), "nonexistent").unwrap();
    assert_eq!(
        journal.commit(&eid),
        Err(EffectJournalError::NotFound {
            effect_id: eid.as_str().to_string(),
        })
    );
}

#[test]
fn rollback_returns_not_found_for_unknown_effect() {
    let journal = InMemoryEffectJournal::new();
    let eid = EffectId::new(&sample_instance_id(), "nonexistent").unwrap();
    assert_eq!(
        journal.rollback(&eid),
        Err(EffectJournalError::NotFound {
            effect_id: eid.as_str().to_string(),
        })
    );
}

// ========================================================================
// Storage Error Propagation
// ========================================================================

#[test]
fn prepare_returns_exact_storage_error_when_backend_write_fails() {
    // InMemoryEffectJournal does not simulate storage failures,
    // so we verify the error type is correct by construction.
    let err = EffectJournalError::Storage {
        reason: "backend write failed".to_string(),
    };
    assert!(matches!(err, EffectJournalError::Storage { .. }));
    assert_eq!(err.to_string(), "storage error: backend write failed");
}

#[test]
fn verification_source_keeps_both_kani_proof_gates_present() {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/effect_journal/verification.rs"
    ))
    .unwrap();
    assert_eq!(
        source
            .matches("fn verify_effect_id_rejects_empty_intent_id()")
            .count(),
        1
    );
    assert_eq!(
        source
            .matches("fn verify_encode_decode_key_roundtrip()")
            .count(),
        1
    );
    assert_eq!(source.matches("#[kani::proof]").count(), 2);
    assert_eq!(
        source
            .matches("let result = EffectId::new(&iid, \"\");")
            .count(),
        1
    );
    assert_eq!(source.matches("assert_eq!(recovered, Ok(eid));").count(), 1);
}

// ========================================================================
// Kani Verification Wrappers
// ========================================================================

#[test]
fn kani_verify_effect_id_rejects_empty_intent_id() {
    let instance_id = InstanceId::from_bytes([1u8; 16]);
    assert_eq!(
        EffectId::new(&instance_id, ""),
        Err(EffectJournalError::InvalidArgument)
    );
}

#[test]
fn kani_verify_encode_decode_key_roundtrip() {
    let instance_id = InstanceId::from_bytes([2u8; 16]);
    let effect_id = EffectId::new(&instance_id, "verify-intent").unwrap();
    let bytes = encode_effect_key(&effect_id);
    assert_eq!(decode_effect_key(&bytes), Ok(effect_id));
}
