use super::*;
use proptest::prelude::*;
use vo_types::events::EventEnvelope;

fn corrupt_payload_at_position(events: &mut [EventEnvelope], position: usize, corruption: &str) {
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
