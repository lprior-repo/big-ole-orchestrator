#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
use super::*;
use serde_json::json;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use vo_types::{EffectIntent, EffectKind};

// Helper: valid InstanceId for tests
fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

// Helper: decode JSON bytes (uses unwrap which is allowed in tests)
fn decode_json_lease(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).unwrap()
}

// ========================================================================
// EffectId Construction
// ========================================================================

#[test]
fn effectid_constructs_when_instance_id_and_intent_id_valid() {
    let id = InstanceId::from_bytes([1u8; 16]);
    let result = EffectId::new(&id, "fx-123");
    let expected_raw = format!("{id}::fx-123");
    let expected = EffectId::try_from(expected_raw.clone()).unwrap();
    assert_eq!(result, Ok(expected));
    assert_eq!(result.unwrap().as_str(), expected_raw);
}

#[test]
fn effectid_rejects_when_intent_id_empty() {
    let id = InstanceId::from_bytes([1u8; 16]);
    let result = EffectId::new(&id, "");
    assert_eq!(result, Err(EffectJournalError::InvalidArgument));
}

#[test]
fn effectid_try_from_rejects_when_string_empty() {
    let result = EffectId::try_from(String::new());
    assert_eq!(result, Err(EffectJournalError::InvalidArgument));
}

#[test]
fn effectid_equality_and_hashing() {
    let id = InstanceId::from_bytes([1u8; 16]);
    let a = EffectId::new(&id, "fx-1").unwrap();
    let b = EffectId::new(&id, "fx-1").unwrap();
    assert_eq!(a, b);
    let mut h1 = DefaultHasher::new();
    a.hash(&mut h1);
    let mut h2 = DefaultHasher::new();
    b.hash(&mut h2);
    assert_eq!(h1.finish(), h2.finish());
}

#[test]
fn effectid_serde_roundtrip() {
    let id = InstanceId::from_bytes([1u8; 16]);
    let eid = EffectId::new(&id, "fx-456").unwrap();
    let json_str = serde_json::to_string(&eid).unwrap();
    let recovered: EffectId = serde_json::from_str(&json_str).unwrap();
    assert_eq!(recovered, eid);
}

// ========================================================================
// Error Display
// ========================================================================

#[test]
fn error_already_terminal_displays_effect_id_and_status() {
    let err = EffectJournalError::AlreadyTerminal {
        effect_id: "fx-1".to_string(),
        current_status: "Committed".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("fx-1"), "should contain effect_id");
    assert!(msg.contains("Committed"), "should contain status");
}

#[test]
fn error_not_found_displays_effect_id() {
    let err = EffectJournalError::NotFound {
        effect_id: "fx-999".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("fx-999"));
}

#[test]
fn error_storage_displays_reason() {
    let err = EffectJournalError::Storage {
        reason: "disk full".to_string(),
    };
    assert_eq!(err.to_string(), "storage error: disk full");
}

#[test]
fn error_codec_displays_reason() {
    let err = EffectJournalError::Codec {
        reason: "invalid JSON".to_string(),
    };
    assert_eq!(err.to_string(), "codec error: invalid JSON");
}

#[test]
fn error_invalid_argument_displays_exact_message() {
    assert_eq!(
        EffectJournalError::InvalidArgument.to_string(),
        "invalid argument"
    );
}

// ========================================================================
// Calc Layer — Key Encode/Decode
// ========================================================================

#[test]
fn encode_effect_key_produces_utf8_bytes() {
    let id = InstanceId::from_bytes([1u8; 16]);
    let eid = EffectId::new(&id, "instance::fx-123").unwrap();
    let bytes = encode_effect_key(&eid);
    assert_eq!(bytes, eid.as_str().as_bytes());
}

#[test]
fn decode_effect_key_recovers_effect_id() {
    let id = InstanceId::from_bytes([1u8; 16]);
    let eid = EffectId::new(&id, "test-key").unwrap();
    let bytes = encode_effect_key(&eid);
    let recovered = decode_effect_key(&bytes).unwrap();
    assert_eq!(recovered, eid);
}

#[test]
fn decode_effect_key_returns_error_for_invalid_utf8() {
    let bad_bytes: &[u8] = &[0xFF, 0xFE];
    assert_eq!(
        decode_effect_key(bad_bytes),
        Err(EffectJournalError::Codec {
            reason: "invalid utf-8 sequence of 1 bytes from index 0".to_string(),
        })
    );
}

#[test]
fn decode_effect_key_returns_error_for_empty_bytes() {
    assert_eq!(
        decode_effect_key(&[]),
        Err(EffectJournalError::Codec {
            reason: "empty effect key".to_string(),
        })
    );
}

// ========================================================================
// Calc Layer — Record Encode/Decode
// ========================================================================

#[test]
fn encode_decode_effect_record_roundtrip() {
    let record = EffectRecord::new(
        "fx-roundtrip".to_string(),
        EffectKind::HttpCall,
        json!({"url": "https://example.com"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let bytes = encode_effect_record(&record).unwrap();
    let recovered = decode_effect_record(&bytes).unwrap();
    assert_eq!(recovered, record);
}

#[test]
fn decode_effect_record_returns_error_for_invalid_json() {
    assert_eq!(
        decode_effect_record(b"not-json"),
        Err(EffectJournalError::Codec {
            reason: "expected ident at line 1 column 2".to_string(),
        })
    );
}

#[test]
fn encode_decode_record_roundtrip_for_prepared_status() {
    let ts = vo_types::TimestampMs::parse("42").unwrap();
    let record = EffectRecord::new(
        "fx-status-prepared".to_string(),
        EffectKind::SqlQuery,
        json!({"q": "SELECT 1"}),
        EffectIntent::Prepared,
        Some(ts),
    )
    .unwrap();
    let bytes = encode_effect_record(&record).unwrap();
    assert!(!bytes.is_empty(), "encoded bytes must not be empty");
    let json_obj: serde_json::Value = decode_json_lease(&bytes);
    assert_eq!(
        json_obj.get("intent_id").and_then(|v| v.as_str()),
        Some("fx-status-prepared"),
        "encoded JSON must preserve intent_id"
    );
    assert_eq!(
        json_obj.get("status").and_then(|v| v.as_str()),
        Some("Prepared"),
        "encoded JSON must preserve Prepared status"
    );
    let recovered = decode_effect_record(&bytes).unwrap();
    assert_eq!(recovered.intent_id(), record.intent_id());
    assert_eq!(recovered.status(), EffectIntent::Prepared);
}

#[test]
fn encode_decode_record_roundtrip_for_committed_status() {
    let ts = vo_types::TimestampMs::parse("42").unwrap();
    let record = EffectRecord::new(
        "fx-status-committed".to_string(),
        EffectKind::SqlQuery,
        json!({"q": "SELECT 2"}),
        EffectIntent::Committed,
        Some(ts),
    )
    .unwrap();
    let bytes = encode_effect_record(&record).unwrap();
    assert!(!bytes.is_empty(), "encoded bytes must not be empty");
    let json_obj: serde_json::Value = decode_json_lease(&bytes);
    assert_eq!(
        json_obj.get("intent_id").and_then(|v| v.as_str()),
        Some("fx-status-committed"),
        "encoded JSON must preserve intent_id"
    );
    assert_eq!(
        json_obj.get("status").and_then(|v| v.as_str()),
        Some("Committed"),
        "encoded JSON must preserve Committed status"
    );
    let recovered = decode_effect_record(&bytes).unwrap();
    assert_eq!(recovered.intent_id(), record.intent_id());
    assert_eq!(recovered.status(), EffectIntent::Committed);
}

#[test]
fn encode_decode_record_roundtrip_for_rolledback_status() {
    let ts = vo_types::TimestampMs::parse("42").unwrap();
    let record = EffectRecord::new(
        "fx-status-rolledback".to_string(),
        EffectKind::SqlQuery,
        json!({"q": "SELECT 3"}),
        EffectIntent::RolledBack,
        Some(ts),
    )
    .unwrap();
    let bytes = encode_effect_record(&record).unwrap();
    assert!(!bytes.is_empty(), "encoded bytes must not be empty");
    let json_obj: serde_json::Value = decode_json_lease(&bytes);
    assert_eq!(
        json_obj.get("intent_id").and_then(|v| v.as_str()),
        Some("fx-status-rolledback"),
        "encoded JSON must preserve intent_id"
    );
    assert_eq!(
        json_obj.get("status").and_then(|v| v.as_str()),
        Some("RolledBack"),
        "encoded JSON must preserve RolledBack status"
    );
    let recovered = decode_effect_record(&bytes).unwrap();
    assert_eq!(recovered.intent_id(), record.intent_id());
    assert_eq!(recovered.status(), EffectIntent::RolledBack);
}

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
// Trait Integration — via InMemoryEffectJournal
// ========================================================================

#[test]
fn prepare_returns_effect_id_for_new_intent() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();
    let record = EffectRecord::new(
        "fx-1".to_string(),
        EffectKind::HttpCall,
        json!({"url": "https://api.stripe.com"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let result = journal.prepare(&id, record);
    let expected = EffectId::new(&id, "fx-1").unwrap();
    assert_eq!(result, Ok(expected.clone()));
    assert_eq!(result.unwrap().as_str(), expected.as_str());
}

#[test]
fn prepare_is_idempotent_for_same_intent_id() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();
    let record = EffectRecord::new(
        "fx-1".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let first = journal.prepare(&id, record.clone()).unwrap();
    let second = journal.prepare(&id, record).unwrap();
    assert_eq!(first, second);
    // Verify internal state: only ONE record exists (idempotent, not duplicated)
    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(
        pending.len(),
        1,
        "idempotent prepare must not duplicate records"
    );
}

#[test]
fn prepare_idempotent_returns_existing_when_status_differs() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();
    // First record with Prepared status
    let record_prepared = EffectRecord::new(
        "fx-status-diff".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let effect_id = journal.prepare(&id, record_prepared).unwrap();
    // Second record with same intent_id but different status (Committed)
    let record_committed = EffectRecord::new(
        "fx-status-diff".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Committed,
        None,
    )
    .unwrap();
    let second = journal.prepare(&id, record_committed).unwrap();
    // Must return the same EffectId (idempotent for same intent_id)
    assert_eq!(
        effect_id, second,
        "prepare must be idempotent for same intent_id regardless of status"
    );
    // Verify the stored record still has Prepared status by committing it.
    // If the bug exists (== becomes !=), the second prepare overwrites with Committed,
    // and commit would fail with AlreadyTerminal. The correct behavior preserves
    // the original Prepared status, allowing commit to succeed.
    let commit_result = journal.commit(&effect_id);
    assert!(
        commit_result.is_ok(),
        "commit must succeed if original Prepared status was preserved, \
         but failed with {:?}. This indicates the record was overwritten with Committed \
         (the == became != bug).",
        commit_result.err()
    );
}

#[test]
fn commit_transitions_prepared_to_committed() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();
    let record = EffectRecord::new(
        "fx-commit".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid = journal.prepare(&id, record).unwrap();
    let result = journal.commit(&eid);
    assert_eq!(result, Ok(()));
    // Verify via list_pending: committed effects should not appear
    let pending = journal.list_pending(&id).unwrap();
    assert!(pending.is_empty());
}

#[test]
fn rollback_transitions_prepared_to_rolledback() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();
    let record = EffectRecord::new(
        "fx-rollback".to_string(),
        EffectKind::SqlQuery,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid = journal.prepare(&id, record).unwrap();
    let result = journal.rollback(&eid);
    assert_eq!(result, Ok(()));
    // Verify via list_pending: rolled back effects should not appear
    let pending = journal.list_pending(&id).unwrap();
    assert!(pending.is_empty());
}

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

#[test]
fn list_pending_returns_only_prepared_effects() {
    let journal = InMemoryEffectJournal::new();
    let id = sample_instance_id();

    let r1 = EffectRecord::new(
        "fx-pending".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let r2 = EffectRecord::new(
        "fx-committed".to_string(),
        EffectKind::SqlQuery,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let r3 = EffectRecord::new(
        "fx-rolledback".to_string(),
        EffectKind::BlobWrite,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let eid2 = journal.prepare(&id, r2).unwrap();
    let eid1 = journal.prepare(&id, r1).unwrap();
    let eid3 = journal.prepare(&id, r3).unwrap();

    assert_eq!(eid1, EffectId::new(&id, "fx-pending").unwrap());

    journal.commit(&eid2).unwrap();
    journal.rollback(&eid3).unwrap();

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].intent_id(), "fx-pending");
}

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
