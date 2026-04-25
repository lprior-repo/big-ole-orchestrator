use super::*;
use vo_types::events::EventEnvelope;

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
    _partial_json: &str,
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
