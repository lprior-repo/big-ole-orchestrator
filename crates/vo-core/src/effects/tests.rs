//! Transition and invariant unit tests for vo-core effect domain types.
//!
//! Tests cover:
//! - EffectIntent variant construction and properties
//! - Transition predicates (can_commit, can_rollback, is_terminal)
//! - Transition helper functions (commit_effect, rollback_effect)
//! - INV-EFF-001: An effect cannot transition to Committed without first being Prepared
//! - INV-EFF-002: Terminal states reject all transitions

use super::*;
use vo_types::EffectTransitionEvent;
use vo_types::TimestampMs;

fn make_effect(status: EffectIntent) -> EffectRecord {
    EffectRecord::new(
        "fx-test-001".to_string(),
        EffectKind::HttpCall,
        serde_json::json!({"url": "https://example.com"}),
        status,
        None,
    )
    .expect("valid test data")
}

// =============================================================================
// EffectIntent properties
// =============================================================================

#[test]
fn effectintent_prepared_is_not_terminal() {
    let effect = make_effect(EffectIntent::Prepared);
    assert!(!is_terminal(&effect));
}

#[test]
fn effectintent_committed_is_terminal() {
    let effect = make_effect(EffectIntent::Committed);
    assert!(is_terminal(&effect));
}

#[test]
fn effectintent_rolledback_is_terminal() {
    let effect = make_effect(EffectIntent::RolledBack);
    assert!(is_terminal(&effect));
}

// =============================================================================
// can_commit predicates
// =============================================================================

#[test]
fn can_commit_returns_true_when_prepared() {
    let effect = make_effect(EffectIntent::Prepared);
    assert!(can_commit(&effect));
}

#[test]
fn can_commit_returns_false_when_committed() {
    let effect = make_effect(EffectIntent::Committed);
    assert!(!can_commit(&effect));
}

#[test]
fn can_commit_returns_false_when_rolledback() {
    let effect = make_effect(EffectIntent::RolledBack);
    assert!(!can_commit(&effect));
}

// =============================================================================
// can_rollback predicates
// =============================================================================

#[test]
fn can_rollback_returns_true_when_prepared() {
    let effect = make_effect(EffectIntent::Prepared);
    assert!(can_rollback(&effect));
}

#[test]
fn can_rollback_returns_false_when_committed() {
    let effect = make_effect(EffectIntent::Committed);
    assert!(!can_rollback(&effect));
}

#[test]
fn can_rollback_returns_false_when_rolledback() {
    let effect = make_effect(EffectIntent::RolledBack);
    assert!(!can_rollback(&effect));
}

// =============================================================================
// commit_effect
// =============================================================================

#[test]
fn commit_effect_returns_committed_record_when_prepared() {
    let effect = make_effect(EffectIntent::Prepared);
    let now = TimestampMs::try_from(1_700_000_000_000u64).unwrap();
    let result = commit_effect(&effect, now);
    assert!(result.is_ok());
    let committed = result.unwrap();
    assert_eq!(committed.status(), EffectIntent::Committed);
    assert_eq!(committed.committed_at(), Some(&now));
}

#[test]
fn rollback_effect_returns_rolledback_record_when_prepared() {
    let effect = make_effect(EffectIntent::Prepared);
    let result = rollback_effect(&effect);
    assert!(result.is_ok());
    let rolled_back = result.unwrap();
    assert_eq!(rolled_back.status(), EffectIntent::RolledBack);
    assert_eq!(rolled_back.committed_at(), None);
}

// =============================================================================
// apply_effect_transition — Happy Paths
// =============================================================================

#[test]
fn apply_effect_transition_prepared_to_committed() {
    let result = apply_effect_transition(EffectIntent::Prepared, EffectTransitionEvent::Commit);
    assert_eq!(result, Ok(EffectIntent::Committed));
}

#[test]
fn apply_effect_transition_prepared_to_rolledback() {
    let result = apply_effect_transition(EffectIntent::Prepared, EffectTransitionEvent::Rollback);
    assert_eq!(result, Ok(EffectIntent::RolledBack));
}

// =============================================================================
// INV-EFF-001: Cannot transition to Committed without first being Prepared
// =============================================================================

#[test]
fn apply_effect_transition_committed_rejects_commit() {
    let result = apply_effect_transition(EffectIntent::Committed, EffectTransitionEvent::Commit);
    assert_eq!(result, Err(EffectTransitionError::TerminalStateTransition));
}

#[test]
fn apply_effect_transition_rolledback_rejects_commit() {
    let result = apply_effect_transition(EffectIntent::RolledBack, EffectTransitionEvent::Commit);
    assert_eq!(result, Err(EffectTransitionError::TerminalStateTransition));
}

#[test]
fn commit_effect_returns_error_when_committed() {
    let effect = make_effect(EffectIntent::Committed);
    let now = TimestampMs::try_from(1_700_000_000_000u64).unwrap();
    let result = commit_effect(&effect, now);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), EffectTransitionError::InvalidTransition);
}

#[test]
fn commit_effect_returns_error_when_rolledback() {
    let effect = make_effect(EffectIntent::RolledBack);
    let now = TimestampMs::try_from(1_700_000_000_000u64).unwrap();
    let result = commit_effect(&effect, now);
    assert!(result.is_err());
}

#[test]
fn rollback_effect_returns_error_when_committed() {
    let effect = make_effect(EffectIntent::Committed);
    let result = rollback_effect(&effect);
    assert!(result.is_err());
}

#[test]
fn rollback_effect_returns_error_when_rolledback() {
    let effect = make_effect(EffectIntent::RolledBack);
    let result = rollback_effect(&effect);
    assert!(result.is_err());
}

// =============================================================================
// INV-EFF-002: Terminal states reject all transitions
// =============================================================================

#[test]
fn apply_effect_transition_committed_rejects_rollback() {
    let result = apply_effect_transition(EffectIntent::Committed, EffectTransitionEvent::Rollback);
    assert_eq!(result, Err(EffectTransitionError::TerminalStateTransition));
}

#[test]
fn apply_effect_transition_rolledback_rejects_rollback() {
    let result = apply_effect_transition(EffectIntent::RolledBack, EffectTransitionEvent::Rollback);
    assert_eq!(result, Err(EffectTransitionError::TerminalStateTransition));
}

// =============================================================================
// EffectRecord construction validation
// =============================================================================

#[test]
fn effectrecord_returns_none_when_intent_id_is_empty() {
    let result = EffectRecord::new(
        "".to_string(),
        EffectKind::HttpCall,
        serde_json::json!({}),
        EffectIntent::Prepared,
        None,
    );
    assert!(result.is_none());
}

#[test]
fn effectrecord_returns_some_when_intent_id_is_single_char() {
    let result = EffectRecord::new(
        "a".to_string(),
        EffectKind::SqlQuery,
        serde_json::json!({"query": "SELECT 1"}),
        EffectIntent::Prepared,
        None,
    );
    assert!(result.is_some());
}

// =============================================================================
// EffectKind variants
// =============================================================================

#[test]
fn effect_kind_all_variants() {
    let variants = EffectKind::all_variants();
    assert_eq!(variants.len(), 3);
    assert!(variants.contains(&EffectKind::HttpCall));
    assert!(variants.contains(&EffectKind::SqlQuery));
    assert!(variants.contains(&EffectKind::BlobWrite));
}

// =============================================================================
// EffectIntent all_variants
// =============================================================================

#[test]
fn effectintent_all_variants() {
    let variants = EffectIntent::all_variants();
    assert_eq!(variants.len(), 3);
    assert!(variants.contains(&EffectIntent::Prepared));
    assert!(variants.contains(&EffectIntent::Committed));
    assert!(variants.contains(&EffectIntent::RolledBack));
}

// =============================================================================
// validate_commit_precondition
// =============================================================================

#[test]
fn validate_commit_precondition_ok_when_prepared() {
    let effect = make_effect(EffectIntent::Prepared);
    assert!(validate_commit_precondition(&effect).is_ok());
}

#[test]
fn validate_commit_precondition_err_when_committed() {
    let effect = make_effect(EffectIntent::Committed);
    assert!(validate_commit_precondition(&effect).is_err());
}

#[test]
fn validate_commit_precondition_err_when_rolledback() {
    let effect = make_effect(EffectIntent::RolledBack);
    assert!(validate_commit_precondition(&effect).is_err());
}