//! Red Queen tests — EffectId construction and shape.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::super::{EffectId, EffectJournalError};
use vo_types::InstanceId;

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
