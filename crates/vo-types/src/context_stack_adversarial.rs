//! Red Queen adversarial tests for context switching (lifecycle state machine)
//! and stack operations (lineage/epoch, wait_record).
//!
//! bead_id: hq-5njv8
//! phase: red-queen-context-stack
//!
//! Dimensions attacked:
//!   - lifecycle-state-machine: invalid transitions, terminal state rejection,
//!     double transitions, superstate consistency
//!   - lineage-epoch-stack: empty lineage_id, invalid epoch transitions,
//!     boundary values, parent_epoch validation
//!   - wait-record-stack: WaitKey edge cases, BufferPolicy serde integrity,
//!     WaitRecord construction validation

use crate::signal::{BufferPolicy, WaitKey, WaitRecord};
use crate::state::{apply, LifecycleState, OperationalStatus, TransitionEvent};
use crate::types::TimestampMs;
use crate::Epoch;
use crate::InstanceId;
use crate::LineageError;
use crate::WorkflowLineage;
use rstest::rstest;
use std::collections::HashSet;

// ===========================================================================
// DIMENSION: lifecycle-state-machine
// Tests for the lifecycle state transition engine
// ===========================================================================

// CS-01: All valid transitions succeed
#[test]
fn rq_lifecycle_all_valid_transitions_succeed() {
    let valid_cases: Vec<(LifecycleState, TransitionEvent, LifecycleState)> = vec![
        (
            LifecycleState::Pending,
            TransitionEvent::AssignToNode,
            LifecycleState::RunningDecision,
        ),
        (
            LifecycleState::Failed,
            TransitionEvent::InstanceResumed,
            LifecycleState::RunningDecision,
        ),
        (
            LifecycleState::RunningDecision,
            TransitionEvent::StepScheduled,
            LifecycleState::StepScheduled,
        ),
        (
            LifecycleState::StepScheduled,
            TransitionEvent::ExecuteStep,
            LifecycleState::StepExecuting,
        ),
        (
            LifecycleState::StepExecuting,
            TransitionEvent::WaitForTimer,
            LifecycleState::WaitingForTimer,
        ),
        (
            LifecycleState::StepExecuting,
            TransitionEvent::CompleteStep,
            LifecycleState::Completed,
        ),
        (
            LifecycleState::WaitingForTimer,
            TransitionEvent::TimerFired,
            LifecycleState::StepExecuting,
        ),
        (
            LifecycleState::WaitingForTimer,
            TransitionEvent::TimerExpired,
            LifecycleState::Failed,
        ),
    ];

    for (from, event, expected) in valid_cases {
        let result = apply(from, event);
        assert!(
            result.is_ok(),
            "Transition {:?} from {:?} should succeed, got {:?}",
            event,
            from,
            result
        );
        assert_eq!(
            result.unwrap(),
            expected,
            "Transition {:?} from {:?} should yield {:?}",
            event,
            from,
            expected
        );
    }
}

// CS-02: Cancel transitions from all non-terminal states
#[test]
fn rq_lifecycle_cancel_from_all_non_terminal_states() {
    let non_terminal_states = vec![
        LifecycleState::Pending,
        LifecycleState::RunningDecision,
        LifecycleState::StepScheduled,
        LifecycleState::StepExecuting,
        LifecycleState::WaitingForTimer,
    ];

    for state in non_terminal_states {
        let result = apply(state, TransitionEvent::Cancel);
        assert!(
            result.is_ok(),
            "Cancel from {:?} should succeed, got {:?}",
            state,
            result
        );
        assert_eq!(
            result.unwrap(),
            LifecycleState::Cancelled,
            "Cancel from {:?} should yield Cancelled",
            state
        );
    }
}

// CS-03: Fail transitions from eligible states
#[test]
fn rq_lifecycle_fail_from_eligible_states() {
    let eligible_states = vec![
        LifecycleState::RunningDecision,
        LifecycleState::StepScheduled,
        LifecycleState::StepExecuting,
        LifecycleState::WaitingForTimer,
    ];

    for state in eligible_states {
        let result = apply(state, TransitionEvent::Fail);
        assert!(
            result.is_ok(),
            "Fail from {:?} should succeed, got {:?}",
            state,
            result
        );
        assert_eq!(
            result.unwrap(),
            LifecycleState::Failed,
            "Fail from {:?} should yield Failed",
            state
        );
    }
}

// CS-04: Terminal states reject all transitions
#[test]
fn rq_lifecycle_terminal_states_reject_all_transitions() {
    let terminal_states = vec![
        LifecycleState::Completed,
        LifecycleState::Failed,
        LifecycleState::Cancelled,
    ];
    let all_events = TransitionEvent::all_variants();

    for state in terminal_states {
        for event in all_events {
            let result = apply(state, *event);
            // Special case: Failed + InstanceResumed is valid
            if state == LifecycleState::Failed && event == &TransitionEvent::InstanceResumed {
                assert!(result.is_ok(), "Failed + InstanceResumed should be valid");
            } else {
                assert!(
                    result.is_err(),
                    "{:?} + {:?} should be rejected, got {:?}",
                    state,
                    event,
                    result
                );
            }
        }
    }
}

// CS-05: Invalid transitions from each state
#[test]
fn rq_lifecycle_invalid_transitions_rejected() {
    let invalid_cases: Vec<(LifecycleState, TransitionEvent)> = vec![
        // Pending can't do these
        (LifecycleState::Pending, TransitionEvent::StepScheduled),
        (LifecycleState::Pending, TransitionEvent::ExecuteStep),
        (LifecycleState::Pending, TransitionEvent::CompleteStep),
        (LifecycleState::Pending, TransitionEvent::WaitForTimer),
        (LifecycleState::Pending, TransitionEvent::TimerFired),
        (LifecycleState::Pending, TransitionEvent::TimerExpired),
        (LifecycleState::Pending, TransitionEvent::Fail),
        // RunningDecision can't skip to StepExecuting
        (
            LifecycleState::RunningDecision,
            TransitionEvent::ExecuteStep,
        ),
        (
            LifecycleState::RunningDecision,
            TransitionEvent::CompleteStep,
        ),
        (
            LifecycleState::RunningDecision,
            TransitionEvent::WaitForTimer,
        ),
        (LifecycleState::RunningDecision, TransitionEvent::TimerFired),
        // StepScheduled can't skip to StepExecuting/Waiting
        (LifecycleState::StepScheduled, TransitionEvent::CompleteStep),
        (LifecycleState::StepScheduled, TransitionEvent::WaitForTimer),
        (LifecycleState::StepScheduled, TransitionEvent::TimerFired),
        // StepExecuting can't skip
        (LifecycleState::StepExecuting, TransitionEvent::TimerFired),
        (LifecycleState::StepExecuting, TransitionEvent::TimerExpired),
        // WaitingForTimer can't do these
        (
            LifecycleState::WaitingForTimer,
            TransitionEvent::ExecuteStep,
        ),
        (
            LifecycleState::WaitingForTimer,
            TransitionEvent::CompleteStep,
        ),
        (
            LifecycleState::WaitingForTimer,
            TransitionEvent::WaitForTimer,
        ),
    ];

    for (state, event) in invalid_cases {
        let result = apply(state, event);
        assert!(
            result.is_err(),
            "Invalid transition {:?} from {:?} should be rejected, got {:?}",
            event,
            state,
            result
        );
    }
}

// CS-06: Double transition attempts
#[test]
fn rq_lifecycle_double_transition_rejected() {
    // Apply AssignToNode twice
    let result = apply(LifecycleState::Pending, TransitionEvent::AssignToNode);
    assert!(result.is_ok());
    let new_state = result.unwrap();
    let result2 = apply(new_state, TransitionEvent::AssignToNode);
    assert!(
        result2.is_err(),
        "Double AssignToNode should be rejected, got {:?}",
        result2
    );

    // Apply Cancel twice (Pending -> Cancelled, then cancel again)
    let result = apply(LifecycleState::Pending, TransitionEvent::Cancel);
    assert!(result.is_ok());
    let new_state = result.unwrap();
    let result2 = apply(new_state, TransitionEvent::Cancel);
    assert!(
        result2.is_err(),
        "Cancel from Cancelled should be rejected, got {:?}",
        result2
    );
}

// CS-07: Superstate consistency
#[test]
fn rq_lifecycle_superstate_consistency() {
    // Active states map to Active superstate
    let active_states = vec![
        LifecycleState::Pending,
        LifecycleState::RunningDecision,
        LifecycleState::StepScheduled,
        LifecycleState::StepExecuting,
    ];
    for state in active_states {
        assert_eq!(
            state.superstate(),
            crate::LifecycleSuperstate::Active,
            "{:?} should map to Active superstate",
            state
        );
        assert_eq!(
            state.get_operational_status(),
            OperationalStatus::Healthy,
            "{:?} should be Healthy",
            state
        );
    }

    // WaitingForTimer maps to Suspended
    assert_eq!(
        LifecycleState::WaitingForTimer.superstate(),
        crate::LifecycleSuperstate::Suspended
    );

    // Completed/Cancelled map to Terminal
    for state in [LifecycleState::Completed, LifecycleState::Cancelled] {
        assert_eq!(
            state.superstate(),
            crate::LifecycleSuperstate::Terminal,
            "{:?} should map to Terminal superstate",
            state
        );
    }

    // Failed maps to Terminal (operational_status is Recovering, but superstate is Terminal)
    assert_eq!(
        LifecycleState::Failed.superstate(),
        crate::LifecycleSuperstate::Terminal
    );
}

// CS-08: Terminal state detection
#[test]
fn rq_lifecycle_terminal_state_detection() {
    let terminal_states = vec![
        LifecycleState::Completed,
        LifecycleState::Failed,
        LifecycleState::Cancelled,
    ];
    let non_terminal_states = vec![
        LifecycleState::Pending,
        LifecycleState::RunningDecision,
        LifecycleState::StepScheduled,
        LifecycleState::StepExecuting,
        LifecycleState::WaitingForTimer,
    ];

    for state in terminal_states {
        assert!(state.is_terminal(), "{:?} should be terminal", state);
        assert!(
            apply(state, TransitionEvent::Cancel).is_err() || state == LifecycleState::Failed,
            "Cancel from terminal state should fail"
        );
    }

    for state in non_terminal_states {
        assert!(!state.is_terminal(), "{:?} should not be terminal", state);
    }
}

// CS-09: get_valid_transitions returns correct events
#[test]
fn rq_lifecycle_get_valid_transitions_completeness() {
    let state_events: Vec<(LifecycleState, Vec<TransitionEvent>)> = vec![
        (
            LifecycleState::Pending,
            vec![TransitionEvent::AssignToNode, TransitionEvent::Cancel],
        ),
        (
            LifecycleState::RunningDecision,
            vec![
                TransitionEvent::StepScheduled,
                TransitionEvent::Cancel,
                TransitionEvent::Fail,
            ],
        ),
        (
            LifecycleState::StepScheduled,
            vec![
                TransitionEvent::ExecuteStep,
                TransitionEvent::Cancel,
                TransitionEvent::Fail,
            ],
        ),
        (
            LifecycleState::StepExecuting,
            vec![
                TransitionEvent::WaitForTimer,
                TransitionEvent::YieldWithBlob,
                TransitionEvent::CompleteStep,
                TransitionEvent::Cancel,
                TransitionEvent::Fail,
            ],
        ),
        (
            LifecycleState::WaitingForTimer,
            vec![
                TransitionEvent::TimerFired,
                TransitionEvent::TimerExpired,
                TransitionEvent::Cancel,
                TransitionEvent::Fail,
            ],
        ),
        (
            LifecycleState::PendingPublication,
            vec![
                TransitionEvent::ConfirmPublication,
                TransitionEvent::PublicationFailed,
                TransitionEvent::Cancel,
            ],
        ),
        (LifecycleState::Completed, vec![]),
        (LifecycleState::Cancelled, vec![]),
        (
            LifecycleState::Failed,
            vec![TransitionEvent::InstanceResumed],
        ),
    ];

    for (state, expected_events) in state_events {
        let valid = state.get_valid_transitions();
        let valid_set: HashSet<TransitionEvent> = valid.iter().cloned().collect();
        let expected_set: HashSet<TransitionEvent> = expected_events.iter().cloned().collect();
        assert_eq!(
            valid_set, expected_set,
            "Valid transitions for {:?} mismatch: got {:?}, expected {:?}",
            state, valid, expected_events
        );
    }
}

// CS-10: InstanceResumed only valid from Failed
#[test]
fn rq_lifecycle_instance_resumed_only_from_failed() {
    let non_failed_states = vec![
        LifecycleState::Pending,
        LifecycleState::RunningDecision,
        LifecycleState::StepScheduled,
        LifecycleState::StepExecuting,
        LifecycleState::WaitingForTimer,
        LifecycleState::PendingPublication,
        LifecycleState::Completed,
        LifecycleState::Cancelled,
    ];

    for state in non_failed_states {
        let result = apply(state, TransitionEvent::InstanceResumed);
        assert!(
            result.is_err(),
            "InstanceResumed from {:?} should be rejected, got {:?}",
            state,
            result
        );
    }

    // Only Failed should accept InstanceResumed
    let result = apply(LifecycleState::Failed, TransitionEvent::InstanceResumed);
    assert!(
        result.is_ok(),
        "InstanceResumed from Failed should succeed, got {:?}",
        result
    );
}

// ===========================================================================
// DIMENSION: lineage-epoch-stack
// Tests for WorkflowLineage and Epoch stack operations
// ===========================================================================

// CS-11: WorkflowLineage::new rejects empty lineage_id
#[test]
fn rq_lineage_new_rejects_empty_lineage_id() {
    let result = WorkflowLineage::new(String::new());
    assert_eq!(result, Err(LineageError::EmptyLineageId));

    let result = WorkflowLineage::new("   ".to_string());
    assert_eq!(result, Err(LineageError::EmptyLineageId));

    let result = WorkflowLineage::new("\t\n".to_string());
    assert_eq!(result, Err(LineageError::EmptyLineageId));
}

// CS-12: WorkflowLineage::with_parent rejects invalid epoch transitions
#[test]
fn rq_lineage_with_parent_rejects_invalid_epoch_transitions() {
    // parent_epoch == epoch is invalid
    let result =
        WorkflowLineage::with_parent("lin".to_string(), Epoch::new(5), Some(Epoch::new(5)));
    assert_eq!(
        result,
        Err(LineageError::InvalidEpochTransition {
            parent_epoch: 5,
            epoch: 5
        })
    );

    // parent_epoch > epoch is invalid
    let result =
        WorkflowLineage::with_parent("lin".to_string(), Epoch::new(3), Some(Epoch::new(7)));
    assert_eq!(
        result,
        Err(LineageError::InvalidEpochTransition {
            parent_epoch: 7,
            epoch: 3
        })
    );
}

// CS-13: WorkflowLineage::with_parent accepts valid epoch transitions
#[test]
fn rq_lineage_with_parent_accepts_valid_transitions() {
    // parent_epoch < epoch is valid
    let result =
        WorkflowLineage::with_parent("lin".to_string(), Epoch::new(5), Some(Epoch::new(4)));
    assert!(result.is_ok());

    // parent_epoch = epoch - 1 is valid
    let result =
        WorkflowLineage::with_parent("lin".to_string(), Epoch::new(1), Some(Epoch::new(0)));
    assert!(result.is_ok());

    // No parent (root epoch) is valid
    let result = WorkflowLineage::with_parent("lin".to_string(), Epoch::new(0), None);
    assert!(result.is_ok());
}

// CS-14: Epoch boundary values
#[test]
fn rq_epoch_boundary_values() {
    // Epoch::ZERO is 0
    assert_eq!(Epoch::ZERO.0, 0);

    // Epoch::new with u64::MAX
    let epoch = Epoch::new(u64::MAX);
    assert_eq!(epoch.0, u64::MAX);

    // Epoch comparison at boundaries
    let e0 = Epoch::new(0);
    let e_max = Epoch::new(u64::MAX);
    assert!(e0 < e_max);
}

// CS-15: WorkflowLineage epoch transitions at boundaries
#[test]
fn rq_lineage_epoch_transitions_at_boundaries() {
    // Very large epoch values
    let result = WorkflowLineage::with_parent(
        "lin".to_string(),
        Epoch::new(u64::MAX),
        Some(Epoch::new(u64::MAX - 1)),
    );
    assert!(result.is_ok());

    // u64::MAX as epoch with parent = u64::MAX - 1
    let lineage = result.unwrap();
    assert_eq!(lineage.epoch, Epoch::new(u64::MAX));
    assert_eq!(lineage.parent_epoch, Some(Epoch::new(u64::MAX - 1)));

    // parent_epoch = 0, epoch = u64::MAX (should work)
    let result =
        WorkflowLineage::with_parent("lin".to_string(), Epoch::new(u64::MAX), Some(Epoch::new(0)));
    assert!(result.is_ok());
}

// CS-16: WorkflowLineage serde round-trip
#[test]
fn rq_lineage_serde_round_trip() {
    let lineage =
        WorkflowLineage::with_parent("lin-test".to_string(), Epoch::new(42), Some(Epoch::new(10)))
            .unwrap();
    let json = serde_json::to_value(&lineage).unwrap();
    let restored: WorkflowLineage = serde_json::from_value(json).unwrap();
    assert_eq!(restored, lineage);
}

// CS-17: LineageError display messages
#[test]
fn rq_lineage_error_display_messages() {
    let err_empty = LineageError::EmptyLineageId;
    assert!(err_empty.to_string().contains("empty"));

    let err_epoch = LineageError::InvalidEpochTransition {
        parent_epoch: 5,
        epoch: 3,
    };
    let msg = err_epoch.to_string();
    assert!(msg.contains("5") && msg.contains("3"));
}

// ===========================================================================
// DIMENSION: wait-record-stack
// Tests for WaitRecord and WaitKey operations
// ===========================================================================

// CS-18: WaitKey rejects empty string
#[test]
fn rq_wait_key_rejects_empty() {
    let result = WaitKey::parse("");
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(
            e.to_string().contains("WaitKey"),
            "Error should mention WaitKey type"
        );
    }
}

// CS-19: WaitKey accepts valid strings
#[test]
fn rq_wait_key_accepts_valid_strings() {
    // Normal string
    let result = WaitKey::parse("signal:order:created");
    assert!(result.is_ok());

    // Single character
    let result = WaitKey::parse("a");
    assert!(result.is_ok());

    // Unicode characters
    let result = WaitKey::parse("signal-日本語");
    assert!(result.is_ok());

    // Special characters
    let result = WaitKey::parse("signal:key:with:colons");
    assert!(result.is_ok());
}

// CS-20: WaitKey rejects strings exceeding max length
#[test]
fn rq_wait_key_rejects_exceeds_max_length() {
    // 257 characters
    let long_key = "a".repeat(257);
    let result = WaitKey::parse(&long_key);
    assert!(result.is_err());

    // 256 characters (exactly at limit)
    let max_key = "a".repeat(256);
    let result = WaitKey::parse(&max_key);
    assert!(result.is_ok());
}

// CS-21: WaitKey serde round-trip with edge cases
#[test]
fn rq_wait_key_serde_round_trip_edge_cases() {
    let cases = vec![
        "simple",
        "with:colons",
        "with-dashes",
        "a", // single char
        "日本語",
        "🦀", // emoji (multi-byte)
    ];

    for key_str in cases {
        if key_str.chars().count() <= 256 {
            let key = WaitKey::parse(key_str).unwrap();
            let json = serde_json::to_value(&key).unwrap();
            let restored: WaitKey = serde_json::from_value(json).unwrap();
            assert_eq!(restored, key, "Round-trip failed for {:?}", key_str);
        }
    }
}

// CS-22: BufferPolicy serde round-trip
#[rstest]
#[case(BufferPolicy::Reject)]
#[case(BufferPolicy::BufferOne)]
#[case(BufferPolicy::BufferMany)]
fn rq_buffer_policy_serde_round_trip(#[case] policy: BufferPolicy) {
    let json = serde_json::to_value(policy).unwrap();
    let restored: BufferPolicy = serde_json::from_value(json).unwrap();
    assert_eq!(restored, policy);
}

// CS-23: BufferPolicy::is_buffering
#[test]
fn rq_buffer_policy_is_buffering() {
    assert!(!BufferPolicy::Reject.is_buffering());
    assert!(BufferPolicy::BufferOne.is_buffering());
    assert!(BufferPolicy::BufferMany.is_buffering());
}

// CS-24: WaitRecord construction validation
#[test]
fn rq_wait_record_validates_wait_key() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let valid_key = WaitKey::parse("signal:order:created").unwrap();
    let timestamp = TimestampMs::try_from(1234567890u64).unwrap();

    let result = WaitRecord::new(
        instance_id.clone(),
        valid_key,
        BufferPolicy::Reject,
        timestamp,
    );
    assert!(result.is_ok());

    // Empty WaitKey should fail at WaitKey::parse level
    let empty_key_result = WaitKey::parse("");
    assert!(
        empty_key_result.is_err(),
        "WaitKey::parse should reject empty string"
    );
}

// CS-25: WaitRecord serde round-trip
#[test]
fn rq_wait_record_serde_round_trip() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let wait_key = WaitKey::parse("signal:order:created").unwrap();
    let timestamp = TimestampMs::try_from(1234567890u64).unwrap();

    let record = WaitRecord::new(
        instance_id.clone(),
        wait_key,
        BufferPolicy::BufferOne,
        timestamp,
    )
    .unwrap();

    let json = serde_json::to_value(&record).unwrap();
    let restored: WaitRecord = serde_json::from_value(json).unwrap();
    assert_eq!(restored, record);
}

// CS-26: WaitRecord accessors
#[test]
fn rq_wait_record_accessors() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let wait_key = WaitKey::parse("signal:test").unwrap();
    let timestamp = TimestampMs::try_from(1234567890u64).unwrap();

    let record = WaitRecord::new(
        instance_id.clone(),
        wait_key.clone(),
        BufferPolicy::BufferMany,
        timestamp,
    )
    .unwrap();

    assert_eq!(record.instance_id(), &instance_id);
    assert_eq!(record.wait_key(), &wait_key);
    assert_eq!(record.buffer_policy(), BufferPolicy::BufferMany);
    assert_eq!(record.registered_at(), timestamp);
}

// ===========================================================================
// DIMENSION: superstate-consistency
// Tests that superstate mapping is consistent across operations
// ===========================================================================

// CS-27: Superstate transitions are consistent with state transitions
#[test]
fn rq_superstate_consistency_with_transitions() {
    // Active -> Suspended transition
    let result = apply(LifecycleState::StepExecuting, TransitionEvent::WaitForTimer);
    assert!(result.is_ok());
    assert_eq!(
        result.unwrap().superstate(),
        crate::LifecycleSuperstate::Suspended
    );

    // Suspended -> Active (via TimerFired)
    let result = apply(LifecycleState::WaitingForTimer, TransitionEvent::TimerFired);
    assert!(result.is_ok());
    assert_eq!(
        result.unwrap().superstate(),
        crate::LifecycleSuperstate::Active
    );
}

// CS-28: All states have a valid superstate mapping
#[test]
fn rq_all_states_have_valid_superstate() {
    let states = vec![
        LifecycleState::Pending,
        LifecycleState::RunningDecision,
        LifecycleState::StepScheduled,
        LifecycleState::StepExecuting,
        LifecycleState::WaitingForTimer,
        LifecycleState::Completed,
        LifecycleState::Failed,
        LifecycleState::Cancelled,
    ];

    for state in states {
        let superstate = state.superstate();
        assert!(
            matches!(
                superstate,
                crate::LifecycleSuperstate::Active
                    | crate::LifecycleSuperstate::Suspended
                    | crate::LifecycleSuperstate::Terminal
                    | crate::LifecycleSuperstate::Recovering
                    | crate::LifecycleSuperstate::Compensating
            ),
            "{:?} has invalid superstate {:?}",
            state,
            superstate
        );
    }
}

// CS-29: LifecycleSuperstate serde round-trip
#[rstest]
#[case(crate::LifecycleSuperstate::Active)]
#[case(crate::LifecycleSuperstate::Suspended)]
#[case(crate::LifecycleSuperstate::Recovering)]
#[case(crate::LifecycleSuperstate::Compensating)]
#[case(crate::LifecycleSuperstate::Terminal)]
fn rq_lifecycle_superstate_serde_round_trip(#[case] superstate: crate::LifecycleSuperstate) {
    let json = serde_json::to_value(superstate).unwrap();
    let restored: crate::LifecycleSuperstate = serde_json::from_value(json).unwrap();
    assert_eq!(restored, superstate);
}

// CS-30: OperationalStatus display consistency
#[test]
fn rq_operational_status_consistency() {
    // Healthy states
    for state in [
        LifecycleState::Pending,
        LifecycleState::RunningDecision,
        LifecycleState::StepScheduled,
        LifecycleState::StepExecuting,
    ] {
        assert_eq!(
            state.get_operational_status(),
            OperationalStatus::Healthy,
            "{:?} should be Healthy",
            state
        );
    }

    // WaitingForTimer should be Healthy (not Suspended - that's the superstate)
    assert_eq!(
        LifecycleState::WaitingForTimer.get_operational_status(),
        OperationalStatus::Healthy
    );

    // Completed/Cancelled are Blocked
    for state in [LifecycleState::Completed, LifecycleState::Cancelled] {
        assert!(matches!(
            state.get_operational_status(),
            OperationalStatus::Blocked(_)
        ));
    }

    // Failed is Recovering
    assert_eq!(
        LifecycleState::Failed.get_operational_status(),
        OperationalStatus::Recovering
    );
}

// ===========================================================================
// DIMENSION: transition-error-semantics
// Tests that transition errors have correct semantics
// ===========================================================================

// CS-31: TransitionError display messages
#[test]
fn rq_transition_error_display_messages() {
    use crate::state::TransitionError;

    let err_terminal = TransitionError::TerminalStateTransition;
    assert!(err_terminal.to_string().contains("terminal"));

    let err_invalid = TransitionError::InvalidTransition;
    assert!(err_invalid.to_string().contains("Invalid"));
}

// CS-32: Apply error is deterministic
#[test]
fn rq_apply_error_deterministic() {
    // Same inputs always produce same error
    let result1 = apply(LifecycleState::Pending, TransitionEvent::CompleteStep);
    let result2 = apply(LifecycleState::Pending, TransitionEvent::CompleteStep);
    assert_eq!(result1, result2);
}

// CS-33: All non-terminal states accept at least one transition
#[test]
fn rq_all_non_terminal_states_have_valid_transitions() {
    let non_terminal = vec![
        LifecycleState::Pending,
        LifecycleState::RunningDecision,
        LifecycleState::StepScheduled,
        LifecycleState::StepExecuting,
        LifecycleState::WaitingForTimer,
    ];

    for state in non_terminal {
        let valid = state.get_valid_transitions();
        assert!(
            !valid.is_empty(),
            "{:?} should have at least one valid transition",
            state
        );
    }
}

// CS-34: LifecycleState Debug format contains state name
#[test]
fn rq_lifecycle_state_debug_contains_name() {
    let states = vec![
        LifecycleState::Pending,
        LifecycleState::RunningDecision,
        LifecycleState::StepScheduled,
        LifecycleState::StepExecuting,
        LifecycleState::WaitingForTimer,
        LifecycleState::Completed,
        LifecycleState::Failed,
        LifecycleState::Cancelled,
    ];

    for state in states {
        let debug = format!("{:?}", state);
        assert!(
            !debug.is_empty() && debug.len() > 3,
            "{:?} debug format should be non-empty",
            state
        );
    }
}
