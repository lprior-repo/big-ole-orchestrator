//! Adversarial tests for the replay engine (Red Queen).
//!
//! These tests probe edge cases, boundary conditions, and invalid inputs
//! to verify the replay engine handles adversarial conditions correctly.

use super::engine::ReplayEngine;
use super::test_helpers::*;
use super::types::ReplayError;
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
mod corrupted_payload_injection {
    use super::*;

    fn make_corrupted_event(instance_id: &str, sequence: u64, corruption: &str) -> EventEnvelope {
        make_event(
            instance_id,
            sequence,
            serde_json::json!({
                "type": corruption,
                "workflow_id": "wf-1",
                "version": 1
            }),
        )
    }

    fn make_truncated_payload_event(
        instance_id: &str,
        sequence: u64,
        partial_json: &str,
    ) -> EventEnvelope {
        EventEnvelope {
            schema_version: 1,
            instance_id: instance_id.to_string(),
            sequence,
            timestamp_ms: 1000 * sequence,
            payload: serde_json::json!({
                "type": "WorkflowStarted",
                "workflow_id": "wf-1",
                "binary_hash": "sha256abc",
                "workflow_version_hash": "wvhash123",
                "dedupe_key_hash": null,
                "version": 1
            }),
            metadata: vo_types::events::EventMetadata::default(),
        }
    }

    #[test]
    fn replay_rejects_corrupted_payload_at_sequence_2() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_corrupted_event("inst-1", 2, "InvalidEventType"),
            make_event("inst-1", 3, step_scheduled_payload("wf-1", "step-1")),
        ];
        let err = engine
            .replay(&events)
            .expect_err("should fail at corrupted event");
        assert!(matches!(
            err,
            ReplayError::PayloadDecodeFailed { sequence: 2, .. }
        ));
    }

    #[test]
    fn replay_rejects_corrupted_payload_at_sequence_3() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_corrupted_event("inst-1", 3, "UnknownEventType"),
        ];
        let err = engine
            .replay(&events)
            .expect_err("should fail at corrupted event");
        assert!(matches!(
            err,
            ReplayError::PayloadDecodeFailed { sequence: 3, .. }
        ));
    }

    #[test]
    fn replay_rejects_corrupted_payload_at_sequence_4() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_corrupted_event("inst-1", 4, "FakeEventType"),
        ];
        let err = engine
            .replay(&events)
            .expect_err("should fail at corrupted event");
        assert!(matches!(
            err,
            ReplayError::PayloadDecodeFailed { sequence: 4, .. }
        ));
    }

    #[test]
    fn replay_rejects_malformed_json_payload_at_sequence_2() {
        let engine = ReplayEngine::new();
        let json = serde_json::json!({
            "type": "WorkflowStarted",
            "workflow_id": "wf-1",
            "binary_hash": "sha256abc",
        });
        let mut events = vec![make_event("inst-1", 1, workflow_started_payload("wf-1"))];
        let mut corrupt_event = make_event("inst-1", 2, json);
        corrupt_event.payload = serde_json::Value::String("{malformed".to_string());
        events.push(corrupt_event);
        let err = engine
            .replay(&events)
            .expect_err("should fail at malformed json");
        assert!(matches!(
            err,
            ReplayError::PayloadDecodeFailed { sequence: 2, .. }
        ));
    }

    #[test]
    fn replay_rejects_null_type_field_at_sequence_3() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event(
                "inst-1",
                3,
                serde_json::json!({
                    "type": null,
                    "workflow_id": "wf-1",
                    "version": 1
                }),
            ),
        ];
        let err = engine
            .replay(&events)
            .expect_err("should fail at null type");
        assert!(matches!(
            err,
            ReplayError::PayloadDecodeFailed { sequence: 3, .. }
        ));
    }

    #[test]
    fn replay_rejects_wrong_type_for_required_field_at_sequence_2() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event(
                "inst-1",
                2,
                serde_json::json!({
                    "type": "StepScheduled",
                    "workflow_id": 123,
                    "step_id": "step-1",
                    "attempt": 1,
                    "fence": 1,
                    "execution_id": "exec-1",
                    "version": 1
                }),
            ),
        ];
        let err = engine
            .replay(&events)
            .expect_err("should fail at wrong type");
        assert!(matches!(
            err,
            ReplayError::PayloadDecodeFailed { sequence: 2, .. }
        ));
    }

    #[test]
    fn replay_rejects_negative_sequence_number() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", u64::MAX, step_started_payload("wf-1", "step-1")),
        ];
        let err = engine.replay(&events).expect_err("should fail");
        assert!(matches!(err, ReplayError::SequenceGap { .. }));
    }
}

#[cfg(test)]
mod exponential_blowup_scenarios {
    use super::*;

    #[test]
    fn replay_handles_deeply_nested_json_payload() {
        let engine = ReplayEngine::new();

        fn build_nested_json(depth: usize) -> serde_json::Value {
            if depth == 0 {
                serde_json::json!({"base": "value"})
            } else {
                serde_json::json!({
                    "nested": build_nested_json(depth - 1)
                })
            }
        }

        let deep_payload = build_nested_json(100);
        let json = serde_json::json!({
            "type": "WorkflowStarted",
            "workflow_id": "wf-1",
            "binary_hash": "sha256abc",
            "workflow_version_hash": "wvhash123",
            "dedupe_key_hash": null,
            "version": 1,
            "deep_data": deep_payload
        });

        let events = [make_event("inst-1", 1, json)];
        let result = engine
            .replay(&events)
            .expect("deeply nested should not blow up");
        assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
    }

    #[test]
    fn replay_handles_wide_json_payload() {
        let engine = ReplayEngine::new();

        let mut wide_obj = serde_json::Map::new();
        wide_obj.insert(
            "type".to_string(),
            serde_json::Value::String("WorkflowStarted".to_string()),
        );
        wide_obj.insert(
            "workflow_id".to_string(),
            serde_json::Value::String("wf-1".to_string()),
        );
        wide_obj.insert(
            "binary_hash".to_string(),
            serde_json::Value::String("sha256abc".to_string()),
        );
        wide_obj.insert(
            "workflow_version_hash".to_string(),
            serde_json::Value::String("wvhash123".to_string()),
        );
        wide_obj.insert("dedupe_key_hash".to_string(), serde_json::Value::Null);
        wide_obj.insert("version".to_string(), serde_json::Value::Number(1.into()));

        for i in 0..1000 {
            wide_obj.insert(
                format!("field_{}", i),
                serde_json::Value::String(format!("value_{}", i)),
            );
        }

        let json = serde_json::Value::Object(wide_obj);
        let events = [make_event("inst-1", 1, json)];
        let result = engine
            .replay(&events)
            .expect("wide payload should not blow up");
        assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
    }

    #[test]
    fn replay_handles_large_event_sequence_linear_time() {
        let engine = ReplayEngine::new();
        let mut events = Vec::new();

        events.push(make_event("inst-1", 1, workflow_started_payload("wf-1")));

        for i in 2..=1000 {
            let step_num = (i - 2) % 4;
            let payload = match step_num {
                0 => step_scheduled_payload("wf-1", &format!("step-{}", i)),
                1 => step_started_payload("wf-1", &format!("step-{}", i)),
                2 => step_completed_payload("wf-1", &format!("step-{}", i)),
                _ => step_scheduled_payload("wf-1", &format!("step-{}", i + 1)),
            };
            events.push(make_event("inst-1", i, payload));
        }

        let result = engine.replay(&events).expect("1000 events should replay");
        assert_eq!(result.events_applied, 1000);
    }

    #[test]
    fn replay_detects_sequence_gap_in_large_sequence() {
        let engine = ReplayEngine::new();
        let mut events = Vec::new();

        events.push(make_event("inst-1", 1, workflow_started_payload("wf-1")));

        for i in 2..=100 {
            if i == 50 {
                events.push(make_event(
                    "inst-1",
                    52,
                    step_scheduled_payload("wf-1", "step-50"),
                ));
            } else if i < 50 {
                events.push(make_event(
                    "inst-1",
                    i,
                    step_scheduled_payload("wf-1", &format!("step-{}", i)),
                ));
            } else {
                events.push(make_event(
                    "inst-1",
                    i + 1,
                    step_scheduled_payload("wf-1", &format!("step-{}", i)),
                ));
            }
        }

        let err = engine
            .replay(&events)
            .expect_err("should detect gap at 50->52");
        assert!(matches!(
            err,
            ReplayError::SequenceGap {
                expected: 51,
                actual: 52,
                at_index: 49
            }
        ));
    }
}

#[cfg(test)]
mod max_history_depth_boundary {
    use super::*;
    use vo_types::command_history::MAX_HISTORY_DEPTH;

    #[test]
    fn replay_handles_exactly_max_history_depth_events() {
        let engine = ReplayEngine::new();
        let mut events = Vec::new();

        events.push(make_event("inst-1", 1, workflow_started_payload("wf-1")));

        for i in 2..=MAX_HISTORY_DEPTH {
            let payload = step_scheduled_payload("wf-1", &format!("step-{}", i));
            events.push(make_event("inst-1", i as u64, payload));
        }

        let result = engine
            .replay(&events)
            .expect("MAX_HISTORY_DEPTH events should replay");
        assert_eq!(result.events_applied, MAX_HISTORY_DEPTH);
        assert_eq!(result.final_state, Some(LifecycleState::StepScheduled));
    }

    #[test]
    fn replay_handles_max_history_depth_plus_one_events() {
        let engine = ReplayEngine::new();
        let mut events = Vec::new();

        events.push(make_event("inst-1", 1, workflow_started_payload("wf-1")));

        for i in 2..=(MAX_HISTORY_DEPTH + 1) {
            let payload = step_scheduled_payload("wf-1", &format!("step-{}", i));
            events.push(make_event("inst-1", i as u64, payload));
        }

        let result = engine
            .replay(&events)
            .expect("MAX_HISTORY_DEPTH+1 events should replay");
        assert_eq!(result.events_applied, MAX_HISTORY_DEPTH + 1);
    }

    #[test]
    fn replay_handles_max_history_depth_with_failure_recovery() {
        let engine = ReplayEngine::new();
        let mut events = Vec::new();

        events.push(make_event("inst-1", 1, workflow_started_payload("wf-1")));

        for i in 2..=(MAX_HISTORY_DEPTH / 2) {
            events.push(make_event(
                "inst-1",
                ((i - 1) * 4 + 1) as u64,
                step_scheduled_payload("wf-1", &format!("step-{}", i * 4)),
            ));
            events.push(make_event(
                "inst-1",
                ((i - 1) * 4 + 2) as u64,
                step_started_payload("wf-1", &format!("step-{}", i * 4)),
            ));
            events.push(make_event(
                "inst-1",
                ((i - 1) * 4 + 3) as u64,
                step_failed_payload("wf-1", &format!("step-{}", i * 4)),
            ));
            events.push(make_event(
                "inst-1",
                ((i - 1) * 4 + 4) as u64,
                instance_resumed_payload("wf-1"),
            ));
        }

        let result = engine
            .replay(&events)
            .expect("deep failure recovery should work");
        assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
    }

    #[test]
    fn replay_stops_at_completed_before_max_history_depth() {
        let engine = ReplayEngine::new();
        let mut events = Vec::new();

        events.push(make_event("inst-1", 1, workflow_started_payload("wf-1")));

        for i in 2..=(MAX_HISTORY_DEPTH + 100) {
            let payload = if i == 4 {
                step_completed_payload("wf-1", "step-1")
            } else if i > 4 {
                step_scheduled_payload("wf-1", &format!("step-{}", i))
            } else {
                step_scheduled_payload("wf-1", &format!("step-{}", i))
            };
            events.push(make_event("inst-1", i as u64, payload));
        }

        let result = engine.replay(&events).expect("should stop at completed");
        assert_eq!(result.final_state, Some(LifecycleState::Completed));
        assert_eq!(result.events_applied, 4);
    }
}

#[cfg(test)]
mod mismatched_instance_id_injection {
    use super::*;

    #[test]
    fn replay_rejects_instance_id_switch_at_sequence_2() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-2", 2, step_scheduled_payload("wf-1", "step-1")),
        ];
        let err = engine
            .replay(&events)
            .expect_err("should fail at instance mismatch");
        assert!(matches!(
            err,
            ReplayError::InstanceMismatch {
                expected: _,
                actual: _
            }
        ));
    }

    #[test]
    fn replay_rejects_instance_id_switch_at_sequence_3() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-2", 3, step_started_payload("wf-1", "step-1")),
        ];
        let err = engine
            .replay(&events)
            .expect_err("should fail at instance mismatch");
        assert!(matches!(
            err,
            ReplayError::InstanceMismatch {
                expected: _,
                actual: _
            }
        ));
    }

    #[test]
    fn replay_rejects_instance_id_switch_at_sequence_4() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-3", 4, step_completed_payload("wf-1", "step-1")),
        ];
        let err = engine
            .replay(&events)
            .expect_err("should fail at instance mismatch");
        assert!(matches!(
            err,
            ReplayError::InstanceMismatch {
                expected: _,
                actual: _
            }
        ));
    }

    #[test]
    fn replay_rejects_whitespace_instance_id_variant() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1 ", 3, step_started_payload("wf-1", "step-1")),
        ];
        let err = engine
            .replay(&events)
            .expect_err("should fail - trailing space");
        assert!(matches!(err, ReplayError::InstanceMismatch { .. }));
    }

    #[test]
    fn replay_rejects_case_mismatch_instance_id() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("Inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        ];
        let err = engine
            .replay(&events)
            .expect_err("should fail - case mismatch");
        assert!(matches!(err, ReplayError::InstanceMismatch { .. }));
    }

    #[test]
    fn replay_rejects_empty_instance_id_at_sequence_2() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("", 2, step_scheduled_payload("wf-1", "step-1")),
        ];
        let err = engine
            .replay(&events)
            .expect_err("should fail - empty instance_id");
        assert!(matches!(err, ReplayError::InstanceMismatch { .. }));
    }

    #[test]
    fn replay_rejects_instance_id_change_after_failure_recovery() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
            make_event("inst-1", 5, instance_resumed_payload("wf-1")),
            make_event("inst-2", 6, step_scheduled_payload("wf-1", "step-1")),
        ];
        let err = engine
            .replay(&events)
            .expect_err("should fail at instance mismatch after recovery");
        assert!(matches!(err, ReplayError::InstanceMismatch { .. }));
    }
}

#[cfg(test)]
mod memory_pressure_with_large_payloads {
    use super::*;

    fn make_large_payload_event(
        instance_id: &str,
        sequence: u64,
        payload_size_bytes: usize,
    ) -> EventEnvelope {
        let large_string = "x".repeat(payload_size_bytes);
        make_event(
            instance_id,
            sequence,
            serde_json::json!({
                "type": "WorkflowStarted",
                "workflow_id": "wf-1",
                "binary_hash": "sha256abc",
                "workflow_version_hash": "wvhash123",
                "dedupe_key_hash": null,
                "version": 1,
                "large_field": large_string
            }),
        )
    }

    fn make_large_nested_payload_event(
        instance_id: &str,
        sequence: u64,
        num_nested_objects: usize,
    ) -> EventEnvelope {
        let mut nested = serde_json::json!({"value": "leaf"});
        for _ in 0..num_nested_objects {
            nested = serde_json::json!({"nested": nested});
        }
        make_event(
            instance_id,
            sequence,
            serde_json::json!({
                "type": "WorkflowStarted",
                "workflow_id": "wf-1",
                "binary_hash": "sha256abc",
                "workflow_version_hash": "wvhash123",
                "dedupe_key_hash": null,
                "version": 1,
                "nested_data": nested
            }),
        )
    }

    #[test]
    fn replay_handles_1mb_payload() {
        let engine = ReplayEngine::new();
        let events = [make_large_payload_event("inst-1", 1, 1_000_000)];
        let result = engine.replay(&events).expect("1MB payload should not OOM");
        assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
    }

    #[test]
    fn replay_handles_10mb_payload() {
        let engine = ReplayEngine::new();
        let events = [make_large_payload_event("inst-1", 1, 10_000_000)];
        let result = engine.replay(&events).expect("10MB payload should not OOM");
        assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
    }

    #[test]
    fn replay_handles_multiple_large_payloads_in_sequence() {
        let engine = ReplayEngine::new();
        let events = [
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_large_payload_event("inst-1", 2, 1_000_000),
            make_event("inst-1", 3, step_scheduled_payload("wf-1", "step-1")),
            make_large_payload_event("inst-1", 4, 1_000_000),
        ];
        let result = engine
            .replay(&events)
            .expect("multiple 1MB payloads should not OOM");
        assert_eq!(result.final_state, Some(LifecycleState::StepScheduled));
    }

    #[test]
    fn replay_handles_deeply_nested_structure_1000_levels() {
        let engine = ReplayEngine::new();
        let events = [make_large_nested_payload_event("inst-1", 1, 1000)];
        let result = engine
            .replay(&events)
            .expect("1000 levels nested should not blow stack");
        assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
    }

    #[test]
    fn replay_rejects_corrupted_payload_between_large_payloads() {
        let engine = ReplayEngine::new();
        let events = [
            make_large_payload_event("inst-1", 1, 1_000_000),
            make_event(
                "inst-1",
                2,
                serde_json::json!({
                    "type": "InvalidGarbageType",
                    "workflow_id": "wf-1",
                    "version": 1
                }),
            ),
            make_large_payload_event("inst-1", 3, 1_000_000),
        ];
        let err = engine
            .replay(&events)
            .expect_err("should fail at corrupted event between large payloads");
        assert!(matches!(
            err,
            ReplayError::PayloadDecodeFailed { sequence: 2, .. }
        ));
    }

    #[test]
    fn replay_handles_100_events_each_100kb() {
        let engine = ReplayEngine::new();
        let mut events = Vec::with_capacity(100);
        events.push(make_event("inst-1", 1, workflow_started_payload("wf-1")));

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
