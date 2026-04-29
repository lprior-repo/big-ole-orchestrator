//! Corruption detection tests for effect_journal verification module.
//!
//! Proves that encode/decode functions catch malformed data.
//! These are unit tests that complement the Kani proofs in verification.rs.
//!
//! bead_id: tw-4454

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::super::*;

// ========================================================================
// decode_effect_key — corruption detection tests
// ========================================================================

#[test]
fn verify_decode_effect_key_rejects_empty_bytes() {
    let result = decode_effect_key(&[]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, EffectJournalError::Codec { .. }));
    assert!(err.to_string().contains("empty"));
}

#[test]
fn verify_decode_effect_key_rejects_non_utf8_bytes() {
    let malformed = vec![0xFF, 0xFE, 0xFD, 0xFC];
    let result = decode_effect_key(&malformed);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), EffectJournalError::Codec { .. }));
}

#[test]
fn verify_decode_effect_key_rejects_partial_utf8_continuation_bytes() {
    let partial = b"valid-prefix\x80\x80";
    let result = decode_effect_key(partial);
    assert!(result.is_err());
}

#[test]
fn verify_decode_effect_key_roundtrip_valid_key() {
    let instance_id = vo_types::InstanceId::from_bytes([0x42u8; 16]);
    let effect_id = EffectId::new(&instance_id, "test-intent-tw4454").unwrap();
    let encoded = encode_effect_key(&effect_id);
    let decoded = decode_effect_key(&encoded).unwrap();
    assert_eq!(decoded, effect_id);
}

#[test]
fn verify_decode_effect_key_roundtrip_special_chars_in_intent_id() {
    let instance_id = vo_types::InstanceId::from_bytes([0x99u8; 16]);
    let effect_id = EffectId::new(&instance_id, "intent:with:colons-underscore.dots").unwrap();
    let encoded = encode_effect_key(&effect_id);
    let decoded = decode_effect_key(&encoded).unwrap();
    assert_eq!(decoded, effect_id);
}

#[test]
fn verify_decode_effect_key_roundtrip_unicode_intent_id() {
    let instance_id = vo_types::InstanceId::from_bytes([0xABu8; 16]);
    let effect_id = EffectId::new(&instance_id, "intent-日本語-emoji-🎉").unwrap();
    let encoded = encode_effect_key(&effect_id);
    let decoded = decode_effect_key(&encoded).unwrap();
    assert_eq!(decoded, effect_id);
}

#[test]
fn verify_encode_effect_key_produces_valid_utf8() {
    let instance_id = vo_types::InstanceId::from_bytes([0xF0u8; 16]);
    let effect_id = EffectId::new(&instance_id, "verify-utf8-tw4454").unwrap();
    let bytes = encode_effect_key(&effect_id);
    let result = std::str::from_utf8(&bytes);
    assert!(result.is_ok(), "encode_effect_key must produce valid UTF-8");
    assert_eq!(result.unwrap(), effect_id.as_str());
}

// ========================================================================
// EffectId::new — boundary tests
// ========================================================================

#[test]
fn verify_effect_id_rejects_whitespace_only_intent_id() {
    let iid = vo_types::InstanceId::from_bytes([1u8; 16]);
    let result = EffectId::new(&iid, "   ");
    assert!(result.is_err());
}

#[test]
fn verify_effect_id_rejects_newline_in_intent_id() {
    let iid = vo_types::InstanceId::from_bytes([1u8; 16]);
    let result = EffectId::new(&iid, "intent\nwith\nnewlines");
    assert!(result.is_err());
}

#[test]
fn verify_effect_id_accepts_max_length_intent_id() {
    let iid = vo_types::InstanceId::from_bytes([1u8; 16]);
    let max_intent = "x".repeat(1000);
    let result = EffectId::new(&iid, &max_intent);
    assert!(result.is_ok());
}

// ========================================================================
// EffectJournalError — error string non-empty invariants
// ========================================================================

#[test]
fn verify_effect_journal_error_display_is_non_empty() {
    let err = EffectJournalError::InvalidArgument;
    let s = err.to_string();
    assert!(!s.is_empty(), "error display must be non-empty");

    let not_found = EffectJournalError::NotFound {
        effect_id: "fx-123".to_string(),
    };
    assert!(!not_found.to_string().is_empty());

    let storage = EffectJournalError::Storage {
        reason: "disk full".to_string(),
    };
    assert!(!storage.to_string().is_empty());
}

#[test]
fn verify_effect_journal_error_already_terminal_format() {
    let err = EffectJournalError::AlreadyTerminal {
        effect_id: "fx-abc".to_string(),
        current_status: "Committed".to_string(),
    };
    let s = err.to_string();
    assert!(s.contains("fx-abc"));
    assert!(s.contains("Committed"));
}