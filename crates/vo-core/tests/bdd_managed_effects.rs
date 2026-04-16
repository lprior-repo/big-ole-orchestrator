//! BDD tests for managed effect sink contracts (ADR-030).

use vo_core::effects::{commit_effect, is_terminal, rollback_effect};
use vo_types::effects::{EffectIntent, EffectKind, EffectRecord, Receipt};
use vo_types::TimestampMs;

fn make_effect(id: &str, status: EffectIntent) -> EffectRecord {
    EffectRecord::new(
        id.into(),
        EffectKind::HttpCall,
        serde_json::json!({}),
        status,
        None,
    )
    .expect("valid")
}

#[test]
fn given_prepared_effect_when_commit_then_commits_successfully() {
    let effect = make_effect("fx-001", EffectIntent::Prepared);
    let committed = commit_effect(&effect, TimestampMs::new_unchecked(1000));
    assert!(committed.is_ok());
    assert_eq!(
        committed.expect("committed").status(),
        EffectIntent::Committed
    );
}

#[test]
fn given_prepared_effect_when_rollback_then_rolls_back_successfully() {
    let effect = make_effect("fx-002", EffectIntent::Prepared);
    let result = rollback_effect(&effect);
    assert!(result.is_ok());
    assert_eq!(
        result.expect("rolled back").status(),
        EffectIntent::RolledBack
    );
}

#[test]
fn given_committed_effect_when_commit_then_returns_error() {
    let effect = make_effect("fx-003", EffectIntent::Committed);
    assert!(commit_effect(&effect, TimestampMs::new_unchecked(2000)).is_err());
}

#[test]
fn given_committed_effect_when_rollback_then_returns_error() {
    let effect = make_effect("fx-004", EffectIntent::Committed);
    assert!(rollback_effect(&effect).is_err());
}

#[test]
fn given_rolled_back_effect_when_commit_then_returns_error() {
    let effect = make_effect("fx-005", EffectIntent::RolledBack);
    assert!(commit_effect(&effect, TimestampMs::new_unchecked(3000)).is_err());
}

#[test]
fn given_rolled_back_effect_when_rollback_then_returns_error() {
    let effect = make_effect("fx-006", EffectIntent::RolledBack);
    assert!(rollback_effect(&effect).is_err());
}

#[test]
fn given_effect_when_checking_terminal_then_only_prepared_is_not_terminal() {
    let prepared = make_effect("fx-007", EffectIntent::Prepared);
    let committed = make_effect("fx-008", EffectIntent::Committed);
    let rolled_back = make_effect("fx-009", EffectIntent::RolledBack);
    assert!(!is_terminal(&prepared));
    assert!(is_terminal(&committed));
    assert!(is_terminal(&rolled_back));
}

#[test]
fn given_valid_data_when_receipt_created_then_succeeds() {
    let receipt = Receipt::new(
        "fx-010".into(),
        "stripe".into(),
        "v1".into(),
        serde_json::json!({}),
        TimestampMs::new_unchecked(4000),
    );
    assert!(receipt.is_some());
    let r = receipt.expect("valid");
    assert_eq!(r.effect_id(), "fx-010");
    assert_eq!(r.connector_type(), "stripe");
}

#[test]
fn given_empty_effect_id_when_receipt_created_then_returns_none() {
    assert!(Receipt::new(
        "".into(),
        "stripe".into(),
        "v1".into(),
        serde_json::json!({}),
        TimestampMs::new_unchecked(5000)
    )
    .is_none());
}

#[test]
fn given_empty_connector_type_when_receipt_created_then_returns_none() {
    assert!(Receipt::new(
        "fx-011".into(),
        "".into(),
        "v1".into(),
        serde_json::json!({}),
        TimestampMs::new_unchecked(6000)
    )
    .is_none());
}
