#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
use super::super::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// Helper: valid InstanceId for tests
fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
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
