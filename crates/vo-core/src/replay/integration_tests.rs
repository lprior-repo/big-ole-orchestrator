//! Integration tests for the replay engine.

use super::engine::ReplayEngine;
use super::test_helpers::*;
use vo_types::events::EventEnvelope;
use vo_types::state::LifecycleState;

// =========================================================================
// Integration: Real EventEnvelope wiring
// =========================================================================

#[test]
fn replay_works_with_real_event_envelope_serialization() {
    let engine = ReplayEngine::new();
    let json = serde_json::json!({
        "version": 1,
        "instance_id": "inst-real",
        "sequence": 1,
        "timestamp_ms": 1000,
        "payload": {
            "type": "WorkflowStarted",
            "workflow_id": "wf-real",
            "binary_hash": "sha256abc",
            "version": 1
        },
        "metadata": {}
    });
    let bytes = serde_json::to_vec(&json).expect("serialize");
    let envelope = EventEnvelope::from_bytes(&bytes).expect("parse envelope");
    let result = engine.replay(&[envelope]).expect("replay");
    assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
    assert_eq!(result.events_applied, 1);
}

// =========================================================================
// Full lifecycle integration test
// =========================================================================

#[test]
fn replay_full_lifecycle_pending_to_completed() {
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
fn replay_full_lifecycle_with_timer_round_trip() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, timer_set_payload("wf-1", "timer-1")),
        make_event("inst-1", 5, timer_fired_payload("wf-1", "timer-1")),
        make_event("inst-1", 6, step_completed_payload("wf-1", "step-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Completed));
    assert_eq!(result.events_applied, 6);
}

#[test]
fn replay_failure_recovery_cycle() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
        make_event("inst-1", 5, instance_resumed_payload("wf-1")),
        make_event("inst-1", 6, step_scheduled_payload("wf-1", "step-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::StepScheduled));
    assert_eq!(result.events_applied, 6);
}
