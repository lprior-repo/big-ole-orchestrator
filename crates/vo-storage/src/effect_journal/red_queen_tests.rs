//! Red Queen adversarial tests for effect_journal
//! These tests attempt to find bugs by violating contracts and testing edge cases.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::{
    decode_effect_key, decode_effect_record, encode_effect_key, encode_effect_record, EffectId,
    EffectJournal, EffectJournalError, InMemoryEffectJournal, EFFECTS_PARTITION,
};
use serde_json::json;
use vo_types::{EffectIntent, EffectKind, InstanceId};

// Helper
fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

// ========================================================================
// DIMENSION: effectid-construction
// Contract: EffectId::new rejects empty intent_id, TryFrom<String> rejects empty string
// ========================================================================

#[test]
fn red_queen_effectid_rejects_empty_intent_id_direct() {
    // This is the core precondition: empty intent_id must be rejected
    let id = InstanceId::from_bytes([1u8; 16]);
    let result = EffectId::new(&id, "");
    assert!(
        result.is_err(),
        "BUG: EffectId::new accepted empty intent_id"
    );
    assert!(
        matches!(result.unwrap_err(), EffectJournalError::InvalidArgument),
        "BUG: Wrong error variant for empty intent_id"
    );
}

#[test]
fn red_queen_try_from_empty_string_rejects() {
    let result = EffectId::try_from(String::new());
    assert!(
        result.is_err(),
        "BUG: EffectId::try_from accepted empty string"
    );
    assert!(
        matches!(result.unwrap_err(), EffectJournalError::InvalidArgument),
        "BUG: Wrong error variant for empty string TryFrom"
    );
}

// ========================================================================
// DIMENSION: effectid-shape
// Contract: EffectId::new produces "<instance_id>::<intent_id>" shape
// ========================================================================

#[test]
fn red_queen_effectid_new_produces_correct_delimiter() {
    let id = InstanceId::from_bytes([1u8; 16]);
    let effect_id = EffectId::new(&id, "test-intent").unwrap();
    let as_str = effect_id.as_str();
    // Must contain exactly one "::" delimiter at the right position
    assert!(
        as_str.contains("::"),
        "BUG: EffectId does not contain :: delimiter"
    );
    let parts: Vec<&str> = as_str.split("::").collect();
    assert_eq!(
        parts.len(),
        2,
        "BUG: EffectId contains multiple :: delimiters"
    );
}

#[test]
fn red_queen_effectid_try_from_preserves_any_nonempty_string() {
    // Contract says: TryFrom<String> does NOT validate delimiter shape
    // So "not-a-ulid::whatever" should be accepted
    let cases = vec![
        "not-a-ulid::fx-123",
        "no-delimiter",
        "multiple::colons::in::intent",
        "🦀 rust 🦀", // Unicode
    ];
    for s in cases {
        let result = EffectId::try_from(s.to_string());
        assert!(
            result.is_ok(),
            "BUG: EffectId::try_from rejected valid string: {}",
            s
        );
        assert_eq!(result.unwrap().as_str(), s);
    }
}

#[test]
fn red_queen_effectid_as_str_returns_exact_string() {
    let id = InstanceId::from_bytes([1u8; 16]);
    let effect_id = EffectId::new(&id, "my-intent").unwrap();
    let from: String = effect_id.clone().into();
    assert_eq!(
        from,
        effect_id.as_str(),
        "BUG: From<EffectId> for String doesn't match as_str()"
    );
}

// ========================================================================
// DIMENSION: key-codec
// Contract: encode/decode roundtrip, invalid UTF-8 returns Codec error
// ========================================================================

#[test]
fn red_queen_encode_decode_key_roundtrip_preserves_id() {
    let id = InstanceId::from_bytes([1u8; 16]);
    let effect_id = EffectId::new(&id, "roundtrip-test").unwrap();
    let bytes = encode_effect_key(&effect_id);
    let recovered = decode_effect_key(&bytes).unwrap();
    assert_eq!(
        recovered, effect_id,
        "BUG: key codec roundtrip changed EffectId"
    );
}

#[test]
fn red_queen_decode_effect_key_rejects_invalid_utf8() {
    let bad_bytes: Vec<u8> = vec![0x80, 0x81, 0xFF, 0xFE];
    let result = decode_effect_key(&bad_bytes);
    assert!(
        result.is_err(),
        "BUG: decode_effect_key accepted invalid UTF-8"
    );
    match result.unwrap_err() {
        EffectJournalError::Codec { .. } => {} // Expected
        other => panic!("BUG: Wrong error variant for invalid UTF-8: {:?}", other),
    }
}

#[test]
fn red_queen_decode_effect_key_rejects_empty_bytes() {
    let result = decode_effect_key(&[]);
    assert!(
        result.is_err(),
        "BUG: decode_effect_key accepted empty bytes"
    );
    match result.unwrap_err() {
        EffectJournalError::Codec { reason } => {
            assert!(
                reason.contains("empty"),
                "BUG: Empty key error message doesn't mention 'empty'"
            );
        }
        other => panic!("BUG: Wrong error variant for empty bytes: {:?}", other),
    }
}

#[test]
fn red_queen_encode_effect_key_produces_utf8() {
    let id = InstanceId::from_bytes([1u8; 16]);
    let effect_id = EffectId::new(&id, "utf8-test").unwrap();
    let bytes = encode_effect_key(&effect_id);
    // Must be valid UTF-8
    let s = String::from_utf8(bytes.clone()).expect("BUG: encode_effect_key produced non-UTF-8");
    assert_eq!(
        s,
        effect_id.as_str(),
        "BUG: UTF-8 encoding doesn't match original"
    );
}

// ========================================================================
// DIMENSION: record-codec
// Contract: encode/decode roundtrip preserves EffectRecord
// ========================================================================

#[test]
fn red_queen_encode_decode_record_roundtrip() {
    let record = vo_types::EffectRecord::new(
        "fx-record".to_string(),
        EffectKind::HttpCall,
        json!({"url": "https://example.com"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let bytes = encode_effect_record(&record).expect("BUG: encode failed");
    let recovered = decode_effect_record(&bytes).expect("BUG: decode failed");
    assert_eq!(
        recovered, record,
        "BUG: record codec roundtrip changed EffectRecord"
    );
}

#[test]
fn red_queen_decode_effect_record_rejects_invalid_json() {
    let result = decode_effect_record(b"not json at all");
    assert!(
        result.is_err(),
        "BUG: decode_effect_record accepted invalid JSON"
    );
    match result.unwrap_err() {
        EffectJournalError::Codec { .. } => {} // Expected
        other => panic!("BUG: Wrong error variant for invalid JSON: {:?}", other),
    }
}

// ========================================================================
// DIMENSION: journal-lifecycle
// Contract: prepare->commit->AlreadyTerminal, prepare->rollback->AlreadyTerminal
// ========================================================================

#[test]
fn red_queen_commit_then_rollback_returns_already_terminal() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();
    let record = vo_types::EffectRecord::new(
        "fx-commit-then-rb".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid = journal.prepare(&id, record).unwrap();

    // Commit first
    journal.commit(&eid).expect("BUG: commit failed");

    // Rollback should fail with AlreadyTerminal
    let result = journal.rollback(&eid);
    assert!(
        result.is_err(),
        "BUG: rollback succeeded after commit (should be AlreadyTerminal)"
    );
    match result.unwrap_err() {
        EffectJournalError::AlreadyTerminal {
            effect_id,
            current_status,
        } => {
            assert_eq!(effect_id, eid.as_str());
            assert!(
                current_status.contains("Committed"),
                "BUG: status not Committed"
            );
        }
        other => panic!("BUG: Wrong error variant for commit+rollback: {:?}", other),
    }
}

#[test]
fn red_queen_rollback_then_commit_returns_already_terminal() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();
    let record = vo_types::EffectRecord::new(
        "fx-rb-then-commit".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid = journal.prepare(&id, record).unwrap();

    // Rollback first
    journal.rollback(&eid).expect("BUG: rollback failed");

    // Commit should fail with AlreadyTerminal
    let result = journal.commit(&eid);
    assert!(
        result.is_err(),
        "BUG: commit succeeded after rollback (should be AlreadyTerminal)"
    );
    match result.unwrap_err() {
        EffectJournalError::AlreadyTerminal {
            effect_id,
            current_status,
        } => {
            assert_eq!(effect_id, eid.as_str());
            assert!(
                current_status.contains("RolledBack"),
                "BUG: status not RolledBack"
            );
        }
        other => panic!("BUG: Wrong error variant for rollback+commit: {:?}", other),
    }
}

#[test]
fn red_queen_prepare_validates_intent_id_non_empty() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();
    // Create a record with valid (non-empty) intent_id
    let record = vo_types::EffectRecord::new(
        "valid-intent".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap(); // This succeeds because intent_id is non-empty

    // prepare should succeed with valid input
    let result = journal.prepare(&id, record);
    assert!(
        result.is_ok(),
        "BUG: prepare rejected valid record with non-empty intent_id"
    );

    // Now verify that EffectRecord::new correctly rejects empty intent_id (contract enforcement)
    let empty_record_result = vo_types::EffectRecord::new(
        "".to_string(), // Empty intent_id
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    );
    assert!(
        empty_record_result.is_none(),
        "BUG: EffectRecord::new accepted empty intent_id"
    );
    // This proves the contract layer (EffectRecord) and calc layer (EffectId::new) both validate
}

// ========================================================================
// DIMENSION: journal-idempotency
// Contract: prepare is idempotent - same intent_id returns same EffectId
// ========================================================================

#[test]
fn red_queen_prepare_idempotent_different_status_preserves_original() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    // First prepare with Prepared status
    let record_prepared = vo_types::EffectRecord::new(
        "fx-idempotent".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid1 = journal.prepare(&id, record_prepared).unwrap();

    // Second prepare with Committed status (same intent_id)
    let record_committed = vo_types::EffectRecord::new(
        "fx-idempotent".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Committed, // Different status
        None,
    )
    .unwrap();
    let eid2 = journal.prepare(&id, record_committed).unwrap();

    // Must return same EffectId
    assert_eq!(
        eid1, eid2,
        "BUG: prepare returned different EffectId for same intent_id"
    );

    // And the stored record must still be Prepared (not overwritten with Committed)
    // This is verified by successfully committing
    let commit_result = journal.commit(&eid1);
    assert!(
        commit_result.is_ok(),
        "BUG: commit failed after idempotent prepare - original status was overwritten"
    );
}

#[test]
fn red_queen_list_pending_only_returns_prepared() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    // Prepare 3 effects
    for i in 0..3 {
        let record = vo_types::EffectRecord::new(
            format!("fx-pending-{}", i),
            EffectKind::HttpCall,
            json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        journal.prepare(&id, record).unwrap();
    }

    // Commit one, rollback another
    let eid_committed = EffectId::new(&id, "fx-pending-0").unwrap();
    let eid_rolledback = EffectId::new(&id, "fx-pending-1").unwrap();
    journal.commit(&eid_committed).unwrap();
    journal.rollback(&eid_rolledback).unwrap();

    // list_pending should return only the one still Prepared
    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending.len(), 1, "BUG: list_pending returned wrong count");
    assert_eq!(pending[0].intent_id(), "fx-pending-2");
}

// ========================================================================
// DIMENSION: journal-not-found
// Contract: commit/rollback on unknown effect returns NotFound
// ========================================================================

#[test]
fn red_queen_commit_unknown_effect_returns_not_found() {
    let journal = InMemoryEffectJournal::new();
    let eid = EffectId::new(&sample_instance_id(), "nonexistent").unwrap();
    let result = journal.commit(&eid);
    assert!(result.is_err(), "BUG: commit succeeded for unknown effect");
    assert!(
        matches!(result.unwrap_err(), EffectJournalError::NotFound { .. }),
        "BUG: Wrong error variant for commit on unknown"
    );
}

#[test]
fn red_queen_rollback_unknown_effect_returns_not_found() {
    let journal = InMemoryEffectJournal::new();
    let eid = EffectId::new(&sample_instance_id(), "nonexistent").unwrap();
    let result = journal.rollback(&eid);
    assert!(
        result.is_err(),
        "BUG: rollback succeeded for unknown effect"
    );
    assert!(
        matches!(result.unwrap_err(), EffectJournalError::NotFound { .. }),
        "BUG: Wrong error variant for rollback on unknown"
    );
}

// ========================================================================
// DIMENSION: partition-constant
// Contract: EFFECTS_PARTITION == "effects"
// ========================================================================

#[test]
fn red_queen_effects_partition_constant_is_correct() {
    assert_eq!(
        EFFECTS_PARTITION, "effects",
        "BUG: EFFECTS_PARTITION has wrong value"
    );
}
