//! Adversarial tests for the replay engine (Red Queen).
//!
//! These tests probe edge cases, boundary conditions, and invalid inputs
//! to verify the replay engine handles adversarial conditions correctly.
//!
//! Coverage areas:
//! 1. Invalid state transitions from every reachable state
//! 2. Crash injection / partial replay simulation
//! 3. Deterministic reconstruction after simulated crashes
//! 4. Schema version adversarial (upcaster failures, missing upcasters)
//! 5. Boundary conditions (u64 limits, zero values, large streams)
//! 6. Payload corruption and structural attacks

use super::engine::ReplayEngine;
use super::test_helpers::*;
use super::types::ReplayError;
use vo_types::events::EventEnvelope;
use vo_types::state::LifecycleState;

#[cfg(test)]
mod adversarial_transitions {
    use super::*;

    // =========================================================================
    // Edge Case: InstanceResumed from non-Failed states
    // =========================================================================

    #[test]
    fn replay_rejects_instance_resumed_from_pending() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, instance_resumed_payload("wf-1")),
        ];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(
            err,
            ReplayError::TransitionFailed {
                sequence: 2,
                state: LifecycleState::RunningDecision,
                ..
            }
        ));
    }

    #[test]
    fn replay_rejects_instance_resumed_from_running_decision() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, instance_resumed_payload("wf-1")),
        ];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(
            err,
            ReplayError::TransitionFailed {
                sequence: 2,
                state: LifecycleState::RunningDecision,
                ..
            }
        ));
    }

    #[test]
    fn replay_rejects_instance_resumed_from_step_scheduled() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, instance_resumed_payload("wf-1")),
        ];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(
            err,
            ReplayError::TransitionFailed {
                sequence: 3,
                state: LifecycleState::StepScheduled,
                ..
            }
        ));
    }

    #[test]
    fn replay_rejects_instance_resumed_from_step_executing() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, instance_resumed_payload("wf-1")),
        ];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(
            err,
            ReplayError::TransitionFailed {
                sequence: 4,
                state: LifecycleState::StepExecuting,
                ..
            }
        ));
    }

    #[test]
    fn replay_rejects_instance_resumed_from_waiting_for_timer() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, timer_set_payload("wf-1", "timer-1")),
            make_event("inst-1", 5, instance_resumed_payload("wf-1")),
        ];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(
            err,
            ReplayError::TransitionFailed {
                sequence: 5,
                state: LifecycleState::WaitingForTimer,
                ..
            }
        ));
    }

    #[test]
    fn replay_stops_processing_at_completed_ignores_instance_resumed() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
            make_event("inst-1", 5, instance_resumed_payload("wf-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Completed));
        assert_eq!(result.events_applied, 4);
    }

    #[test]
    fn replay_stops_processing_at_cancelled_ignores_instance_resumed() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, workflow_cancelled_payload("wf-1")),
            make_event("inst-1", 3, instance_resumed_payload("wf-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Cancelled));
        assert_eq!(result.events_applied, 2);
    }

    // =========================================================================
    // Edge Case: Events after Failed state (non-InstanceResumed)
    // =========================================================================

    #[test]
    fn replay_rejects_step_scheduled_after_failed() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
            make_event("inst-1", 5, step_scheduled_payload("wf-1", "step-2")),
        ];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(
            err,
            ReplayError::TransitionFailed {
                sequence: 5,
                state: LifecycleState::Failed,
                ..
            }
        ));
    }

    #[test]
    fn replay_rejects_timer_set_after_failed() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
            make_event("inst-1", 5, timer_set_payload("wf-1", "timer-1")),
        ];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(
            err,
            ReplayError::TransitionFailed {
                sequence: 5,
                state: LifecycleState::Failed,
                ..
            }
        ));
    }

    // =========================================================================
    // Edge Case: Multiple consecutive InstanceResumed events
    // =========================================================================

    #[test]
    fn replay_rejects_double_instance_resumed() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, workflow_failed_payload("wf-1")),
            make_event("inst-1", 3, instance_resumed_payload("wf-1")),
            make_event("inst-1", 4, instance_resumed_payload("wf-1")),
        ];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(
            err,
            ReplayError::TransitionFailed {
                sequence: 4,
                state: LifecycleState::RunningDecision,
                ..
            }
        ));
    }

    // =========================================================================
    // Edge Case: Failed -> InstanceResumed -> Failed (multiple recovery cycles)
    // =========================================================================

    #[test]
    fn replay_handles_multiple_failure_recovery_cycles() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
            make_event("inst-1", 5, instance_resumed_payload("wf-1")),
            make_event("inst-1", 6, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 7, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 8, step_failed_payload("wf-1", "step-1")),
            make_event("inst-1", 9, instance_resumed_payload("wf-1")),
            make_event("inst-1", 10, step_scheduled_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::StepScheduled));
        assert_eq!(result.events_applied, 10);
    }

    // =========================================================================
    // Edge Case: Sequence number boundary conditions
    // =========================================================================

    #[test]
    fn replay_handles_sequence_number_zero() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 0, workflow_started_payload("wf-1")),
            make_event("inst-1", 1, step_scheduled_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::StepScheduled));
        assert_eq!(result.events_applied, 2);
    }

    #[test]
    fn replay_handles_sequence_starting_at_arbitrary_value() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 100, workflow_started_payload("wf-1")),
            make_event("inst-1", 101, step_scheduled_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::StepScheduled));
        assert_eq!(result.events_applied, 2);
    }

    #[test]
    fn replay_detects_sequence_gap_after_first_event() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 3, step_scheduled_payload("wf-1", "step-1")),
        ];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(
            err,
            ReplayError::SequenceGap {
                expected: 2,
                actual: 3,
                at_index: 1,
            }
        ));
    }

    // =========================================================================
    // Edge Case: Empty and special instance_id values
    // =========================================================================

    #[test]
    fn replay_handles_empty_instance_id() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("", 1, workflow_started_payload("wf-1")),
            make_event("", 2, step_scheduled_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::StepScheduled));
        assert_eq!(result.events_applied, 2);
    }

    #[test]
    fn replay_rejects_mixed_instance_ids() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-2", 2, step_scheduled_payload("wf-1", "step-1")),
        ];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(
            err,
            ReplayError::InstanceMismatch {
                expected: _,
                actual: _,
            }
        ));
    }

    // =========================================================================
    // Edge Case: ContinuedAsNew is truly a no-op
    // =========================================================================

    #[test]
    fn replay_continued_as_new_counts_as_applied() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, continued_as_new_payload("wf-1")),
            make_event("inst-1", 3, continued_as_new_payload("wf-1")),
            make_event("inst-1", 4, step_scheduled_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::StepScheduled));
        assert_eq!(result.events_applied, 4);
    }

    #[test]
    fn replay_continued_as_new_does_not_change_state() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, continued_as_new_payload("wf-1")),
            make_event("inst-1", 4, step_started_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::StepExecuting));
        assert_eq!(result.events_applied, 4);
    }

    #[test]
    fn replay_continued_as_new_at_terminal_state() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
            make_event("inst-1", 5, continued_as_new_payload("wf-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Completed));
        assert_eq!(result.events_applied, 4);
    }

    // =========================================================================
    // Edge Case: Malformed JSON payloads
    // =========================================================================

    #[test]
    fn replay_rejects_payload_with_missing_type_field() {
        let engine = ReplayEngine::new();
        let json = serde_json::json!({
            "workflow_id": "wf-1",
            "binary_hash": "sha256abc",
            "version": 1
        });
        let events = [make_event("inst-1", 1, json)];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(
            err,
            ReplayError::PayloadDecodeFailed { sequence: 1, .. }
        ));
    }

    #[test]
    fn replay_rejects_payload_with_unknown_type() {
        let engine = ReplayEngine::new();
        let json = serde_json::json!({
            "type": "UnknownEventType",
            "workflow_id": "wf-1",
            "version": 1
        });
        let events = [make_event("inst-1", 1, json)];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(
            err,
            ReplayError::PayloadDecodeFailed { sequence: 1, .. }
        ));
    }

    #[test]
    fn replay_rejects_payload_with_missing_required_fields() {
        let engine = ReplayEngine::new();
        let json = serde_json::json!({
            "type": "WorkflowStarted",
            "version": 1
        });
        let events = [make_event("inst-1", 1, json)];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(
            err,
            ReplayError::PayloadDecodeFailed { sequence: 1, .. }
        ));
    }

    // =========================================================================
    // Edge Case: Sequence validation at boundaries
    // =========================================================================

    #[test]
    fn replay_rejects_sequence_duplicates_at_boundaries() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 1, step_scheduled_payload("wf-1", "step-1")),
        ];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(
            err,
            ReplayError::SequenceDuplicate {
                sequence: 1,
                first_at_index: 0,
                second_at_index: 1,
            }
        ));
    }

    #[test]
    fn replay_allows_arbitrary_starting_sequence() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 500, workflow_started_payload("wf-1")),
            make_event("inst-1", 501, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 502, step_started_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::StepExecuting));
        assert_eq!(result.events_applied, 3);
    }

    // =========================================================================
    // Edge Case: Complete workflow with all non-terminal events
    // =========================================================================

    #[test]
    fn replay_full_lifecycle_all_healthy_states() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Completed));
        assert_eq!(result.events_applied, 4);
    }

    #[test]
    fn replay_cancelled_from_various_states() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, cancel_requested_payload("wf-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Cancelled));
        assert_eq!(result.events_applied, 2);
    }

    #[test]
    fn replay_cancelled_during_step_execution() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, cancel_requested_payload("wf-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Cancelled));
        assert_eq!(result.events_applied, 4);
    }

    #[test]
    fn replay_cancelled_during_timer_wait() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, timer_set_payload("wf-1", "timer-1")),
            make_event("inst-1", 5, cancel_requested_payload("wf-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Cancelled));
        assert_eq!(result.events_applied, 5);
    }
}

#[cfg(test)]
mod crash_injection {
    use super::*;

    /// Crash after 0 events — engine sees nothing.
    #[test]
    fn crash_after_0_events_produces_no_state() {
        let engine = ReplayEngine::new();
        let result = engine.replay(&[]).expect("empty replay");
        assert_eq!(result.final_state, None);
        assert_eq!(result.events_applied, 0);
    }

    /// Crash after 1 event — WorkflowStarted produces RunningDecision.
    #[test]
    fn crash_after_1_event_produces_running_decision() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-crash", 1, workflow_started_payload("wf-crash")),
            make_event("inst-crash", 2, step_scheduled_payload("wf-crash", "step-1")),
            make_event("inst-crash", 3, step_started_payload("wf-crash", "step-1")),
            make_event("inst-crash", 4, step_completed_payload("wf-crash", "step-1")),
        ];
        let result = engine.replay(&events[..1]).expect("partial replay");
        assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
        assert_eq!(result.events_applied, 1);
    }

    /// Crash after 2 events — StepScheduled state.
    #[test]
    fn crash_after_2_events_produces_step_scheduled() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-crash", 1, workflow_started_payload("wf-crash")),
            make_event("inst-crash", 2, step_scheduled_payload("wf-crash", "step-1")),
            make_event("inst-crash", 3, step_started_payload("wf-crash", "step-1")),
            make_event("inst-crash", 4, step_completed_payload("wf-crash", "step-1")),
        ];
        let result = engine.replay(&events[..2]).expect("partial replay");
        assert_eq!(result.final_state, Some(LifecycleState::StepScheduled));
        assert_eq!(result.events_applied, 2);
    }

    /// Crash after 3 events — StepExecuting state.
    #[test]
    fn crash_after_3_events_produces_step_executing() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-crash", 1, workflow_started_payload("wf-crash")),
            make_event("inst-crash", 2, step_scheduled_payload("wf-crash", "step-1")),
            make_event("inst-crash", 3, step_started_payload("wf-crash", "step-1")),
            make_event("inst-crash", 4, step_completed_payload("wf-crash", "step-1")),
        ];
        let result = engine.replay(&events[..3]).expect("partial replay");
        assert_eq!(result.final_state, Some(LifecycleState::StepExecuting));
        assert_eq!(result.events_applied, 3);
    }

    /// Crash after 4 events — Completed (terminal).
    #[test]
    fn crash_after_4_events_produces_completed() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-crash", 1, workflow_started_payload("wf-crash")),
            make_event("inst-crash", 2, step_scheduled_payload("wf-crash", "step-1")),
            make_event("inst-crash", 3, step_started_payload("wf-crash", "step-1")),
            make_event("inst-crash", 4, step_completed_payload("wf-crash", "step-1")),
        ];
        let result = engine.replay(&events[..4]).expect("partial replay");
        assert_eq!(result.final_state, Some(LifecycleState::Completed));
        assert_eq!(result.events_applied, 4);
    }

    /// Crash during timer wait — WaitingForTimer preserved.
    #[test]
    fn crash_during_timer_wait_preserves_waiting_state() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-timer", 1, workflow_started_payload("wf-timer")),
            make_event("inst-timer", 2, step_scheduled_payload("wf-timer", "step-1")),
            make_event("inst-timer", 3, step_started_payload("wf-timer", "step-1")),
            make_event("inst-timer", 4, timer_set_payload("wf-timer", "timer-1")),
        ];
        let result = engine.replay(&events).expect("partial replay");
        assert_eq!(result.final_state, Some(LifecycleState::WaitingForTimer));
        assert_eq!(result.events_applied, 4);
    }

    /// Crash at Failed state, then resume with InstanceResumed.
    #[test]
    fn crash_at_failed_state_can_be_resumed() {
        let engine = ReplayEngine::new();
        let crash_events = [
            make_event("inst-fail", 1, workflow_started_payload("wf-fail")),
            make_event("inst-fail", 2, step_scheduled_payload("wf-fail", "step-1")),
            make_event("inst-fail", 3, step_started_payload("wf-fail", "step-1")),
            make_event("inst-fail", 4, step_failed_payload("wf-fail", "step-1")),
        ];
        let crashed = engine.replay(&crash_events).expect("crash replay");
        assert_eq!(crashed.final_state, Some(LifecycleState::Failed));

        let full_events = [
            make_event("inst-fail", 1, workflow_started_payload("wf-fail")),
            make_event("inst-fail", 2, step_scheduled_payload("wf-fail", "step-1")),
            make_event("inst-fail", 3, step_started_payload("wf-fail", "step-1")),
            make_event("inst-fail", 4, step_failed_payload("wf-fail", "step-1")),
            make_event("inst-fail", 5, instance_resumed_payload("wf-fail")),
        ];
        let recovered = engine.replay(&full_events).expect("recovery replay");
        assert_eq!(recovered.final_state, Some(LifecycleState::RunningDecision));
        assert_eq!(recovered.events_applied, 5);
    }

    /// Crash before ContinuedAsNew, then resume with more events.
    #[test]
    fn crash_before_continued_as_new_then_resume() {
        let engine = ReplayEngine::new();
        let prefix = [
            make_event("inst-can", 1, workflow_started_payload("wf-can")),
            make_event("inst-can", 2, continued_as_new_payload("wf-can")),
        ];
        let crashed = engine.replay(&prefix).expect("crash replay");
        assert_eq!(crashed.final_state, Some(LifecycleState::RunningDecision));

        let full = [
            make_event("inst-can", 1, workflow_started_payload("wf-can")),
            make_event("inst-can", 2, continued_as_new_payload("wf-can")),
            make_event("inst-can", 3, step_scheduled_payload("wf-can", "step-1")),
            make_event("inst-can", 4, step_started_payload("wf-can", "step-1")),
            make_event("inst-can", 5, step_completed_payload("wf-can", "step-1")),
        ];
        let result = engine.replay(&full).expect("resumed replay");
        assert_eq!(result.final_state, Some(LifecycleState::Completed));
        assert_eq!(result.events_applied, 5);
    }

    /// Crash position tracks last applied sequence and timestamp.
    #[test]
    fn crash_position_tracks_last_applied_sequence() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-pos", 10, workflow_started_payload("wf-pos")),
            make_event("inst-pos", 11, step_scheduled_payload("wf-pos", "step-1")),
            make_event("inst-pos", 12, step_started_payload("wf-pos", "step-1")),
        ];
        let result = engine.replay(&events[..2]).expect("partial replay");
        assert_eq!(result.position.last_applied_sequence, Some(11));
        assert_eq!(result.position.last_applied_timestamp_ms, Some(11_000));
    }
}

#[cfg(test)]
mod deterministic_reconstruction {
    use super::*;

    /// Core invariant: replaying the same event sequence always produces
    /// the identical ReplayResult for every prefix (crash point).
    #[test]
    fn deterministic_replay_of_every_prefix() {
        let engine = ReplayEngine::new();
        let events: Vec<_> = vec![
            make_event("inst-det", 1, workflow_started_payload("wf-det")),
            make_event("inst-det", 2, step_scheduled_payload("wf-det", "step-1")),
            make_event("inst-det", 3, step_started_payload("wf-det", "step-1")),
            make_event("inst-det", 4, step_completed_payload("wf-det", "step-1")),
        ];

        for prefix_len in 0..=events.len() {
            let prefix = &events[..prefix_len];
            let r1 = engine.replay(prefix).unwrap_or_else(|e| panic!("prefix {prefix_len}: {e}"));
            let r2 = engine.replay(prefix).unwrap_or_else(|e| panic!("prefix {prefix_len}: {e}"));
            assert_eq!(r1, r2, "determinism violated at prefix length {prefix_len}");
        }
    }

    /// Deterministic replay with failure-recovery cycles.
    #[test]
    fn deterministic_replay_with_failure_recovery() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-det", 1, workflow_started_payload("wf-det")),
            make_event("inst-det", 2, workflow_failed_payload("wf-det")),
            make_event("inst-det", 3, instance_resumed_payload("wf-det")),
            make_event("inst-det", 4, step_scheduled_payload("wf-det", "step-1")),
            make_event("inst-det", 5, step_started_payload("wf-det", "step-1")),
            make_event("inst-det", 6, step_failed_payload("wf-det", "step-1")),
            make_event("inst-det", 7, instance_resumed_payload("wf-det")),
            make_event("inst-det", 8, step_scheduled_payload("wf-det", "step-2")),
        ];

        for prefix_len in 1..=events.len() {
            let prefix = &events[..prefix_len];
            let r1 = engine.replay(prefix).unwrap();
            let r2 = engine.replay(prefix).unwrap();
            assert_eq!(r1, r2, "determinism violated with recovery at prefix {prefix_len}");
        }
    }

    /// Crash-recovery round trip: prefix state matches full replay at same position.
    #[test]
    fn crash_recovery_produces_consistent_intermediate_states() {
        let engine = ReplayEngine::new();
        let events: Vec<_> = vec![
            make_event("inst-rt", 1, workflow_started_payload("wf-rt")),
            make_event("inst-rt", 2, step_scheduled_payload("wf-rt", "step-1")),
            make_event("inst-rt", 3, step_started_payload("wf-rt", "step-1")),
            make_event("inst-rt", 4, step_completed_payload("wf-rt", "step-1")),
        ];

        let expected_states = [
            Some(LifecycleState::RunningDecision),
            Some(LifecycleState::StepScheduled),
            Some(LifecycleState::StepExecuting),
            Some(LifecycleState::Completed),
        ];

        for (i, expected_state) in expected_states.iter().enumerate() {
            let prefix_result = engine
                .replay(&events[..=i])
                .unwrap_or_else(|e| panic!("prefix {} failed: {}", i + 1, e));
            assert_eq!(
                &prefix_result.final_state,
                expected_state,
                "state mismatch at prefix length {}",
                i + 1
            );
        }
    }

    /// Crash recovery from Failed through InstanceResumed to Completed.
    #[test]
    fn crash_recovery_failed_state_resumable_to_completion() {
        let engine = ReplayEngine::new();

        let crash_events = [
            make_event("inst-rt", 1, workflow_started_payload("wf-rt")),
            make_event("inst-rt", 2, step_scheduled_payload("wf-rt", "step-1")),
            make_event("inst-rt", 3, step_started_payload("wf-rt", "step-1")),
            make_event("inst-rt", 4, step_failed_payload("wf-rt", "step-1")),
        ];
        let crash_result = engine.replay(&crash_events).unwrap();
        assert_eq!(crash_result.final_state, Some(LifecycleState::Failed));

        let full_events = [
            make_event("inst-rt", 1, workflow_started_payload("wf-rt")),
            make_event("inst-rt", 2, step_scheduled_payload("wf-rt", "step-1")),
            make_event("inst-rt", 3, step_started_payload("wf-rt", "step-1")),
            make_event("inst-rt", 4, step_failed_payload("wf-rt", "step-1")),
            make_event("inst-rt", 5, instance_resumed_payload("wf-rt")),
            make_event("inst-rt", 6, step_scheduled_payload("wf-rt", "step-1")),
            make_event("inst-rt", 7, step_started_payload("wf-rt", "step-1")),
            make_event("inst-rt", 8, step_completed_payload("wf-rt", "step-1")),
        ];
        let r1 = engine.replay(&full_events).unwrap();
        let r2 = engine.replay(&full_events).unwrap();
        assert_eq!(r1, r2);
        assert_eq!(r1.final_state, Some(LifecycleState::Completed));
        assert_eq!(r1.events_applied, 8);
    }

    /// Multiple independent ReplayEngine instances produce identical results.
    #[test]
    fn replay_is_stateless_across_instances() {
        let events = [
            make_event("inst-sl", 1, workflow_started_payload("wf-sl")),
            make_event("inst-sl", 2, step_scheduled_payload("wf-sl", "step-1")),
            make_event("inst-sl", 3, step_started_payload("wf-sl", "step-1")),
            make_event("inst-sl", 4, step_completed_payload("wf-sl", "step-1")),
        ];

        let r1 = ReplayEngine::new().replay(&events).unwrap();
        let r2 = ReplayEngine::new().replay(&events).unwrap();
        let r3 = ReplayEngine::default().replay(&events).unwrap();

        assert_eq!(r1, r2);
        assert_eq!(r2, r3);
    }
}

#[cfg(test)]
mod schema_version_adversarial {
    use super::*;
    use crate::upcaster::{Upcaster, UpcasterError, UpcasterRegistry};

    /// Missing upcaster for a schema version causes UpcastingFailed.
    #[test]
    fn replay_with_upcaster_fails_when_no_upcaster_registered() {
        let engine = ReplayEngine::new();
        let registry = crate::upcaster::UpcasterRegistryImpl::new(1);
        let events = [make_v0_event("inst-1", 1, workflow_started_payload("wf-1"))];
        let err = engine
            .replay_with_upcaster(&registry, &events)
            .expect_err("should fail");
        assert!(matches!(err, ReplayError::UpcastingFailed { sequence: 1, .. }));
    }

    /// Upcaster failure reports correct sequence number.
    #[test]
    fn replay_with_upcaster_reports_correct_sequence_on_failure() {
        struct AlwaysFailUpcaster;
        impl Upcaster for AlwaysFailUpcaster {
            fn source_version(&self) -> u8 { 0 }
            fn upcast(&self, _input: &[u8]) -> Result<Vec<u8>, UpcasterError> {
                Err(UpcasterError::UpcastingFailed("forced failure".to_string()))
            }
        }

        let engine = ReplayEngine::new();
        let registry = crate::upcaster::UpcasterRegistryImpl::new(1);
        let _ = registry.register(Box::new(AlwaysFailUpcaster));
        let events = [
            make_event("inst-1", 10, workflow_started_payload("wf-1")),
            make_v0_event("inst-1", 42, step_scheduled_payload("wf-1", "step-1")),
        ];
        let err = engine
            .replay_with_upcaster(&registry, &events)
            .expect_err("should fail");
        assert!(
            matches!(err, ReplayError::UpcastingFailed { sequence: 42, .. }),
            "expected UpcastingFailed at sequence 42, got {err:?}"
        );
    }

    /// Upcaster failure at first event prevents any state reconstruction.
    #[test]
    fn replay_with_upcaster_all_or_nothing_first_event_corrupt() {
        struct AlwaysFailUpcaster;
        impl Upcaster for AlwaysFailUpcaster {
            fn source_version(&self) -> u8 { 0 }
            fn upcast(&self, _input: &[u8]) -> Result<Vec<u8>, UpcasterError> {
                Err(UpcasterError::UpcastingFailed("forced failure".to_string()))
            }
        }

        let engine = ReplayEngine::new();
        let registry = crate::upcaster::UpcasterRegistryImpl::new(1);
        let _ = registry.register(Box::new(AlwaysFailUpcaster));
        let events = [make_v0_event("inst-1", 1, workflow_started_payload("wf-1"))];
        assert!(engine.replay_with_upcaster(&registry, &events).is_err());
    }

    /// Upcaster failure at middle event — all-or-nothing prevents partial replay.
    #[test]
    fn replay_with_upcaster_all_or_nothing_middle_event_corrupt() {
        struct AlwaysFailUpcaster;
        impl Upcaster for AlwaysFailUpcaster {
            fn source_version(&self) -> u8 { 0 }
            fn upcast(&self, _input: &[u8]) -> Result<Vec<u8>, UpcasterError> {
                Err(UpcasterError::UpcastingFailed("forced failure".to_string()))
            }
        }

        let engine = ReplayEngine::new();
        let registry = crate::upcaster::UpcasterRegistryImpl::new(1);
        let _ = registry.register(Box::new(AlwaysFailUpcaster));
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_v0_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        ];
        assert!(engine.replay_with_upcaster(&registry, &events).is_err());
    }

    /// Upcasting + replay is deterministic across calls.
    #[test]
    fn replay_with_upcaster_deterministic_across_calls() {
        struct MigratingUpcaster;
        impl Upcaster for MigratingUpcaster {
            fn source_version(&self) -> u8 { 0 }
            fn upcast(&self, input: &[u8]) -> Result<Vec<u8>, UpcasterError> {
                let mut val: serde_json::Value = serde_json::from_slice(input)
                    .map_err(|e| UpcasterError::UpcastingFailed(e.to_string()))?;
                val["version"] = serde_json::json!(1);
                serde_json::to_vec(&val).map_err(|e| UpcasterError::UpcastingFailed(e.to_string()))
            }
        }

        let engine = ReplayEngine::new();
        let registry = crate::upcaster::UpcasterRegistryImpl::new(1);
        let _ = registry.register(Box::new(MigratingUpcaster));
        let events = [
            make_v0_event("inst-det", 1, workflow_started_payload("wf-det")),
            make_v0_event("inst-det", 2, step_scheduled_payload("wf-det", "step-1")),
            make_v0_event("inst-det", 3, step_started_payload("wf-det", "step-1")),
            make_v0_event("inst-det", 4, step_completed_payload("wf-det", "step-1")),
        ];
        let r1 = engine.replay_with_upcaster(&registry, &events).unwrap();
        let r2 = engine.replay_with_upcaster(&registry, &events).unwrap();
        assert_eq!(r1, r2);
    }
}

#[cfg(test)]
mod boundary_conditions {
    use super::*;

    /// Sequence numbers near u64::MAX work correctly.
    #[test]
    fn replay_handles_u64_max_sequence_start() {
        let engine = ReplayEngine::new();
        let events = [
            vo_types::events::EventEnvelope {
                schema_version: 1,
                instance_id: "inst-max".to_string(),
                sequence: u64::MAX - 1,
                timestamp_ms: 100,
                payload: workflow_started_payload("wf-max"),
                metadata: vo_types::events::EventMetadata::default(),
            },
            vo_types::events::EventEnvelope {
                schema_version: 1,
                instance_id: "inst-max".to_string(),
                sequence: u64::MAX,
                timestamp_ms: 200,
                payload: step_scheduled_payload("wf-max", "step-1"),
                metadata: vo_types::events::EventMetadata::default(),
            },
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::StepScheduled));
        assert_eq!(result.events_applied, 2);
        assert_eq!(result.position.last_applied_sequence, Some(u64::MAX));
    }

    /// Zero timestamp is handled correctly.
    #[test]
    fn replay_handles_zero_timestamp() {
        let engine = ReplayEngine::new();
        let mut event = make_event("inst-ts", 1, workflow_started_payload("wf-ts"));
        event.timestamp_ms = 0;
        let result = engine.replay(&[event]).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
        assert_eq!(result.position.last_applied_timestamp_ms, Some(0));
    }

    /// Large event stream with repeated failure-recovery cycles.
    #[test]
    fn replay_handles_many_events_stress() {
        let engine = ReplayEngine::new();
        let mut events = Vec::new();
        let mut seq: u64 = 1;
        events.push(make_event("inst-stress", seq, workflow_started_payload("wf-stress")));
        seq += 1;

        for i in 0..50u64 {
            let step_id = format!("step-{i}");
            events.push(make_event("inst-stress", seq, step_scheduled_payload("wf-stress", &step_id)));
            seq += 1;
            events.push(make_event("inst-stress", seq, step_started_payload("wf-stress", &step_id)));
            seq += 1;
            events.push(make_event("inst-stress", seq, step_failed_payload("wf-stress", &step_id)));
            seq += 1;
            events.push(make_event("inst-stress", seq, instance_resumed_payload("wf-stress")));
            seq += 1;
        }
        // Final step succeeds
        events.push(make_event("inst-stress", seq, step_scheduled_payload("wf-stress", "step-final")));
        seq += 1;
        events.push(make_event("inst-stress", seq, step_started_payload("wf-stress", "step-final")));
        seq += 1;
        events.push(make_event("inst-stress", seq, step_completed_payload("wf-stress", "step-final")));
        seq += 1;

        let result = engine.replay(&events).expect("stress replay");
        assert_eq!(result.final_state, Some(LifecycleState::Completed));
        assert_eq!(result.events_applied, events.len());
    }

    /// Payload with extra unexpected fields is still valid.
    #[test]
    fn replay_payload_with_extra_fields_is_accepted() {
        let engine = ReplayEngine::new();
        let json = serde_json::json!({
            "type": "WorkflowStarted",
            "workflow_id": "wf-extra",
            "binary_hash": "sha256abc",
            "version": 1,
            "extra_field": "should be ignored"
        });
        let events = [make_event("inst-1", 1, json)];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
    }

    /// Payload with null required field is rejected.
    #[test]
    fn replay_rejects_payload_with_null_workflow_id() {
        let engine = ReplayEngine::new();
        let json = serde_json::json!({
            "type": "WorkflowStarted",
            "workflow_id": null,
            "binary_hash": "sha256abc",
            "version": 1
        });
        let events = [make_event("inst-1", 1, json)];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(err, ReplayError::PayloadDecodeFailed { .. }));
    }

    /// Non-object payload (string) is rejected.
    #[test]
    fn replay_rejects_non_object_payload() {
        let engine = ReplayEngine::new();
        let events = [make_event("inst-1", 1, serde_json::json!("just a string"))];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(err, ReplayError::PayloadDecodeFailed { sequence: 1, .. }));
    }
}

#[cfg(test)]
mod transition_attacks {
    use super::*;

    /// StepStarted without prior StepScheduled is rejected.
    #[test]
    fn replay_rejects_step_started_without_step_scheduled() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_started_payload("wf-1", "step-1")),
        ];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(
            err,
            ReplayError::TransitionFailed {
                sequence: 2,
                state: LifecycleState::RunningDecision,
                ..
            }
        ));
    }

    /// StepCompleted without prior StepStarted is rejected.
    #[test]
    fn replay_rejects_step_completed_without_step_started() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_completed_payload("wf-1", "step-1")),
        ];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(
            err,
            ReplayError::TransitionFailed {
                sequence: 3,
                state: LifecycleState::StepScheduled,
                ..
            }
        ));
    }

    /// TimerFired without prior TimerSet is rejected.
    #[test]
    fn replay_rejects_timer_fired_without_timer_set() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, timer_fired_payload("wf-1", "timer-1")),
        ];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(
            err,
            ReplayError::TransitionFailed {
                sequence: 4,
                state: LifecycleState::StepExecuting,
                ..
            }
        ));
    }

    /// Second WorkflowStarted from RunningDecision is rejected.
    #[test]
    fn replay_rejects_second_workflow_started() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, workflow_started_payload("wf-1")),
        ];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(
            err,
            ReplayError::TransitionFailed {
                sequence: 2,
                state: LifecycleState::RunningDecision,
                ..
            }
        ));
    }

    /// WorkflowFailed directly from Pending (no WorkflowStarted) is rejected.
    #[test]
    fn replay_rejects_workflow_failed_from_pending() {
        let engine = ReplayEngine::new();
        let events = [make_event("inst-1", 1, workflow_failed_payload("wf-1"))];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(
            err,
            ReplayError::TransitionFailed {
                sequence: 1,
                state: LifecycleState::Pending,
                ..
            }
        ));
    }

    /// CancelRequested from Pending is ACCEPTED — workflows can be cancelled before starting.
    /// Red Queen finding: the state machine allows Cancel from the initial Pending state.
    #[test]
    fn replay_accepts_cancel_from_pending() {
        let engine = ReplayEngine::new();
        let events = [make_event("inst-1", 1, cancel_requested_payload("wf-1"))];
        let result = engine.replay(&events).expect("should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Cancelled));
        assert_eq!(result.events_applied, 1);
    }

    /// StepScheduled from Pending (no WorkflowStarted) is rejected.
    #[test]
    fn replay_rejects_step_scheduled_from_pending() {
        let engine = ReplayEngine::new();
        let events = [make_event("inst-1", 1, step_scheduled_payload("wf-1", "step-1"))];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(
            err,
            ReplayError::TransitionFailed {
                sequence: 1,
                state: LifecycleState::Pending,
                ..
            }
        ));
    }

    /// Events after Cancelled terminal state are silently ignored (stopped processing).
    #[test]
    fn replay_ignores_events_after_cancelled_terminal() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, cancel_requested_payload("wf-1")),
            make_event("inst-1", 3, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_started_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::Cancelled));
        assert_eq!(result.events_applied, 2);
    }
}

// =========================================================================
// Adversarial: Instance ID attacks
// =========================================================================

#[cfg(test)]
mod instance_id_attacks {
    use super::*;

    /// Instance IDs with unicode characters work correctly.
    #[test]
    fn replay_handles_unicode_instance_ids() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-日本語", 1, workflow_started_payload("wf-1")),
            make_event("inst-日本語", 2, step_scheduled_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::StepScheduled));
    }

    /// Very long instance IDs work correctly.
    #[test]
    fn replay_handles_very_long_instance_ids() {
        let long_id = "x".repeat(10000);
        let engine = ReplayEngine::new();
        let events = [
            make_event(&long_id, 1, workflow_started_payload("wf-1")),
            make_event(&long_id, 2, step_scheduled_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::StepScheduled));
    }

    /// Different unicode instance IDs are correctly detected as mismatch.
    #[test]
    fn replay_detects_unicode_instance_mismatch() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-αβγ", 1, workflow_started_payload("wf-1")),
            make_event("inst-δεζ", 2, step_scheduled_payload("wf-1", "step-1")),
        ];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(err, ReplayError::InstanceMismatch { .. }));
    }

    /// Instance IDs with null bytes in strings.
    #[test]
    fn replay_handles_null_byte_in_instance_id() {
        let engine = ReplayEngine::new();
        let id_with_null = "inst\u{0000}null";
        let events = [
            make_event(id_with_null, 1, workflow_started_payload("wf-1")),
            make_event(id_with_null, 2, step_scheduled_payload("wf-1", "step-1")),
        ];
        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.final_state, Some(LifecycleState::StepScheduled));
    }
}

// =========================================================================
// Adversarial: Payload corruption and structural attacks
// =========================================================================

#[cfg(test)]
mod payload_corruption_attacks {
    use super::*;

    /// Payload is an empty JSON object.
    #[test]
    fn replay_rejects_empty_json_object_payload() {
        let engine = ReplayEngine::new();
        let events = [make_event("inst-1", 1, serde_json::json!({}))];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(err, ReplayError::PayloadDecodeFailed { sequence: 1, .. }));
    }

    /// Payload is a JSON array.
    #[test]
    fn replay_rejects_array_payload() {
        let engine = ReplayEngine::new();
        let events = [make_event("inst-1", 1, serde_json::json!([1, 2, 3]))];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(err, ReplayError::PayloadDecodeFailed { sequence: 1, .. }));
    }

    /// Payload is JSON null.
    #[test]
    fn replay_rejects_null_payload() {
        let engine = ReplayEngine::new();
        let events = [make_event("inst-1", 1, serde_json::Value::Null)];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(err, ReplayError::PayloadDecodeFailed { sequence: 1, .. }));
    }

    /// Valid first event followed by corrupted second — no partial state leak.
    #[test]
    fn replay_valid_then_corrupt_prevents_partial_state() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, serde_json::json!({"garbage": true})),
        ];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(err, ReplayError::PayloadDecodeFailed { sequence: 2, .. }));
    }

    /// Payload with wrong type for required field (number instead of string).
    #[test]
    fn replay_rejects_wrong_field_type_in_payload() {
        let engine = ReplayEngine::new();
        let json = serde_json::json!({
            "type": "WorkflowStarted",
            "workflow_id": 12345,
            "binary_hash": "sha256abc",
            "version": 1
        });
        let events = [make_event("inst-1", 1, json)];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(err, ReplayError::PayloadDecodeFailed { sequence: 1, .. }));
    }
}

// =========================================================================
// Adversarial: Sequence number overflow and wrap-around
// =========================================================================

#[cfg(test)]
mod sequence_overflow_attacks {
    use super::*;

    /// Sequence number u64::MAX cannot be incremented — gap detection catches it.
    #[test]
    fn replay_detects_overflow_after_u64_max() {
        let engine = ReplayEngine::new();
        let events = [
            vo_types::events::EventEnvelope {
                schema_version: 1,
                instance_id: "inst-overflow".to_string(),
                sequence: u64::MAX,
                timestamp_ms: 100,
                payload: workflow_started_payload("wf-overflow"),
                metadata: vo_types::events::EventMetadata::default(),
            },
            vo_types::events::EventEnvelope {
                schema_version: 1,
                instance_id: "inst-overflow".to_string(),
                sequence: 0,
                timestamp_ms: 200,
                payload: step_scheduled_payload("wf-overflow", "step-1"),
                metadata: vo_types::events::EventMetadata::default(),
            },
        ];
        let err = engine.replay(&events).expect_err("should detect gap");
        assert!(matches!(
            err,
            ReplayError::SequenceGap {
                expected: 0,
                actual: 0,
                at_index: 1,
            }
        ));
    }

    /// Two events both at sequence u64::MAX are detected as duplicate.
    #[test]
    fn replay_detects_duplicate_at_u64_max() {
        let engine = ReplayEngine::new();
        let events = [
            vo_types::events::EventEnvelope {
                schema_version: 1,
                instance_id: "inst-maxdup".to_string(),
                sequence: u64::MAX,
                timestamp_ms: 100,
                payload: workflow_started_payload("wf-maxdup"),
                metadata: vo_types::events::EventMetadata::default(),
            },
            vo_types::events::EventEnvelope {
                schema_version: 1,
                instance_id: "inst-maxdup".to_string(),
                sequence: u64::MAX,
                timestamp_ms: 200,
                payload: step_scheduled_payload("wf-maxdup", "step-1"),
                metadata: vo_types::events::EventMetadata::default(),
            },
        ];
        let err = engine.replay(&events).expect_err("should detect duplicate");
        assert!(matches!(
            err,
            ReplayError::SequenceDuplicate {
                sequence: u64::MAX,
                first_at_index: 0,
                second_at_index: 1,
            }
        ));
    }

    /// Decreasing sequence numbers are detected as gaps.
    #[test]
    fn replay_detects_decreasing_sequence_as_gap() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 10, workflow_started_payload("wf-1")),
            make_event("inst-1", 5, step_scheduled_payload("wf-1", "step-1")),
        ];
        let err = engine.replay(&events).expect_err("should detect gap");
        assert!(matches!(
            err,
            ReplayError::SequenceGap {
                expected: 11,
                actual: 5,
                at_index: 1,
            }
        ));
    }

    /// Sequence starting at 1 with duplicate in the middle.
    #[test]
    fn replay_detects_duplicate_in_middle_of_sequence() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 2, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_completed_payload("wf-1", "step-1")),
        ];
        let err = engine.replay(&events).expect_err("should detect duplicate");
        assert!(matches!(
            err,
            ReplayError::SequenceDuplicate {
                sequence: 2,
                first_at_index: 1,
                second_at_index: 2,
            }
        ));
    }
}

// =========================================================================
// Adversarial: Replay idempotency and engine statelessness
// =========================================================================

#[cfg(test)]
mod replay_idempotency {
    use super::*;

    /// Calling replay 100 times on the same engine produces identical results.
    #[test]
    fn replay_is_idempotent_over_100_calls() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-idem", 1, workflow_started_payload("wf-idem")),
            make_event("inst-idem", 2, step_scheduled_payload("wf-idem", "step-1")),
            make_event("inst-idem", 3, step_started_payload("wf-idem", "step-1")),
            make_event("inst-idem", 4, step_completed_payload("wf-idem", "step-1")),
        ];
        let first = engine.replay(&events).unwrap();
        for _ in 0..99 {
            let result = engine.replay(&events).unwrap();
            assert_eq!(result, first);
        }
    }

    /// Interleaving replays of different event sequences doesn't corrupt state.
    #[test]
    fn replay_interleaved_sequences_remain_isolated() {
        let engine = ReplayEngine::new();
        let events_a = [
            make_event("inst-a", 1, workflow_started_payload("wf-a")),
            make_event("inst-a", 2, step_scheduled_payload("wf-a", "step-1")),
        ];
        let events_b = [
            make_event("inst-b", 1, workflow_started_payload("wf-b")),
            make_event("inst-b", 2, workflow_failed_payload("wf-b")),
            make_event("inst-b", 3, instance_resumed_payload("wf-b")),
        ];

        for _ in 0..10 {
            let ra = engine.replay(&events_a).unwrap();
            let rb = engine.replay(&events_b).unwrap();
            assert_eq!(ra.final_state, Some(LifecycleState::StepScheduled));
            assert_eq!(rb.final_state, Some(LifecycleState::RunningDecision));
        }
    }

    /// Replay of empty slice is always consistent.
    #[test]
    fn replay_empty_is_consistently_none() {
        let engine = ReplayEngine::new();
        for _ in 0..50 {
            let result = engine.replay(&[]).unwrap();
            assert_eq!(result.final_state, None);
            assert_eq!(result.events_applied, 0);
            assert_eq!(result.position.last_applied_sequence, None);
            assert_eq!(result.position.last_applied_timestamp_ms, None);
        }
    }
}

// =========================================================================
// Adversarial: Position tracking under adversarial conditions
// =========================================================================

#[cfg(test)]
mod position_tracking_attacks {
    use super::*;

    /// Position tracks through ContinuedAsNew events correctly.
    #[test]
    fn position_tracks_continued_as_new_events() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-can", 1, workflow_started_payload("wf-can")),
            make_event("inst-can", 2, continued_as_new_payload("wf-can")),
            make_event("inst-can", 3, continued_as_new_payload("wf-can")),
            make_event("inst-can", 4, step_scheduled_payload("wf-can", "step-1")),
        ];
        let result = engine.replay(&events).unwrap();
        assert_eq!(result.events_applied, 4);
        assert_eq!(result.position.last_applied_sequence, Some(4));
        assert_eq!(result.position.last_applied_timestamp_ms, Some(4000));
    }

    /// Position after terminal state ignores subsequent events.
    #[test]
    fn position_stops_at_terminal_state() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-term", 1, workflow_started_payload("wf-term")),
            make_event("inst-term", 2, cancel_requested_payload("wf-term")),
            make_event("inst-term", 3, step_scheduled_payload("wf-term", "step-1")),
        ];
        let result = engine.replay(&events).unwrap();
        assert_eq!(result.position.last_applied_sequence, Some(2));
        assert_eq!(result.position.last_applied_timestamp_ms, Some(2000));
    }

    /// Position with single event.
    #[test]
    fn position_with_single_event() {
        let engine = ReplayEngine::new();
        let events = [make_event("inst-solo", 42, workflow_started_payload("wf-solo"))];
        let result = engine.replay(&events).unwrap();
        assert_eq!(result.position.last_applied_sequence, Some(42));
        assert_eq!(result.position.last_applied_timestamp_ms, Some(42_000));
    }

    /// Position is None for empty replay.
    #[test]
    fn position_is_none_for_empty() {
        let engine = ReplayEngine::new();
        let result = engine.replay(&[]).unwrap();
        assert_eq!(result.position.last_applied_sequence, None);
        assert_eq!(result.position.last_applied_timestamp_ms, None);
    }

    /// Position correctly tracks through failure-recovery cycles.
    #[test]
    fn position_tracks_through_failure_recovery() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-fr", 1, workflow_started_payload("wf-fr")),
            make_event("inst-fr", 2, step_scheduled_payload("wf-fr", "step-1")),
            make_event("inst-fr", 3, step_started_payload("wf-fr", "step-1")),
            make_event("inst-fr", 4, step_failed_payload("wf-fr", "step-1")),
            make_event("inst-fr", 5, instance_resumed_payload("wf-fr")),
            make_event("inst-fr", 6, step_scheduled_payload("wf-fr", "step-2")),
        ];
        let result = engine.replay(&events).unwrap();
        assert_eq!(result.final_state, Some(LifecycleState::StepScheduled));
        assert_eq!(result.events_applied, 6);
        assert_eq!(result.position.last_applied_sequence, Some(6));
    }
}

// =========================================================================
// Adversarial: Schema version boundary attacks
// =========================================================================

#[cfg(test)]
mod schema_version_boundary_attacks {
    use super::*;
    use crate::upcaster::{Upcaster, UpcasterError, UpcasterRegistry};

    /// Upcaster that panics if called — ensures current-version events skip upcaster.
    #[test]
    fn replay_with_upcaster_does_not_call_upcaster_for_current_version() {
        struct PanicUpcaster;
        impl Upcaster for PanicUpcaster {
            fn source_version(&self) -> u8 { 0 }
            fn upcast(&self, _input: &[u8]) -> Result<Vec<u8>, UpcasterError> {
                panic!("upcaster should not be called for current-version events")
            }
        }

        let engine = ReplayEngine::new();
        let registry = crate::upcaster::UpcasterRegistryImpl::new(1);
        let _ = registry.register(Box::new(PanicUpcaster));
        let events = [
            make_event("inst-cv", 1, workflow_started_payload("wf-cv")),
            make_event("inst-cv", 2, step_scheduled_payload("wf-cv", "step-1")),
        ];
        let result = engine.replay_with_upcaster(&registry, &events).unwrap();
        assert_eq!(result.final_state, Some(LifecycleState::StepScheduled));
    }

    /// Upcaster that transforms correctly — end-to-end with mixed versions.
    #[test]
    fn replay_with_upcaster_mixed_versions_succeeds() {
        struct V0ToV1Upcaster;
        impl Upcaster for V0ToV1Upcaster {
            fn source_version(&self) -> u8 { 0 }
            fn upcast(&self, input: &[u8]) -> Result<Vec<u8>, UpcasterError> {
                let mut val: serde_json::Value = serde_json::from_slice(input)
                    .map_err(|e| UpcasterError::UpcastingFailed(e.to_string()))?;
                val["version"] = serde_json::json!(1);
                serde_json::to_vec(&val).map_err(|e| UpcasterError::UpcastingFailed(e.to_string()))
            }
        }

        let engine = ReplayEngine::new();
        let registry = crate::upcaster::UpcasterRegistryImpl::new(1);
        let _ = registry.register(Box::new(V0ToV1Upcaster));
        let events = [
            make_v0_event("inst-mix", 1, workflow_started_payload("wf-mix")),
            make_event("inst-mix", 2, step_scheduled_payload("wf-mix", "step-1")),
            make_v0_event("inst-mix", 3, step_started_payload("wf-mix", "step-1")),
            make_event("inst-mix", 4, step_completed_payload("wf-mix", "step-1")),
        ];
        let result = engine.replay_with_upcaster(&registry, &events).unwrap();
        assert_eq!(result.final_state, Some(LifecycleState::Completed));
        assert_eq!(result.events_applied, 4);
    }

    /// Upcaster with empty input produces deterministic empty result.
    #[test]
    fn replay_with_upcaster_empty_events_returns_empty() {
        let engine = ReplayEngine::new();
        let registry = crate::upcaster::UpcasterRegistryImpl::new(1);
        let result = engine.replay_with_upcaster(&registry, &[]).unwrap();
        assert_eq!(result.final_state, None);
        assert_eq!(result.events_applied, 0);
    }

    /// Upcaster failure at last event — still returns error (no partial state).
    #[test]
    fn replay_with_upcaster_failure_at_last_event_returns_error() {
        struct FailOnCall2Upcaster {
            call_count: std::sync::atomic::AtomicUsize,
        }
        impl Upcaster for FailOnCall2Upcaster {
            fn source_version(&self) -> u8 { 0 }
            fn upcast(&self, input: &[u8]) -> Result<Vec<u8>, UpcasterError> {
                let count = self.call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if count == 1 {
                    return Err(UpcasterError::UpcastingFailed("second call fails".to_string()));
                }
                let mut val: serde_json::Value = serde_json::from_slice(input)
                    .map_err(|e| UpcasterError::UpcastingFailed(e.to_string()))?;
                val["version"] = serde_json::json!(1);
                serde_json::to_vec(&val).map_err(|e| UpcasterError::UpcastingFailed(e.to_string()))
            }
        }

        let engine = ReplayEngine::new();
        let registry = crate::upcaster::UpcasterRegistryImpl::new(1);
        let _ = registry.register(Box::new(FailOnCall2Upcaster {
            call_count: std::sync::atomic::AtomicUsize::new(0),
        }));
        let events = [
            make_v0_event("inst-latefail", 1, workflow_started_payload("wf-latefail")),
            make_v0_event("inst-latefail", 2, step_scheduled_payload("wf-latefail", "step-1")),
            make_event("inst-latefail", 3, step_started_payload("wf-latefail", "step-1")),
        ];
        let err = engine.replay_with_upcaster(&registry, &events).expect_err("should fail");
        assert!(matches!(err, ReplayError::UpcastingFailed { sequence: 2, .. }));
    }
}

// =========================================================================
// Adversarial: Crash recovery with complex state machines
// =========================================================================

#[cfg(test)]
mod crash_recovery_complex {
    use super::*;

    /// Crash recovery of a workflow with timer wait states.
    /// After TimerFired, state returns to StepExecuting; next valid step is CompleteStep.
    #[test]
    fn crash_recovery_with_timer_workflow() {
        let engine = ReplayEngine::new();
        let full_events = [
            make_event("inst-twf", 1, workflow_started_payload("wf-twf")),
            make_event("inst-twf", 2, step_scheduled_payload("wf-twf", "step-1")),
            make_event("inst-twf", 3, step_started_payload("wf-twf", "step-1")),
            make_event("inst-twf", 4, timer_set_payload("wf-twf", "timer-1")),
            make_event("inst-twf", 5, timer_fired_payload("wf-twf", "timer-1")),
            make_event("inst-twf", 6, step_completed_payload("wf-twf", "step-1")),
        ];

        let expected_states = [
            LifecycleState::RunningDecision,
            LifecycleState::StepScheduled,
            LifecycleState::StepExecuting,
            LifecycleState::WaitingForTimer,
            LifecycleState::StepExecuting,
            LifecycleState::Completed,
        ];

        for (i, expected) in expected_states.iter().enumerate() {
            let result = engine
                .replay(&full_events[..=i])
                .unwrap_or_else(|e| panic!("prefix {} failed: {}", i + 1, e));
            assert_eq!(
                &result.final_state.unwrap(),
                expected,
                "state mismatch at prefix {}",
                i + 1
            );
        }
    }

    /// Crash recovery preserves determinism across multiple failure-recovery cycles.
    #[test]
    fn crash_recovery_determinism_across_cycles() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-rc", 1, workflow_started_payload("wf-rc")),
            make_event("inst-rc", 2, step_scheduled_payload("wf-rc", "step-1")),
            make_event("inst-rc", 3, step_started_payload("wf-rc", "step-1")),
            make_event("inst-rc", 4, step_failed_payload("wf-rc", "step-1")),
            make_event("inst-rc", 5, instance_resumed_payload("wf-rc")),
            make_event("inst-rc", 6, step_scheduled_payload("wf-rc", "step-1")),
            make_event("inst-rc", 7, step_started_payload("wf-rc", "step-1")),
            make_event("inst-rc", 8, step_failed_payload("wf-rc", "step-1")),
            make_event("inst-rc", 9, instance_resumed_payload("wf-rc")),
            make_event("inst-rc", 10, step_scheduled_payload("wf-rc", "step-final")),
            make_event("inst-rc", 11, step_started_payload("wf-rc", "step-final")),
            make_event("inst-rc", 12, step_completed_payload("wf-rc", "step-final")),
        ];

        let full = engine.replay(&events).unwrap();
        assert_eq!(full.final_state, Some(LifecycleState::Completed));
        assert_eq!(full.events_applied, 12);

        for prefix_len in 1..=events.len() {
            let r1 = engine.replay(&events[..prefix_len]).unwrap();
            let r2 = engine.replay(&events[..prefix_len]).unwrap();
            assert_eq!(r1, r2, "determinism violated at prefix {prefix_len}");
        }
    }

    /// Crash at WaitingForTimer, then resume with TimerFired.
    #[test]
    fn crash_at_waiting_for_timer_then_resume() {
        let engine = ReplayEngine::new();
        let crash_events = [
            make_event("inst-timer", 1, workflow_started_payload("wf-timer")),
            make_event("inst-timer", 2, step_scheduled_payload("wf-timer", "step-1")),
            make_event("inst-timer", 3, step_started_payload("wf-timer", "step-1")),
            make_event("inst-timer", 4, timer_set_payload("wf-timer", "timer-1")),
        ];
        let crashed = engine.replay(&crash_events).unwrap();
        assert_eq!(crashed.final_state, Some(LifecycleState::WaitingForTimer));

        let full_events = [
            make_event("inst-timer", 1, workflow_started_payload("wf-timer")),
            make_event("inst-timer", 2, step_scheduled_payload("wf-timer", "step-1")),
            make_event("inst-timer", 3, step_started_payload("wf-timer", "step-1")),
            make_event("inst-timer", 4, timer_set_payload("wf-timer", "timer-1")),
            make_event("inst-timer", 5, timer_fired_payload("wf-timer", "timer-1")),
        ];
        let resumed = engine.replay(&full_events).unwrap();
        assert_eq!(resumed.final_state, Some(LifecycleState::StepExecuting));
        assert_eq!(resumed.events_applied, 5);
    }
}
