use super::*;
use vo_types::events::EventEnvelope;
use vo_types::state::LifecycleState;

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
