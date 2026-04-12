//! Unit tests for transaction coordinator types.

use crate::tx_coordinator::types::{
    apply_coordinator_transition, CoordinatorDecision, CoordinatorTransition,
    CoordinatorTransitionError, ParticipantRecord, ParticipantStatus, ParticipantVote,
    TransactionRecord, TransactionState,
};

#[test]
fn transaction_state_debug_format_equals_variant_name() {
    assert_eq!(format!("{:?}", TransactionState::Init), "Init");
    assert_eq!(format!("{:?}", TransactionState::Enrolling), "Enrolling");
    assert_eq!(format!("{:?}", TransactionState::Preparing), "Preparing");
    assert_eq!(format!("{:?}", TransactionState::Prepared), "Prepared");
    assert_eq!(format!("{:?}", TransactionState::Committing), "Committing");
    assert_eq!(format!("{:?}", TransactionState::Committed), "Committed");
    assert_eq!(
        format!("{:?}", TransactionState::RollingBack),
        "RollingBack"
    );
    assert_eq!(format!("{:?}", TransactionState::RolledBack), "RolledBack");
    assert_eq!(format!("{:?}", TransactionState::Aborted), "Aborted");
    assert_eq!(format!("{:?}", TransactionState::Ambiguous), "Ambiguous");
}

#[test]
fn transaction_state_is_terminal_returns_true_for_committed() {
    assert!(TransactionState::Committed.is_terminal());
}

#[test]
fn transaction_state_is_terminal_returns_true_for_rolled_back() {
    assert!(TransactionState::RolledBack.is_terminal());
}

#[test]
fn transaction_state_is_terminal_returns_true_for_aborted() {
    assert!(TransactionState::Aborted.is_terminal());
}

#[test]
fn transaction_state_is_terminal_returns_false_for_ambiguous() {
    assert!(!TransactionState::Ambiguous.is_terminal());
}

#[test]
fn transaction_state_is_terminal_returns_false_for_prepared() {
    assert!(!TransactionState::Prepared.is_terminal());
}

#[test]
fn coordinator_decision_debug_format_equals_variant_name() {
    assert_eq!(format!("{:?}", CoordinatorDecision::Commit), "Commit");
    assert_eq!(format!("{:?}", CoordinatorDecision::Rollback), "Rollback");
}

#[test]
fn participant_status_debug_format_equals_variant_name() {
    assert_eq!(format!("{:?}", ParticipantStatus::Enrolled), "Enrolled");
    assert_eq!(format!("{:?}", ParticipantStatus::Prepared), "Prepared");
    assert_eq!(
        format!("{:?}", ParticipantStatus::VotedRollback),
        "VotedRollback"
    );
    assert_eq!(format!("{:?}", ParticipantStatus::Committed), "Committed");
    assert_eq!(format!("{:?}", ParticipantStatus::RolledBack), "RolledBack");
    assert_eq!(format!("{:?}", ParticipantStatus::Unknown), "Unknown");
}

#[test]
fn participant_vote_debug_format_equals_variant_name() {
    assert_eq!(format!("{:?}", ParticipantVote::Prepared), "Prepared");
    assert_eq!(format!("{:?}", ParticipantVote::Rollback), "Rollback");
}

#[test]
fn coordinator_transition_error_terminal_state_transition_displays_correct_message() {
    let err = CoordinatorTransitionError::TerminalStateTransition;
    assert_eq!(
        err.to_string(),
        "Cannot transition from terminal transaction state"
    );
}

#[test]
fn coordinator_transition_error_invalid_transition_displays_correct_message() {
    let err = CoordinatorTransitionError::InvalidTransition;
    assert_eq!(
        err.to_string(),
        "Invalid transaction coordinator state transition"
    );
}

#[test]
fn coordinator_transition_error_insufficient_votes_displays_correct_message() {
    let err = CoordinatorTransitionError::InsufficientVotes;
    assert_eq!(
        err.to_string(),
        "Insufficient participant votes to transition"
    );
}

// ========================================================================
// apply_coordinator_transition — Happy Paths
// ========================================================================

#[test]
fn apply_coordinator_transition_init_to_enrolling() {
    let result =
        apply_coordinator_transition(TransactionState::Init, CoordinatorTransition::BeginEnroll);
    assert_eq!(result, Ok(TransactionState::Enrolling));
}

#[test]
fn apply_coordinator_transition_enrolling_to_preparing() {
    let result = apply_coordinator_transition(
        TransactionState::Enrolling,
        CoordinatorTransition::BeginPrepare,
    );
    assert_eq!(result, Ok(TransactionState::Preparing));
}

#[test]
fn apply_coordinator_transition_preparing_stays_preparing_on_participant_prepared() {
    let result = apply_coordinator_transition(
        TransactionState::Preparing,
        CoordinatorTransition::ParticipantPrepared,
    );
    assert_eq!(result, Ok(TransactionState::Preparing));
}

#[test]
fn apply_coordinator_transition_preparing_stays_preparing_on_participant_rollback() {
    let result = apply_coordinator_transition(
        TransactionState::Preparing,
        CoordinatorTransition::ParticipantRollback,
    );
    assert_eq!(result, Ok(TransactionState::Preparing));
}

#[test]
fn apply_coordinator_transition_preparing_to_prepared() {
    let result = apply_coordinator_transition(
        TransactionState::Preparing,
        CoordinatorTransition::AllResponded,
    );
    assert_eq!(result, Ok(TransactionState::Prepared));
}

#[test]
fn apply_coordinator_transition_preparing_to_aborted_on_timeout() {
    let result =
        apply_coordinator_transition(TransactionState::Preparing, CoordinatorTransition::Timeout);
    assert_eq!(result, Ok(TransactionState::Aborted));
}

#[test]
fn apply_coordinator_transition_prepared_to_committing() {
    let result = apply_coordinator_transition(
        TransactionState::Prepared,
        CoordinatorTransition::DecideCommit,
    );
    assert_eq!(result, Ok(TransactionState::Committing));
}

#[test]
fn apply_coordinator_transition_prepared_to_rolling_back() {
    let result = apply_coordinator_transition(
        TransactionState::Prepared,
        CoordinatorTransition::DecideRollback,
    );
    assert_eq!(result, Ok(TransactionState::RollingBack));
}

#[test]
fn apply_coordinator_transition_commiting_to_committed() {
    let result = apply_coordinator_transition(
        TransactionState::Committing,
        CoordinatorTransition::AllResponded,
    );
    assert_eq!(result, Ok(TransactionState::Committed));
}

#[test]
fn apply_coordinator_transition_commiting_to_ambiguous_on_timeout() {
    let result =
        apply_coordinator_transition(TransactionState::Committing, CoordinatorTransition::Timeout);
    assert_eq!(result, Ok(TransactionState::Ambiguous));
}

#[test]
fn apply_coordinator_transition_rolling_back_to_rolled_back() {
    let result = apply_coordinator_transition(
        TransactionState::RollingBack,
        CoordinatorTransition::AllResponded,
    );
    assert_eq!(result, Ok(TransactionState::RolledBack));
}

#[test]
fn apply_coordinator_transition_rolling_back_to_ambiguous_on_timeout() {
    let result = apply_coordinator_transition(
        TransactionState::RollingBack,
        CoordinatorTransition::Timeout,
    );
    assert_eq!(result, Ok(TransactionState::Ambiguous));
}

#[test]
fn apply_coordinator_transition_ambiguous_to_committed_on_reconcile() {
    let result = apply_coordinator_transition(
        TransactionState::Ambiguous,
        CoordinatorTransition::ReconcileCommitted,
    );
    assert_eq!(result, Ok(TransactionState::Committed));
}

#[test]
fn apply_coordinator_transition_ambiguous_to_rolled_back_on_reconcile() {
    let result = apply_coordinator_transition(
        TransactionState::Ambiguous,
        CoordinatorTransition::ReconcileRolledBack,
    );
    assert_eq!(result, Ok(TransactionState::RolledBack));
}

#[test]
fn apply_coordinator_transition_ambiguous_stays_ambiguous_on_retry() {
    let result = apply_coordinator_transition(
        TransactionState::Ambiguous,
        CoordinatorTransition::ReconcileRetry,
    );
    assert_eq!(result, Ok(TransactionState::Ambiguous));
}

// ========================================================================
// apply_coordinator_transition — Terminal Rejections
// ========================================================================

#[test]
fn apply_coordinator_transition_committed_rejects_all() {
    let result = apply_coordinator_transition(
        TransactionState::Committed,
        CoordinatorTransition::BeginEnroll,
    );
    assert_eq!(
        result,
        Err(CoordinatorTransitionError::TerminalStateTransition)
    );
}

#[test]
fn apply_coordinator_transition_rolled_back_rejects_all() {
    let result = apply_coordinator_transition(
        TransactionState::RolledBack,
        CoordinatorTransition::BeginEnroll,
    );
    assert_eq!(
        result,
        Err(CoordinatorTransitionError::TerminalStateTransition)
    );
}

#[test]
fn apply_coordinator_transition_aborted_rejects_all() {
    let result = apply_coordinator_transition(
        TransactionState::Aborted,
        CoordinatorTransition::BeginEnroll,
    );
    assert_eq!(
        result,
        Err(CoordinatorTransitionError::TerminalStateTransition)
    );
}

// ========================================================================
// apply_coordinator_transition — Invalid Transitions
// ========================================================================

#[test]
fn apply_coordinator_transition_init_rejects_participant_prepared() {
    let result = apply_coordinator_transition(
        TransactionState::Init,
        CoordinatorTransition::ParticipantPrepared,
    );
    assert_eq!(result, Err(CoordinatorTransitionError::InvalidTransition));
}

#[test]
fn apply_coordinator_transition_init_rejects_decide_commit() {
    let result =
        apply_coordinator_transition(TransactionState::Init, CoordinatorTransition::DecideCommit);
    assert_eq!(result, Err(CoordinatorTransitionError::InvalidTransition));
}

#[test]
fn apply_coordinator_transition_prepared_rejects_participant_prepared() {
    let result = apply_coordinator_transition(
        TransactionState::Prepared,
        CoordinatorTransition::ParticipantPrepared,
    );
    assert_eq!(result, Err(CoordinatorTransitionError::InvalidTransition));
}

#[test]
fn apply_coordinator_transition_commiting_rejects_decide_commit() {
    let result = apply_coordinator_transition(
        TransactionState::Committing,
        CoordinatorTransition::DecideCommit,
    );
    assert_eq!(result, Err(CoordinatorTransitionError::InvalidTransition));
}

// ========================================================================
// TransactionRecord Tests
// ========================================================================

#[test]
fn transaction_record_returns_some_when_constructed_with_valid_id() {
    let record = TransactionRecord::new(
        "tx-123".to_string(),
        TransactionState::Init,
        None,
        vec![],
        None,
        None,
        None,
    );
    assert!(record.is_some());
    let r = record.unwrap();
    assert_eq!(r.transaction_id(), "tx-123");
    assert_eq!(r.state(), TransactionState::Init);
    assert_eq!(r.decision(), None);
}

#[test]
fn transaction_record_returns_none_when_id_is_empty() {
    let record = TransactionRecord::new(
        "".to_string(),
        TransactionState::Init,
        None,
        vec![],
        None,
        None,
        None,
    );
    assert_eq!(record, None);
}

// ========================================================================
// ParticipantRecord Tests
// ========================================================================

#[test]
fn participant_record_returns_some_when_constructed_with_valid_id() {
    let record = ParticipantRecord::new("p-1".to_string(), ParticipantStatus::Enrolled, None);
    assert!(record.is_some());
    let r = record.unwrap();
    assert_eq!(r.participant_id(), "p-1");
    assert_eq!(r.status(), ParticipantStatus::Enrolled);
    assert_eq!(r.vote(), None);
}

#[test]
fn participant_record_returns_none_when_id_is_empty() {
    let record = ParticipantRecord::new("".to_string(), ParticipantStatus::Enrolled, None);
    assert_eq!(record, None);
}

// ========================================================================
// All Variants Tests
// ========================================================================

#[test]
fn transaction_state_all_variants_returns_ten_variants() {
    let variants = TransactionState::all_variants();
    assert_eq!(variants.len(), 10);
}

#[test]
fn participant_status_all_variants_returns_six_variants() {
    let variants = ParticipantStatus::all_variants();
    assert_eq!(variants.len(), 6);
}

#[test]
fn coordinator_decision_all_variants_returns_two_variants() {
    let variants = CoordinatorDecision::all_variants();
    assert_eq!(variants.len(), 2);
}

#[test]
fn coordinator_transition_all_variants_returns_twelve_variants() {
    let variants = CoordinatorTransition::all_variants();
    assert_eq!(variants.len(), 12);
}
