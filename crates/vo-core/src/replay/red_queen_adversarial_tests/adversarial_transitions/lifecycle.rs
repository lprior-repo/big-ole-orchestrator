use super::super::*;
use vo_types::state::LifecycleState;

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
