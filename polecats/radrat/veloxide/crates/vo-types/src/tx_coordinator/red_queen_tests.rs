//! Red Queen adversarial tests for tx_coordinator distributed transaction coordinator.
//!
//! bead_id: ve-wol
//! phase: Red Queen (adversarial testing)
//!
//! Dimensions attacked:
//!   - serde-attacks: Deserialize invalid states, boundary values, malformed JSON
//!   - exhaustiveness: All 120 (state, event) combinations covered
//!   - transition-attacks: Invalid state/event combinations, edge cases
//!   - invariant-attacks: INV-TC invariants tested to destruction
//!   - path-attacks: Malicious transition sequences that break invariants
//!   - error-taxonomy: All CoordinatorTransitionError variants exercised

use crate::tx_coordinator::{
    apply_coordinator_transition, CoordinatorDecision, CoordinatorTransition,
    CoordinatorTransitionError, ParticipantRecord, ParticipantStatus, ParticipantVote,
    TransactionRecord, TransactionState,
};

// ===========================================================================
// DIMENSION: serde-attacks
// Deserialize invalid states that bypass constructor validation
// ===========================================================================

/// RQ-TC-01: TransactionState serde round-trip preserves equality
#[test]
fn rq_transaction_state_serde_roundtrip_all_variants() {
    for state in TransactionState::all_variants() {
        let json = serde_json::to_string(state).unwrap();
        let recovered: TransactionState = serde_json::from_str(&json).unwrap();
        assert_eq!(
            state, &recovered,
            "TransactionState {:?} failed round-trip",
            state
        );
    }
}

/// RQ-TC-02: CoordinatorDecision serde round-trip
#[test]
fn rq_coordinator_decision_serde_roundtrip() {
    for decision in CoordinatorDecision::all_variants() {
        let json = serde_json::to_string(decision).unwrap();
        let recovered: CoordinatorDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(decision, &recovered);
    }
}

/// RQ-TC-03: CoordinatorTransition Debug format round-trips via Clone
/// Note: CoordinatorTransition does NOT derive Serialize/Deserialize
#[test]
fn rq_coordinator_transition_debug_clone_roundtrip() {
    for evt in CoordinatorTransition::all_variants() {
        let debug_str = format!("{:?}", evt);
        // Can't round-trip via serde, but verify Debug produces consistent output
        assert!(!debug_str.is_empty());
        // Clone preserves equality
        let cloned = evt.clone();
        assert_eq!(evt, &cloned);
    }
}

/// RQ-TC-04: ParticipantStatus serde round-trip
#[test]
fn rq_participant_status_serde_roundtrip_all_variants() {
    for status in ParticipantStatus::all_variants() {
        let json = serde_json::to_string(status).unwrap();
        let recovered: ParticipantStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, &recovered);
    }
}

/// RQ-TC-05: ParticipantVote serde round-trip
#[test]
fn rq_participant_vote_serde_roundtrip() {
    for vote in &[ParticipantVote::Prepared, ParticipantVote::Rollback] {
        let json = serde_json::to_string(vote).unwrap();
        let recovered: ParticipantVote = serde_json::from_str(&json).unwrap();
        assert_eq!(vote, &recovered);
    }
}

/// RQ-TC-06: TransactionRecord serde with empty transaction_id
/// INV-TC-001: new() returns None if transaction_id is empty
/// But serde doesn't call new() - it deserializes directly
#[test]
fn rq_transaction_record_empty_id_via_serde() {
    let json = r#"{
        "transaction_id": "",
        "state": "Init",
        "decision": null,
        "participants": [],
        "created_at": null,
        "prepared_at": null,
        "committed_at": null
    }"#;

    let record: TransactionRecord = serde_json::from_str(json).unwrap();
    assert_eq!(record.transaction_id(), "");
}

/// RQ-TC-07: ParticipantRecord serde with empty participant_id
/// INV-TC-002: new() returns None if participant_id is empty
/// But serde deserializes directly bypassing validation
#[test]
fn rq_participant_record_empty_id_via_serde() {
    let json = r#"{
        "participant_id": "",
        "status": "Enrolled",
        "vote": null
    }"#;

    let record: ParticipantRecord = serde_json::from_str(json).unwrap();
    assert_eq!(record.participant_id(), "");
}

/// RQ-TC-08: TransactionRecord serde with all states
#[test]
fn rq_transaction_record_all_states() {
    let states = [
        "Init",
        "Enrolling",
        "Preparing",
        "Prepared",
        "Committing",
        "Committed",
        "RollingBack",
        "RolledBack",
        "Aborted",
        "Ambiguous",
    ];

    for state in states {
        let json = format!(
            r#"{{
            "transaction_id": "tx-{}",
            "state": "{}",
            "decision": null,
            "participants": [],
            "created_at": null,
            "prepared_at": null,
            "committed_at": null
        }}"#,
            state, state
        );

        let record: TransactionRecord = serde_json::from_str(&json).unwrap();
        let expected: TransactionState = serde_json::from_str(&format!("\"{}\"", state)).unwrap();
        assert_eq!(record.state(), expected);
    }
}

/// RQ-TC-09: TransactionRecord serde with both decision variants
#[test]
fn rq_transaction_record_both_decisions() {
    for decision in &["\"Commit\"", "\"Rollback\""] {
        let json = format!(
            r#"{{
            "transaction_id": "tx-123",
            "state": "Prepared",
            "decision": {},
            "participants": [],
            "created_at": null,
            "prepared_at": null,
            "committed_at": null
        }}"#,
            decision
        );

        let record: TransactionRecord = serde_json::from_str(&json).unwrap();
        let expected: CoordinatorDecision = serde_json::from_str(decision).unwrap();
        assert_eq!(record.decision(), Some(expected));
    }
}

/// RQ-TC-10: ParticipantRecord serde with all status variants
#[test]
fn rq_participant_record_all_statuses() {
    let statuses = [
        "Enrolled",
        "Prepared",
        "VotedRollback",
        "Committed",
        "RolledBack",
        "Unknown",
    ];

    for status in statuses {
        let json = format!(
            r#"{{
            "participant_id": "p-1",
            "status": "{}",
            "vote": null
        }}"#,
            status
        );

        let record: ParticipantRecord = serde_json::from_str(&json).unwrap();
        let expected: ParticipantStatus = serde_json::from_str(&format!("\"{}\"", status)).unwrap();
        assert_eq!(record.status(), expected);
    }
}

/// RQ-TC-11: ParticipantRecord serde with both vote variants
#[test]
fn rq_participant_record_both_votes() {
    for vote in &["\"Prepared\"", "\"Rollback\""] {
        let json = format!(
            r#"{{
            "participant_id": "p-1",
            "status": "Prepared",
            "vote": {}
        }}"#,
            vote
        );

        let record: ParticipantRecord = serde_json::from_str(&json).unwrap();
        let expected: ParticipantVote = serde_json::from_str(vote).unwrap();
        assert_eq!(record.vote(), Some(expected));
    }
}

/// RQ-TC-12: CoordinatorTransitionError Debug output is non-empty
/// Note: CoordinatorTransitionError does NOT derive Serialize/Deserialize
#[test]
fn rq_transition_error_debug_is_nonempty() {
    let errors = [
        CoordinatorTransitionError::TerminalStateTransition,
        CoordinatorTransitionError::InvalidTransition,
        CoordinatorTransitionError::InsufficientVotes,
    ];

    for err in &errors {
        let debug_str = format!("{:?}", err);
        assert!(!debug_str.is_empty());
    }
}

// ===========================================================================
// DIMENSION: exhaustiveness
// All 120 (state, event) combinations must be handled without panic
// ===========================================================================

/// RQ-TC-13: All 10 states × 12 events = 120 combinations covered
#[test]
fn rq_all_state_event_combinations_tested() {
    let states = TransactionState::all_variants();
    let events = CoordinatorTransition::all_variants();

    assert_eq!(states.len(), 10, "Expected 10 TransactionState variants");
    assert_eq!(
        events.len(),
        12,
        "Expected 12 CoordinatorTransition variants"
    );
    assert_eq!(
        states.len() * events.len(),
        120,
        "Expected 120 combinations"
    );

    // Verify every combination is handled (returns Result, not panics)
    for &state in states {
        for &event in events {
            let result = std::panic::catch_unwind(|| apply_coordinator_transition(state, event));
            // Must not panic - must return Result
            assert!(
                result.is_ok(),
                "apply_coordinator_transition({:?}, {:?}) panicked",
                state,
                event
            );
        }
    }
}

/// RQ-TC-14: Each non-terminal state has at least one valid transition
#[test]
fn rq_non_terminal_states_have_valid_transitions() {
    let non_terminal = [
        TransactionState::Init,
        TransactionState::Enrolling,
        TransactionState::Preparing,
        TransactionState::Prepared,
        TransactionState::Committing,
        TransactionState::RollingBack,
        TransactionState::Ambiguous,
    ];
    let events = CoordinatorTransition::all_variants();

    for state in non_terminal {
        let valid_count = events
            .iter()
            .filter(|&e| apply_coordinator_transition(state, *e).is_ok())
            .count();

        assert!(
            valid_count > 0,
            "Non-terminal state {:?} has no valid transitions",
            state
        );
    }

    // Terminal states have 0 valid transitions (all events rejected)
    let terminal_states = [
        TransactionState::Committed,
        TransactionState::RolledBack,
        TransactionState::Aborted,
    ];

    for state in terminal_states {
        let valid_count = events
            .iter()
            .filter(|&e| apply_coordinator_transition(state, *e).is_ok())
            .count();
        assert_eq!(
            valid_count, 0,
            "Terminal state {:?} should have 0 valid transitions",
            state
        );
    }
}

/// RQ-TC-15: Each state rejects at least some events
#[test]
fn rq_each_state_rejects_some_events() {
    let states = TransactionState::all_variants();
    let events = CoordinatorTransition::all_variants();

    for &state in states {
        let invalid_count = events
            .iter()
            .filter(|&e| apply_coordinator_transition(state, *e).is_err())
            .count();

        assert!(
            invalid_count > 0,
            "State {:?} accepts ALL events - impossible",
            state
        );
    }
}

// ===========================================================================
// DIMENSION: invariant-attacks
// Test INV-TC invariants to destruction
// ===========================================================================

/// RQ-TC-16: INV-TC-003 Terminal states reject ALL 12 events
#[test]
fn rq_terminal_states_reject_all_events() {
    let terminal_states = [
        TransactionState::Committed,
        TransactionState::RolledBack,
        TransactionState::Aborted,
    ];

    for state in terminal_states {
        for &event in CoordinatorTransition::all_variants() {
            let result = apply_coordinator_transition(state, event);
            assert!(
                matches!(
                    result,
                    Err(CoordinatorTransitionError::TerminalStateTransition)
                ),
                "Terminal state {:?} should reject {:?} with TerminalStateTransition, got {:?}",
                state,
                event,
                result
            );
        }
    }
}

/// RQ-TC-17: INV-TC-004 Ambiguous is NOT terminal - accepts recovery transitions
#[test]
fn rq_ambiguous_not_terminal_accepts_recovery() {
    let state = TransactionState::Ambiguous;

    // Recovery transitions should work
    assert!(apply_coordinator_transition(state, CoordinatorTransition::ReconcileCommitted).is_ok());
    assert!(
        apply_coordinator_transition(state, CoordinatorTransition::ReconcileRolledBack).is_ok()
    );
    assert!(apply_coordinator_transition(state, CoordinatorTransition::ReconcileRetry).is_ok());

    // But it's not terminal, so Recover should also work (stays ambiguous)
    let result = apply_coordinator_transition(state, CoordinatorTransition::Recover);
    assert!(result.is_ok());
}

/// RQ-TC-18: INV-TC-005 Recover from any non-terminal state → Ambiguous
#[test]
fn rq_recover_transitions_to_ambiguous() {
    let non_terminal = [
        TransactionState::Init,
        TransactionState::Enrolling,
        TransactionState::Preparing,
        TransactionState::Prepared,
        TransactionState::Committing,
        TransactionState::RollingBack,
        TransactionState::Ambiguous,
    ];

    for state in non_terminal {
        let result = apply_coordinator_transition(state, CoordinatorTransition::Recover);
        assert_eq!(
            result,
            Ok(TransactionState::Ambiguous),
            "Recover from {:?} should transition to Ambiguous",
            state
        );
    }
}

/// RQ-TC-19: INV-TC-006 Timeout in Preparing → Aborted
#[test]
fn rq_preparing_timeout_aborts() {
    let result =
        apply_coordinator_transition(TransactionState::Preparing, CoordinatorTransition::Timeout);
    assert_eq!(result, Ok(TransactionState::Aborted));
}

/// RQ-TC-20: INV-TC-007 Timeout in Committing/RollingBack → Ambiguous
#[test]
fn rq_timeout_in_commit_or_rollback_ambiguous() {
    let result_commit =
        apply_coordinator_transition(TransactionState::Committing, CoordinatorTransition::Timeout);
    assert_eq!(result_commit, Ok(TransactionState::Ambiguous));

    let result_rollback = apply_coordinator_transition(
        TransactionState::RollingBack,
        CoordinatorTransition::Timeout,
    );
    assert_eq!(result_rollback, Ok(TransactionState::Ambiguous));
}

/// RQ-TC-21: INV-TC-008 Preparing absorbs ParticipantPrepared/ParticipantRollback
#[test]
fn rq_preparing_absorbs_votes() {
    let state = TransactionState::Preparing;

    let result1 = apply_coordinator_transition(state, CoordinatorTransition::ParticipantPrepared);
    assert_eq!(result1, Ok(TransactionState::Preparing));

    let result2 = apply_coordinator_transition(state, CoordinatorTransition::ParticipantRollback);
    assert_eq!(result2, Ok(TransactionState::Preparing));
}

/// RQ-TC-22: INV-TC-009 AllResponded valid from Preparing, Committing, RollingBack
#[test]
fn rq_allresponded_valid_from_three_states() {
    assert_eq!(
        apply_coordinator_transition(
            TransactionState::Preparing,
            CoordinatorTransition::AllResponded
        ),
        Ok(TransactionState::Prepared)
    );
    assert_eq!(
        apply_coordinator_transition(
            TransactionState::Committing,
            CoordinatorTransition::AllResponded
        ),
        Ok(TransactionState::Committed)
    );
    assert_eq!(
        apply_coordinator_transition(
            TransactionState::RollingBack,
            CoordinatorTransition::AllResponded
        ),
        Ok(TransactionState::RolledBack)
    );
}

/// RQ-TC-23: INV-TC-010/INV-TC-005 Prepared accepts specific events + Recover
/// Note: INV-TC-010 says Prepared only accepts DecideCommit, DecideRollback, Timeout.
/// But INV-TC-005 says Recover is valid from any non-terminal state.
/// The implementation follows INV-TC-005 (catch-all for Recover), so Prepared accepts Recover.
#[test]
fn rq_prepared_accepts_valid_events_and_recover() {
    let state = TransactionState::Prepared;
    let valid_events = [
        CoordinatorTransition::DecideCommit,
        CoordinatorTransition::DecideRollback,
        CoordinatorTransition::Timeout,
        CoordinatorTransition::Recover, // INV-TC-005: Recover from any non-terminal
    ];

    for event in valid_events {
        let result = apply_coordinator_transition(state, event);
        assert!(
            result.is_ok(),
            "Prepared should accept {:?}, got {:?}",
            event,
            result
        );
    }

    // Other events should be rejected
    let invalid_events: Vec<_> = CoordinatorTransition::all_variants()
        .iter()
        .filter(|e| !valid_events.contains(e))
        .collect();

    for event in invalid_events {
        let result = apply_coordinator_transition(state, *event);
        assert!(
            result.is_err(),
            "Prepared should reject {:?}, got {:?}",
            event,
            result
        );
    }
}

/// RQ-TC-24: INV-TC-011 All invalid combinations return InvalidTransition
#[test]
fn rq_invalid_transitions_return_invalid_error() {
    // Test a sampling of known-invalid (state, event) combinations
    let invalid_cases = [
        (TransactionState::Init, CoordinatorTransition::BeginPrepare),
        (TransactionState::Init, CoordinatorTransition::DecideCommit),
        (
            TransactionState::Enrolling,
            CoordinatorTransition::BeginEnroll,
        ),
        (
            TransactionState::Prepared,
            CoordinatorTransition::BeginEnroll,
        ),
        (
            TransactionState::Committing,
            CoordinatorTransition::DecideCommit,
        ),
    ];

    for (state, event) in invalid_cases {
        let result = apply_coordinator_transition(state, event);
        assert!(
            matches!(result, Err(CoordinatorTransitionError::InvalidTransition)),
            "({:?}, {:?}) should return InvalidTransition, got {:?}",
            state,
            event,
            result
        );
    }
}

/// RQ-TC-25: INV-TC-014 is_terminal completeness
#[test]
fn rq_is_terminal_completeness() {
    // Terminal states
    assert!(TransactionState::Committed.is_terminal());
    assert!(TransactionState::RolledBack.is_terminal());
    assert!(TransactionState::Aborted.is_terminal());

    // Non-terminal states
    assert!(!TransactionState::Init.is_terminal());
    assert!(!TransactionState::Enrolling.is_terminal());
    assert!(!TransactionState::Preparing.is_terminal());
    assert!(!TransactionState::Prepared.is_terminal());
    assert!(!TransactionState::Committing.is_terminal());
    assert!(!TransactionState::RollingBack.is_terminal());
    assert!(!TransactionState::Ambiguous.is_terminal());
}

/// RQ-TC-26: INV-TC-015 all_variants returns all variants in declaration order
#[test]
fn rq_all_variants_order() {
    let state_order = TransactionState::all_variants();
    assert_eq!(state_order.len(), 10);
    // Verify first few
    assert_eq!(state_order[0], TransactionState::Init);
    assert_eq!(state_order[1], TransactionState::Enrolling);
    assert_eq!(state_order[2], TransactionState::Preparing);

    let status_order = ParticipantStatus::all_variants();
    assert_eq!(status_order.len(), 6);
    assert_eq!(status_order[0], ParticipantStatus::Enrolled);

    let decision_order = CoordinatorDecision::all_variants();
    assert_eq!(decision_order.len(), 2);

    let transition_order = CoordinatorTransition::all_variants();
    assert_eq!(transition_order.len(), 12);
}

// ===========================================================================
// DIMENSION: transition-attacks
// Malicious sequences and edge cases
// ===========================================================================

/// RQ-TC-27: Happy path - full 2PC commit sequence
#[test]
fn rq_happy_path_two_phase_commit() {
    let state = TransactionState::Init;

    let s1 = apply_coordinator_transition(state, CoordinatorTransition::BeginEnroll).unwrap();
    assert_eq!(s1, TransactionState::Enrolling);

    let s2 = apply_coordinator_transition(s1, CoordinatorTransition::BeginPrepare).unwrap();
    assert_eq!(s2, TransactionState::Preparing);

    let s3 = apply_coordinator_transition(s2, CoordinatorTransition::ParticipantPrepared).unwrap();
    assert_eq!(s3, TransactionState::Preparing); // Stays

    let s4 = apply_coordinator_transition(s3, CoordinatorTransition::AllResponded).unwrap();
    assert_eq!(s4, TransactionState::Prepared);

    let s5 = apply_coordinator_transition(s4, CoordinatorTransition::DecideCommit).unwrap();
    assert_eq!(s5, TransactionState::Committing);

    let s6 = apply_coordinator_transition(s5, CoordinatorTransition::AllResponded).unwrap();
    assert_eq!(s6, TransactionState::Committed);

    // Now terminal - should reject everything
    let final_result = apply_coordinator_transition(s6, CoordinatorTransition::Recover);
    assert!(matches!(
        final_result,
        Err(CoordinatorTransitionError::TerminalStateTransition)
    ));
}

/// RQ-TC-28: Full 2PC rollback sequence
#[test]
fn rq_full_two_phase_rollback() {
    let state = TransactionState::Init;

    let s1 = apply_coordinator_transition(state, CoordinatorTransition::BeginEnroll).unwrap();
    let s2 = apply_coordinator_transition(s1, CoordinatorTransition::BeginPrepare).unwrap();
    let s3 = apply_coordinator_transition(s2, CoordinatorTransition::ParticipantRollback).unwrap();
    let s4 = apply_coordinator_transition(s3, CoordinatorTransition::AllResponded).unwrap();
    assert_eq!(s4, TransactionState::Prepared);

    let s5 = apply_coordinator_transition(s4, CoordinatorTransition::DecideRollback).unwrap();
    assert_eq!(s5, TransactionState::RollingBack);

    let s6 = apply_coordinator_transition(s5, CoordinatorTransition::AllResponded).unwrap();
    assert_eq!(s6, TransactionState::RolledBack);

    // Terminal - reject all
    assert!(matches!(
        apply_coordinator_transition(s6, CoordinatorTransition::BeginEnroll),
        Err(CoordinatorTransitionError::TerminalStateTransition)
    ));
}

/// RQ-TC-29: Timeout during prepare aborts
#[test]
fn rq_prepare_timeout_aborts() {
    let state = TransactionState::Init;
    let s1 = apply_coordinator_transition(state, CoordinatorTransition::BeginEnroll).unwrap();
    let s2 = apply_coordinator_transition(s1, CoordinatorTransition::BeginPrepare).unwrap();

    // Timeout before any participants responded
    let s3 = apply_coordinator_transition(s2, CoordinatorTransition::Timeout).unwrap();
    assert_eq!(s3, TransactionState::Aborted);
}

/// RQ-TC-30: Multiple ParticipantPrepared events keep state in Preparing
#[test]
fn rq_multiple_participant_prepared_stays_preparing() {
    let state = TransactionState::Preparing;

    for _ in 0..100 {
        let result =
            apply_coordinator_transition(state, CoordinatorTransition::ParticipantPrepared);
        assert_eq!(result, Ok(TransactionState::Preparing));
    }
}

/// RQ-TC-31: Multiple ParticipantRollback events keep state in Preparing
#[test]
fn rq_multiple_participant_rollback_stays_preparing() {
    let state = TransactionState::Preparing;

    for _ in 0..100 {
        let result =
            apply_coordinator_transition(state, CoordinatorTransition::ParticipantRollback);
        assert_eq!(result, Ok(TransactionState::Preparing));
    }
}

/// RQ-TC-32: Mixed votes still stay in Preparing
#[test]
fn rq_mixed_votes_stay_preparing() {
    let state = TransactionState::Preparing;

    let _ = apply_coordinator_transition(state, CoordinatorTransition::ParticipantPrepared);
    let _ = apply_coordinator_transition(state, CoordinatorTransition::ParticipantRollback);
    let _ = apply_coordinator_transition(state, CoordinatorTransition::ParticipantPrepared);
    let _ = apply_coordinator_transition(state, CoordinatorTransition::ParticipantRollback);

    // Still preparing - AllResponded needed to move forward
    let result = apply_coordinator_transition(state, CoordinatorTransition::ParticipantRollback);
    assert_eq!(result, Ok(TransactionState::Preparing));
}

/// RQ-TC-33: Ambiguous recovery - commit path
#[test]
fn rq_ambiguous_recovery_commit() {
    let state = TransactionState::Ambiguous;

    let result =
        apply_coordinator_transition(state, CoordinatorTransition::ReconcileCommitted).unwrap();
    assert_eq!(result, TransactionState::Committed);
}

/// RQ-TC-34: Ambiguous recovery - rollback path
#[test]
fn rq_ambiguous_recovery_rollback() {
    let state = TransactionState::Ambiguous;

    let result =
        apply_coordinator_transition(state, CoordinatorTransition::ReconcileRolledBack).unwrap();
    assert_eq!(result, TransactionState::RolledBack);
}

/// RQ-TC-35: Ambiguous recovery retry loop
#[test]
fn rq_ambiguous_recovery_retry_stays_ambiguous() {
    let state = TransactionState::Ambiguous;

    // Multiple retries should stay ambiguous
    for _ in 0..100 {
        let result =
            apply_coordinator_transition(state, CoordinatorTransition::ReconcileRetry).unwrap();
        assert_eq!(result, TransactionState::Ambiguous);
    }
}

/// RQ-TC-36: Crash recovery from all non-terminal states
#[test]
fn rq_crash_recovery_from_all_states() {
    let states = [
        TransactionState::Init,
        TransactionState::Enrolling,
        TransactionState::Preparing,
        TransactionState::Prepared,
        TransactionState::Committing,
        TransactionState::RollingBack,
    ];

    for state in states {
        let result = apply_coordinator_transition(state, CoordinatorTransition::Recover).unwrap();
        assert_eq!(result, TransactionState::Ambiguous);
    }
}

/// RQ-TC-37: Committing with timeout goes to Ambiguous (may have committed)
#[test]
fn rq_committing_timeout_goes_ambiguous() {
    let result =
        apply_coordinator_transition(TransactionState::Committing, CoordinatorTransition::Timeout)
            .unwrap();
    assert_eq!(result, TransactionState::Ambiguous);
}

/// RQ-TC-38: RollingBack with timeout goes to Ambiguous (may have rolled back)
#[test]
fn rq_rollingback_timeout_goes_ambiguous() {
    let result = apply_coordinator_transition(
        TransactionState::RollingBack,
        CoordinatorTransition::Timeout,
    )
    .unwrap();
    assert_eq!(result, TransactionState::Ambiguous);
}

// ===========================================================================
// DIMENSION: error-taxonomy
// All CoordinatorTransitionError variants exercised
// ===========================================================================

/// RQ-TC-39: TerminalStateTransition error display
#[test]
fn rq_terminal_state_transition_error_display() {
    let err = CoordinatorTransitionError::TerminalStateTransition;
    let display = err.to_string();
    assert!(!display.is_empty());
    assert!(display.contains("terminal") || display.contains("Cannot"));
}

/// RQ-TC-40: InvalidTransition error display
#[test]
fn rq_invalid_transition_error_display() {
    let err = CoordinatorTransitionError::InvalidTransition;
    let display = err.to_string();
    assert!(!display.is_empty());
    assert!(display.contains("Invalid") || display.contains("transition"));
}

/// RQ-TC-41: InsufficientVotes error display
#[test]
fn rq_insufficient_votes_error_display() {
    let err = CoordinatorTransitionError::InsufficientVotes;
    let display = err.to_string();
    assert!(!display.is_empty());
    assert!(display.contains("Insufficient") || display.contains("votes"));
}

/// RQ-TC-42: Error implements std::error::Error
#[test]
fn rq_transition_error_is_error_trait() {
    fn require_error<T: std::error::Error>() {}
    require_error::<CoordinatorTransitionError>();
}

// ===========================================================================
// DIMENSION: boundary-values
// Edge cases and boundary values
// ===========================================================================

/// RQ-TC-43: TransactionRecord with very long transaction_id
#[test]
fn rq_transaction_record_long_id() {
    let long_id = "x".repeat(10000);
    let record = TransactionRecord::new(
        long_id.clone(),
        TransactionState::Init,
        None,
        vec![],
        None,
        None,
        None,
    );
    assert!(record.is_some());
    assert_eq!(record.unwrap().transaction_id(), &long_id);
}

/// RQ-TC-44: TransactionRecord with unicode transaction_id
#[test]
fn rq_transaction_record_unicode_id() {
    let unicode_id = "事务_트랜잭션_🔐";
    let record = TransactionRecord::new(
        unicode_id.to_string(),
        TransactionState::Init,
        None,
        vec![],
        None,
        None,
        None,
    );
    assert!(record.is_some());
    assert_eq!(record.unwrap().transaction_id(), unicode_id);
}

/// RQ-TC-45: ParticipantRecord with very long participant_id
#[test]
fn rq_participant_record_long_id() {
    let long_id = "y".repeat(10000);
    let record = ParticipantRecord::new(long_id.clone(), ParticipantStatus::Enrolled, None);
    assert!(record.is_some());
    assert_eq!(record.unwrap().participant_id(), &long_id);
}

/// RQ-TC-46: TransactionRecord with many participants
#[test]
fn rq_transaction_record_many_participants() {
    let participants: Vec<ParticipantRecord> = (0..1000)
        .map(|i| {
            ParticipantRecord::new(
                format!("participant-{}", i),
                ParticipantStatus::Enrolled,
                None,
            )
            .unwrap()
        })
        .collect();

    let record = TransactionRecord::new(
        "tx-many".to_string(),
        TransactionState::Preparing,
        None,
        participants,
        None,
        None,
        None,
    );
    assert!(record.is_some());
    assert_eq!(record.unwrap().participants().len(), 1000);
}

/// RQ-TC-47: Empty participants list is valid
#[test]
fn rq_transaction_record_empty_participants() {
    let record = TransactionRecord::new(
        "tx-empty".to_string(),
        TransactionState::Init,
        None,
        vec![],
        None,
        None,
        None,
    );
    assert!(record.is_some());
    assert!(record.unwrap().participants().is_empty());
}

// ===========================================================================
// DIMENSION: path-attacks
// Malicious transition sequences
// ===========================================================================

/// RQ-TC-48: Try to skip enroll phase
#[test]
fn rq_skip_enroll_phase_rejected() {
    // Init -> Preparing (skipping Enrolling) should fail
    let result =
        apply_coordinator_transition(TransactionState::Init, CoordinatorTransition::BeginPrepare);
    assert!(result.is_err());
}

/// RQ-TC-49: Try to commit without prepare phase
#[test]
fn rq_commit_without_prepare_rejected() {
    // Init -> Committing directly should fail
    let result =
        apply_coordinator_transition(TransactionState::Init, CoordinatorTransition::DecideCommit);
    assert!(result.is_err());
}

/// RQ-TC-50: Try to decide before all responded
#[test]
fn rq_decide_before_all_responded_rejected() {
    // Preparing -> DecideCommit without AllResponded should fail
    let result = apply_coordinator_transition(
        TransactionState::Preparing,
        CoordinatorTransition::DecideCommit,
    );
    assert!(result.is_err());
}

/// RQ-TC-51: Try to go from Prepared back to Preparing
#[test]
fn rq_prepared_to_preparing_rejected() {
    let result = apply_coordinator_transition(
        TransactionState::Prepared,
        CoordinatorTransition::BeginPrepare,
    );
    assert!(result.is_err());
}

/// RQ-TC-52: Try to reconstitute from terminal state
#[test]
fn rq_recover_from_terminal_rejected() {
    let terminals = [
        TransactionState::Committed,
        TransactionState::RolledBack,
        TransactionState::Aborted,
    ];

    for state in terminals {
        let result = apply_coordinator_transition(state, CoordinatorTransition::Recover);
        assert!(matches!(
            result,
            Err(CoordinatorTransitionError::TerminalStateTransition)
        ));
    }
}

/// RQ-TC-53: Try to AllResponded from Init
#[test]
fn rq_allresponded_from_init_rejected() {
    let result =
        apply_coordinator_transition(TransactionState::Init, CoordinatorTransition::AllResponded);
    assert!(result.is_err());
}

/// RQ-TC-54: Try to AllResponded from Enrolling
#[test]
fn rq_allresponded_from_enrolling_rejected() {
    let result = apply_coordinator_transition(
        TransactionState::Enrolling,
        CoordinatorTransition::AllResponded,
    );
    assert!(result.is_err());
}

/// RQ-TC-55: Try to AllResponded from Prepared (should already have responded)
#[test]
fn rq_allresponded_from_prepared_rejected() {
    let result = apply_coordinator_transition(
        TransactionState::Prepared,
        CoordinatorTransition::AllResponded,
    );
    assert!(result.is_err());
}

/// RQ-TC-56: Rapid state oscillation attack
#[test]
fn rq_rapid_oscillation_preparing() {
    // Rapidly apply ParticipantPrepared/ParticipantRollback
    // State should remain stable in Preparing
    let mut state = TransactionState::Preparing;

    // Simple deterministic alternation
    for i in 0..1000 {
        if i % 2 == 0 {
            state = apply_coordinator_transition(state, CoordinatorTransition::ParticipantPrepared)
                .unwrap();
        } else {
            state = apply_coordinator_transition(state, CoordinatorTransition::ParticipantRollback)
                .unwrap();
        }
        assert_eq!(state, TransactionState::Preparing);
    }
}

/// RQ-TC-57: Timeout after Prepared does not go to Ambiguous (safe abort)
#[test]
fn rq_prepared_timeout_goes_aborted() {
    // Unlike Committing/RollingBack timeout (Ambiguous),
    // Prepared timeout goes to Aborted (safe - no participants committed)
    let result =
        apply_coordinator_transition(TransactionState::Prepared, CoordinatorTransition::Timeout)
            .unwrap();
    assert_eq!(result, TransactionState::Aborted);
}

/// RQ-TC-58: Ambiguous cannot exit via non-recovery events
#[test]
fn rq_ambiguous_cannot_exit_except_via_recovery() {
    let state = TransactionState::Ambiguous;
    let non_recovery_events = [
        CoordinatorTransition::BeginEnroll,
        CoordinatorTransition::BeginPrepare,
        CoordinatorTransition::ParticipantPrepared,
        CoordinatorTransition::ParticipantRollback,
        CoordinatorTransition::AllResponded,
        CoordinatorTransition::DecideCommit,
        CoordinatorTransition::DecideRollback,
        CoordinatorTransition::Timeout,
    ];

    for event in non_recovery_events {
        let result = apply_coordinator_transition(state, event);
        assert!(
            result.is_err(),
            "Ambiguous should reject {:?}, got {:?}",
            event,
            result
        );
    }

    // Only recovery transitions should work
    assert!(apply_coordinator_transition(state, CoordinatorTransition::ReconcileCommitted).is_ok());
}

/// RQ-TC-59: All state transition counts add up to 120
#[test]
fn rq_transition_matrix_cell_count() {
    let states = TransactionState::all_variants();
    let events = CoordinatorTransition::all_variants();

    let mut ok_count = 0;
    let mut err_count = 0;

    for &state in states {
        for &event in events {
            match apply_coordinator_transition(state, event) {
                Ok(_) => ok_count += 1,
                Err(_) => err_count += 1,
            }
        }
    }

    assert_eq!(ok_count + err_count, 120, "Total combinations must be 120");
    // Not all 120 can be valid - some are intentionally invalid
    assert!(
        ok_count > 0 && err_count > 0,
        "Must have mix of valid/invalid"
    );
}

/// RQ-TC-60: TransactionRecord serde round-trip with all participant statuses
#[test]
fn rq_transaction_record_serde_roundtrip() {
    let participants = vec![
        ParticipantRecord::new(
            "p-1".to_string(),
            ParticipantStatus::Enrolled,
            Some(ParticipantVote::Prepared),
        )
        .unwrap(),
        ParticipantRecord::new(
            "p-2".to_string(),
            ParticipantStatus::Prepared,
            Some(ParticipantVote::Prepared),
        )
        .unwrap(),
        ParticipantRecord::new("p-3".to_string(), ParticipantStatus::Committed, None).unwrap(),
    ];

    let record = TransactionRecord::new(
        "tx-123".to_string(),
        TransactionState::Prepared,
        Some(CoordinatorDecision::Commit),
        participants,
        None,
        None,
        None,
    )
    .unwrap();

    let json = serde_json::to_value(&record).unwrap();
    let restored: TransactionRecord = serde_json::from_value(json).unwrap();

    assert_eq!(restored.transaction_id(), record.transaction_id());
    assert_eq!(restored.state(), record.state());
    assert_eq!(restored.decision(), record.decision());
    assert_eq!(restored.participants().len(), 3);
}
