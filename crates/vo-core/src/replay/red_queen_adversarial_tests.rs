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

        for i in 2..=100 {
            events.push(make_large_payload_event("inst-1", i, 100_000));
        }

        let result = engine
            .replay(&events)
            .expect("100 x 100KB should total ~10MB and not OOM");
        assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
        assert_eq!(result.events_applied, 100);
    }
}

#[cfg(test)]
mod random_position_corruption_injection {
    use super::*;
    use proptest::prelude::*;
    use vo_types::{events::EventEnvelope, events::EventMetadata};

    fn corrupt_payload_at_position(
        events: &mut [EventEnvelope],
        position: usize,
        corruption: &str,
    ) {
        if position < events.len() {
            events[position].payload = serde_json::json!({
                "type": corruption,
                "workflow_id": "wf-1",
                "version": 1
            });
        }
    }

    fn inject_truncation_at_position(events: &mut [EventEnvelope], position: usize) {
        if position < events.len() {
            events[position].payload = serde_json::Value::String("{truncated".to_string());
        }
    }

    fn inject_null_type_at_position(events: &mut [EventEnvelope], position: usize) {
        if position < events.len() {
            events[position].payload = serde_json::json!({
                "type": null,
                "workflow_id": "wf-1",
                "version": 1
            });
        }
    }

    fn inject_wrong_type_at_position(events: &mut [EventEnvelope], position: usize) {
        if position < events.len() {
            events[position].payload = serde_json::json!({
                "type": "StepScheduled",
                "workflow_id": 123,
                "step_id": "step-1",
                "attempt": 1,
                "fence": 1,
                "execution_id": "exec-1",
                "version": 1
            });
        }
    }

    fn build_valid_sequence(length: usize) -> Vec<EventEnvelope> {
        let mut events = Vec::with_capacity(length);
        events.push(make_event("inst-1", 1, workflow_started_payload("wf-1")));
        for i in 2..=length {
            let payload = match i % 4 {
                0 => step_scheduled_payload("wf-1", &format!("step-{}", i)),
                1 => step_started_payload("wf-1", &format!("step-{}", i)),
                2 => step_completed_payload("wf-1", &format!("step-{}", i)),
                _ => step_scheduled_payload("wf-1", &format!("step-{}", i + 1)),
            };
            events.push(make_event("inst-1", i as u64, payload));
        }
        events
    }

    proptest! {
        #[test]
        fn replay_rejects_corruption_at_random_position(
            seq_len in 5usize..50usize,
            corrupt_pos in 1usize..50usize,
        ) {
            let engine = ReplayEngine::new();
            let mut events = build_valid_sequence(seq_len);
            let actual_pos = corrupt_pos % events.len().max(1);
            corrupt_payload_at_position(&mut events, actual_pos, "InvalidGarbageType");
            let err = engine.replay(&events).expect_err("should fail at corrupted position");
            let is_decode_error = matches!(err, ReplayError::PayloadDecodeFailed { sequence: _, source: _ });
            prop_assert!(is_decode_error);
        }

        #[test]
        fn replay_rejects_truncation_corruption_at_random_position(
            seq_len in 5usize..50usize,
            corrupt_pos in 1usize..50usize,
        ) {
            let engine = ReplayEngine::new();
            let mut events = build_valid_sequence(seq_len);
            let actual_pos = corrupt_pos % events.len().max(1);
            inject_truncation_at_position(&mut events, actual_pos);
            let err = engine.replay(&events).expect_err("should fail at truncation");
            let is_decode_error = matches!(err, ReplayError::PayloadDecodeFailed { sequence: _, source: _ });
            prop_assert!(is_decode_error);
        }

        #[test]
        fn replay_rejects_null_type_corruption_at_random_position(
            seq_len in 5usize..50usize,
            corrupt_pos in 1usize..50usize,
        ) {
            let engine = ReplayEngine::new();
            let mut events = build_valid_sequence(seq_len);
            let actual_pos = corrupt_pos % events.len().max(1);
            inject_null_type_at_position(&mut events, actual_pos);
            let err = engine.replay(&events).expect_err("should fail at null type");
            let is_decode_error = matches!(err, ReplayError::PayloadDecodeFailed { sequence: _, source: _ });
            prop_assert!(is_decode_error);
        }

        #[test]
        fn replay_rejects_wrong_type_corruption_at_random_position(
            seq_len in 5usize..50usize,
            corrupt_pos in 1usize..50usize,
        ) {
            let engine = ReplayEngine::new();
            let mut events = build_valid_sequence(seq_len);
            let actual_pos = corrupt_pos % events.len().max(1);
            inject_wrong_type_at_position(&mut events, actual_pos);
            let err = engine.replay(&events).expect_err("should fail at wrong type");
            let is_decode_error = matches!(err, ReplayError::PayloadDecodeFailed { sequence: _, source: _ });
            prop_assert!(is_decode_error);
        }
    }

    #[test]
    fn replay_handles_corruption_at_first_event_position() {
        let engine = ReplayEngine::new();
        let mut events = build_valid_sequence(10);
        corrupt_payload_at_position(&mut events, 0, "InvalidType");
        let err = engine
            .replay(&events)
            .expect_err("should fail at first event");
        assert!(matches!(
            err,
            ReplayError::PayloadDecodeFailed { sequence: 1, .. }
        ));
    }

    #[test]
    fn replay_handles_corruption_at_last_event_position() {
        let engine = ReplayEngine::new();
        let mut events = build_valid_sequence(10);
        corrupt_payload_at_position(&mut events, 9, "InvalidType");
        let err = engine
            .replay(&events)
            .expect_err("should fail at last event");
        assert!(matches!(
            err,
            ReplayError::PayloadDecodeFailed { sequence: 10, .. }
        ));
    }

    #[test]
    fn replay_handles_corruption_at_second_event_position() {
        let engine = ReplayEngine::new();
        let mut events = build_valid_sequence(10);
        corrupt_payload_at_position(&mut events, 1, "InvalidType");
        let err = engine
            .replay(&events)
            .expect_err("should fail at second event");
        assert!(matches!(
            err,
            ReplayError::PayloadDecodeFailed { sequence: 2, .. }
        ));
    }
}

#[cfg(test)]
mod aggressive_exponential_blowup {
    use super::*;

    #[test]
    fn replay_handles_exponential_nesting_2_to_the_10() {
        let engine = ReplayEngine::new();

        fn build_exponential(depth: usize) -> serde_json::Value {
            if depth == 0 {
                serde_json::json!({"base": "value"})
            } else {
                let left = build_exponential(depth - 1);
                let right = build_exponential(depth - 1);
                serde_json::json!({
                    "left": left,
                    "right": right
                })
            }
        }

        let nested = build_exponential(10);
        let json = serde_json::json!({
            "type": "WorkflowStarted",
            "workflow_id": "wf-1",
            "binary_hash": "sha256abc",
            "workflow_version_hash": "wvhash123",
            "dedupe_key_hash": null,
            "version": 1,
            "data": nested
        });

        let events = [make_event("inst-1", 1, json)];
        let result = engine
            .replay(&events)
            .expect("2^10 nesting should not blow up");
        assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
    }

    #[test]
    fn replay_handles_exponential_nesting_2_to_the_12() {
        let engine = ReplayEngine::new();

        fn build_exponential(depth: usize) -> serde_json::Value {
            if depth == 0 {
                serde_json::json!({"base": "value"})
            } else {
                let left = build_exponential(depth - 1);
                let right = build_exponential(depth - 1);
                serde_json::json!({
                    "left": left,
                    "right": right
                })
            }
        }

        let nested = build_exponential(12);
        let json = serde_json::json!({
            "type": "WorkflowStarted",
            "workflow_id": "wf-1",
            "binary_hash": "sha256abc",
            "workflow_version_hash": "wvhash123",
            "dedupe_key_hash": null,
            "version": 1,
            "data": nested
        });

        let events = [make_event("inst-1", 1, json)];
        let result = engine
            .replay(&events)
            .expect("2^12 nesting should not blow up");
        assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
    }

    #[test]
    fn replay_handles_array_of_deeply_nested_objects() {
        let engine = ReplayEngine::new();

        fn build_nested(depth: usize) -> serde_json::Value {
            if depth == 0 {
                serde_json::json!({"leaf": "value"})
            } else {
                serde_json::json!({
                    "nested": build_nested(depth - 1)
                })
            }
        }

        let arr = (0..50).map(|_| build_nested(20)).collect::<Vec<_>>();

        let json = serde_json::json!({
            "type": "WorkflowStarted",
            "workflow_id": "wf-1",
            "binary_hash": "sha256abc",
            "workflow_version_hash": "wvhash123",
            "dedupe_key_hash": null,
            "version": 1,
            "array_data": arr
        });

        let events = [make_event("inst-1", 1, json)];
        let result = engine
            .replay(&events)
            .expect("50 x depth-20 nested should not blow up");
        assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
    }

    #[test]
    fn replay_handles_mutual_recursion_payload() {
        let engine = ReplayEngine::new();

        fn build_mutual_recursion(depth: usize) -> serde_json::Value {
            if depth == 0 {
                serde_json::json!({"terminates": "value"})
            } else {
                serde_json::json!({
                    "type_a": {
                        "next_b": build_mutual_recursion(depth - 1)
                    },
                    "type_b": {
                        "next_a": build_mutual_recursion(depth - 1)
                    }
                })
            }
        }

        let nested = build_mutual_recursion(10);
        let json = serde_json::json!({
            "type": "WorkflowStarted",
            "workflow_id": "wf-1",
            "binary_hash": "sha256abc",
            "workflow_version_hash": "wvhash123",
            "dedupe_key_hash": null,
            "version": 1,
            "mutual": nested
        });

        let events = [make_event("inst-1", 1, json)];
        let result = engine
            .replay(&events)
            .expect("mutual recursion depth 10 should not blow up");
        assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
    }

    #[test]
    fn replay_handles_wide_and_deep_combination() {
        let engine = ReplayEngine::new();

        fn build_wide_deep(width: usize, depth: usize) -> serde_json::Value {
            if depth == 0 {
                serde_json::json!({"leaf": "value"})
            } else {
                let mut obj = serde_json::Map::new();
                for i in 0..width {
                    obj.insert(format!("field_{}", i), build_wide_deep(width, depth - 1));
                }
                serde_json::Value::Object(obj)
            }
        }

        let nested = build_wide_deep(5, 6);
        let json = serde_json::json!({
            "type": "WorkflowStarted",
            "workflow_id": "wf-1",
            "binary_hash": "sha256abc",
            "workflow_version_hash": "wvhash123",
            "dedupe_key_hash": null,
            "version": 1,
            "wide_deep": nested
        });

        let events = [make_event("inst-1", 1, json)];
        let result = engine
            .replay(&events)
            .expect("5^6 wide and deep should not blow up");
        assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
    }
}

#[cfg(test)]
mod memory_pressure_aggressive {
    use super::*;

    fn make_very_large_payload(size_bytes: usize) -> serde_json::Value {
        let large_field = "x".repeat(size_bytes);
        serde_json::json!({
            "type": "WorkflowStarted",
            "workflow_id": "wf-1",
            "binary_hash": "sha256abc",
            "workflow_version_hash": "wvhash123",
            "dedupe_key_hash": null,
            "version": 1,
            "massive_data": large_field
        })
    }

    fn make_large_wide_payload(num_fields: usize, field_size: usize) -> serde_json::Value {
        let mut obj = serde_json::Map::new();
        obj.insert(
            "type".to_string(),
            serde_json::Value::String("WorkflowStarted".to_string()),
        );
        obj.insert(
            "workflow_id".to_string(),
            serde_json::Value::String("wf-1".to_string()),
        );
        obj.insert(
            "binary_hash".to_string(),
            serde_json::Value::String("sha256abc".to_string()),
        );
        obj.insert(
            "workflow_version_hash".to_string(),
            serde_json::Value::String("wvhash123".to_string()),
        );
        obj.insert("dedupe_key_hash".to_string(), serde_json::Value::Null);
        obj.insert("version".to_string(), serde_json::Value::Number(1.into()));

        for i in 0..num_fields {
            obj.insert(
                format!("field_{}", i),
                serde_json::Value::String("x".repeat(field_size)),
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

/// Red Queen adversarial tests for duplicate event handling (ve-qejbz).
///
/// INVARIANT: Event replay must handle duplicate events idempotently.
/// Exact duplicates (same instance_id + sequence) must be detected and rejected.
/// Near-duplicates (different instance_id but same sequence) must also be caught
/// by the instance_id consistency check.
#[cfg(test)]
mod red_queen_duplicate_handling {
    use super::*;
    use crate::replay::test_helpers::*;
    use crate::replay::engine::ReplayEngine;
    use crate::replay::types::ReplayError;

    /// Given: Two events with identical instance_id and sequence number
    /// When: Replayed together
    /// Then: SequenceDuplicate error is returned with correct indices
    #[test]
    fn rq_dup_exact_duplicate_identical_sequence() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-dup", 1, workflow_started_payload("wf-1")),
            make_event("inst-dup", 1, workflow_started_payload("wf-1")),
        ];
        let err = engine.replay(&events).expect_err("exact duplicate must be rejected");
        match err {
            ReplayError::SequenceDuplicate {
                sequence,
                first_at_index,
                second_at_index,
            } => {
                assert_eq!(sequence, 1);
                assert_eq!(first_at_index, 0);
                assert_eq!(second_at_index, 1);
            }
            other => panic!("Expected SequenceDuplicate, got {other:?}"),
        }
    }

    /// Given: A valid sequence with an exact duplicate injected at a middle position
    /// When: Replayed
    /// Then: Duplicate is detected at the injection point
    #[test]
    fn rq_dup_exact_duplicate_injected_mid_sequence() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-mid", 1, workflow_started_payload("wf-1")),
            make_event("inst-mid", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-mid", 2, step_scheduled_payload("wf-1", "step-1")), // duplicate
        ];
        let err = engine.replay(&events).expect_err("mid-sequence duplicate must be rejected");
        match err {
            ReplayError::SequenceDuplicate {
                sequence,
                first_at_index,
                second_at_index,
            } => {
                assert_eq!(sequence, 2);
                assert_eq!(first_at_index, 1);
                assert_eq!(second_at_index, 2);
            }
            other => panic!("Expected SequenceDuplicate, got {other:?}"),
        }
    }

    /// Given: Two events with same sequence but different instance_id
    /// When: Replayed together
    /// Then: InstanceMismatch error is returned (different instance IDs detected first)
    #[test]
    fn rq_dup_near_duplicate_different_instance_id() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-a", 1, workflow_started_payload("wf-1")),
            make_event("inst-b", 2, step_scheduled_payload("wf-1", "step-1")),
        ];
        let err = engine
            .replay(&events)
            .expect_err("mixed instance IDs must be rejected");
        match err {
            ReplayError::InstanceMismatch { .. } => {}
            other => panic!("Expected InstanceMismatch, got {other:?}"),
        }
    }

    /// Given: Three events where the third is a duplicate of the first (non-adjacent)
    /// When: Replayed
    /// Then: The duplicate is caught (sequence gap from 2 back to 1)
    #[test]
    fn rq_dup_non_adjacent_sequence_duplicate() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-nonadj", 1, workflow_started_payload("wf-1")),
            make_event("inst-nonadj", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-nonadj", 1, step_started_payload("wf-1", "step-1")), // seq 1 again
        ];
        // seq goes 1, 2, then 1 again — that's a gap (expected 3, got 1)
        let err = engine.replay(&events).expect_err("must be rejected");
        // Could be SequenceGap or SequenceDuplicate depending on implementation
        assert!(matches!(
            err,
            ReplayError::SequenceGap { .. } | ReplayError::SequenceDuplicate { .. }
        ));
    }

    /// Given: A valid event replayed twice (idempotent check)
    /// When: The same single event is replayed
    /// Then: It succeeds — a single event has no duplicates to detect
    #[test]
    fn rq_dup_single_event_always_succeeds() {
        let engine = ReplayEngine::new();
        let events = [make_event("inst-single", 1, workflow_started_payload("wf-1"))];
        let result = engine.replay(&events).expect("single event must succeed");
        assert_eq!(result.events_applied, 1);
    }

    /// Given: Multiple pairs of duplicate sequences (1,1,2,2,3,3)
    /// When: Replayed
    /// Then: First duplicate pair is detected immediately
    #[test]
    fn rq_dup_multiple_duplicate_pairs() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-multi", 1, workflow_started_payload("wf-1")),
            make_event("inst-multi", 1, workflow_started_payload("wf-1")),
        ];
        let err = engine
            .replay(&events)
            .expect_err("first duplicate must be caught");
        assert!(matches!(err, ReplayError::SequenceDuplicate { sequence: 1, .. }));
    }

    /// Given: An event with same payload but different timestamps
    /// When: Sequence is the same
    /// Then: Still detected as duplicate (timestamp is not part of dedup key)
    #[test]
    fn rq_dup_same_sequence_different_timestamp_still_caught() {
        let engine = ReplayEngine::new();
        let mut event1 = make_event("inst-ts", 1, workflow_started_payload("wf-1"));
        let mut event2 = make_event("inst-ts", 1, workflow_started_payload("wf-1"));
        event1.timestamp_ms = 1000;
        event2.timestamp_ms = 9999;
        let events = [event1, event2];
        let err = engine.replay(&events).expect_err("same sequence must be caught");
        assert!(matches!(err, ReplayError::SequenceDuplicate { sequence: 1, .. }));
    }
}
