//! Error-path unit tests for the replay engine (Behaviors 14–20).

use super::engine::ReplayEngine;
use super::test_helpers::*;
use super::types::{ReplayError, ReplayErrorKind};
use serde_json::json;
use vo_types::state::LifecycleState;

// =========================================================================
// Behavior 14: Instance mismatch error
// =========================================================================

#[test]
fn replay_returns_instance_mismatch_when_events_have_different_instance_ids() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-2", 2, step_scheduled_payload("wf-1", "step-1")),
    ];
    let err = engine.replay(&events).expect_err("should fail");
    assert_eq!(
        err,
        ReplayError::InstanceMismatch {
            expected: "inst-1".to_string(),
            actual: "inst-2".to_string(),
        }
    );
}

// =========================================================================
// Behavior 15: Sequence gap error
// =========================================================================

#[test]
fn replay_returns_sequence_gap_when_sequence_numbers_have_gap() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 3, step_scheduled_payload("wf-1", "step-1")),
    ];
    let err = engine.replay(&events).expect_err("should fail");
    assert_eq!(
        err,
        ReplayError::SequenceGap {
            expected: 2,
            actual: 3,
            at_index: 1,
        }
    );
}

// =========================================================================
// Behavior 16: Sequence duplicate error
// =========================================================================

#[test]
fn replay_returns_sequence_duplicate_when_duplicate_sequence_found() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 1, step_scheduled_payload("wf-1", "step-1")),
    ];
    let err = engine.replay(&events).expect_err("should fail");
    assert_eq!(
        err,
        ReplayError::SequenceDuplicate {
            sequence: 1,
            first_at_index: 0,
            second_at_index: 1,
        }
    );
}

// =========================================================================
// Behavior 17: Payload decode failure
// =========================================================================

#[test]
fn replay_returns_payload_decode_failed_when_payload_is_invalid() {
    let engine = ReplayEngine::new();
    let events = [make_event("inst-1", 1, json!({"type": "UnknownType"}))];
    let err = engine.replay(&events).expect_err("should fail");
    assert!(matches!(
        err,
        ReplayError::PayloadDecodeFailed { sequence: 1, .. }
    ));
}

// =========================================================================
// Behavior 18: Transition failure
// =========================================================================

#[test]
fn replay_returns_transition_failed_when_apply_rejects_transition() {
    let engine = ReplayEngine::new();
    // StepCompleted from Pending is invalid (no prior StepScheduled/StepStarted)
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_completed_payload("wf-1", "step-1")),
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

// =========================================================================
// Behavior 20: Terminal state stops processing
// =========================================================================

#[test]
fn replay_stops_processing_after_reaching_completed_state() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
        // This event should be ignored (terminal state reached)
        make_event("inst-1", 5, timer_set_payload("wf-1", "timer-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Completed));
    assert_eq!(result.events_applied, 4);
}

#[test]
fn replay_stops_processing_after_reaching_cancelled_state() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, workflow_cancelled_payload("wf-1")),
        make_event("inst-1", 3, step_scheduled_payload("wf-1", "step-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Cancelled));
    assert_eq!(result.events_applied, 2);
}

// =========================================================================
// Error Kind Mapping Tests (Behaviors 21–22)
// =========================================================================

#[test]
fn instance_mismatch_error_is_deterministic() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-2", 2, step_scheduled_payload("wf-1", "step-1")),
    ];
    let err = engine.replay(&events).expect_err("should fail");
    assert_eq!(err.kind(), ReplayErrorKind::Deterministic);
}

#[test]
fn sequence_gap_error_is_deterministic() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 3, step_scheduled_payload("wf-1", "step-1")),
    ];
    let err = engine.replay(&events).expect_err("should fail");
    assert_eq!(err.kind(), ReplayErrorKind::Deterministic);
}

#[test]
fn sequence_duplicate_error_is_deterministic() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 1, step_scheduled_payload("wf-1", "step-1")),
    ];
    let err = engine.replay(&events).expect_err("should fail");
    assert_eq!(err.kind(), ReplayErrorKind::Deterministic);
}

#[test]
fn payload_decode_failed_error_is_deterministic() {
    let engine = ReplayEngine::new();
    let events = [make_event("inst-1", 1, json!({"type": "UnknownType"}))];
    let err = engine.replay(&events).expect_err("should fail");
    assert_eq!(err.kind(), ReplayErrorKind::Deterministic);
}

#[test]
fn transition_failed_error_is_deterministic() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_completed_payload("wf-1", "step-1")),
    ];
    let err = engine.replay(&events).expect_err("should fail");
    assert_eq!(err.kind(), ReplayErrorKind::Deterministic);
}

// =========================================================================
// BlobPublicationFailed Error Tests (ADR-040 §3)
// =========================================================================

#[test]
fn blob_publication_failed_error_display() {
    let err = ReplayError::BlobPublicationFailed {
        sequence: 5,
        step_id: "step-1".to_string(),
        blob_id: "01H5JQX7K3R4T6V8W0X2Y4Z6A8".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("sequence 5"));
    assert!(msg.contains("step-1"));
    assert!(msg.contains("01H5JQX7K3R4T6V8W0X2Y4Z6A8"));
    assert!(msg.contains("Blob publication failed"));
}

#[test]
fn blob_publication_failed_error_equality() {
    let err1 = ReplayError::BlobPublicationFailed {
        sequence: 5,
        step_id: "step-1".to_string(),
        blob_id: "01H5JQX7K3R4T6V8W0X2Y4Z6A8".to_string(),
    };
    let err2 = ReplayError::BlobPublicationFailed {
        sequence: 5,
        step_id: "step-1".to_string(),
        blob_id: "01H5JQX7K3R4T6V8W0X2Y4Z6A8".to_string(),
    };
    let err3 = ReplayError::BlobPublicationFailed {
        sequence: 6,
        step_id: "step-1".to_string(),
        blob_id: "01H5JQX7K3R4T6V8W0X2Y4Z6A8".to_string(),
    };
    assert_eq!(err1, err2);
    assert_ne!(err1, err3);
}

#[test]
fn blob_publication_failed_error_is_deterministic() {
    let err = ReplayError::BlobPublicationFailed {
        sequence: 5,
        step_id: "step-1".to_string(),
        blob_id: "01H5JQX7K3R4T6V8W0X2Y4Z6A8".to_string(),
    };
    assert_eq!(err.kind(), ReplayErrorKind::Deterministic);
}
