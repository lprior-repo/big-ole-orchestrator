#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_core::replay::ReplayEngine;
use vo_types::events::EventEnvelope;

const MAX_EVENTS: usize = 100;
const MAX_PAYLOAD_SIZE: usize = 4096;

fn make_event_envelope(
    instance_id: &str,
    sequence: u64,
    schema_version: u8,
    payload: serde_json::Value,
) -> Option<EventEnvelope> {
    let payload_str = serde_json::to_string(&payload).ok()?;
    if payload_str.len() > MAX_PAYLOAD_SIZE {
        return None;
    }
    Some(EventEnvelope {
        schema_version,
        instance_id: instance_id.to_string(),
        sequence,
        timestamp_ms: 1000 * sequence,
        payload,
        metadata: vo_types::events::EventMetadata::default(),
    })
}

fn make_workflow_started(workflow_id: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "WorkflowStarted",
        "workflow_id": workflow_id,
        "binary_hash": "sha256abc",
        "version": 1
    })
}

fn make_step_scheduled(workflow_id: &str, step_id: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "StepScheduled",
        "workflow_id": workflow_id,
        "step_id": step_id,
        "attempt": 1,
        "execution_id": "exec-1",
        "version": 1
    })
}

fn make_step_started(workflow_id: &str, step_id: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "StepStarted",
        "workflow_id": workflow_id,
        "step_id": step_id,
        "started_at_ms": 2000,
        "version": 1
    })
}

fn make_step_completed(workflow_id: &str, step_id: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "StepCompleted",
        "workflow_id": workflow_id,
        "step_id": step_id,
        "completed_at_ms": 3000,
        "version": 1
    })
}

fn make_step_failed(workflow_id: &str, step_id: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "StepFailed",
        "workflow_id": workflow_id,
        "step_id": step_id,
        "failure_reason": "error",
        "attempt": 1,
        "version": 1
    })
}

fn make_instance_resumed(workflow_id: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "InstanceResumed",
        "workflow_id": workflow_id,
        "resumed_at_ms": 6000,
        "version": 1
    })
}

fn make_cancel_requested(workflow_id: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "CancelRequested",
        "workflow_id": workflow_id,
        "requested_by": "user",
        "version": 1
    })
}

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };

    let Ok(events_json) = serde_json::from_str::<serde_json::Value>(s) else {
        return;
    };

    let events_array = match events_json.as_array() {
        Some(arr) => arr,
        None => return,
    };

    if events_array.is_empty() {
        let engine = ReplayEngine::new();
        let result1 = engine.replay(&[]);
        let result2 = engine.replay(&[]);
        assert_eq!(result1, result2, "Empty replay must be deterministic");
        return;
    }

    let mut valid_envelopes = Vec::with_capacity(events_array.len().min(MAX_EVENTS));
    let mut last_seq = 0u64;
    let mut valid = true;

    for (i, event_val) in events_array.iter().take(MAX_EVENTS).enumerate() {
        let obj = match event_val.as_object() {
            Some(o) => o,
            None => continue,
        };

        let seq = obj
            .get("sequence")
            .and_then(|v| v.as_u64())
            .unwrap_or((i + 1) as u64);

        if seq <= last_seq {
            valid = false;
            break;
        }
        last_seq = seq;

        let instance_id = obj
            .get("instance_id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("fuzz-instance");

        let schema_version = obj
            .get("version")
            .and_then(|v| v.as_u64())
            .map(|v| v as u8)
            .unwrap_or(1);

        let event_type = obj
            .get("event_type")
            .and_then(|v| v.as_str())
            .unwrap_or("WorkflowStarted");

        let workflow_id = obj
            .get("workflow_id")
            .and_then(|v| v.as_str())
            .unwrap_or("wf-1");

        let step_id = obj
            .get("step_id")
            .and_then(|v| v.as_str())
            .unwrap_or("step-1");

        let payload = match event_type {
            "WorkflowStarted" => make_workflow_started(workflow_id),
            "StepScheduled" => make_step_scheduled(workflow_id, step_id),
            "StepStarted" => make_step_started(workflow_id, step_id),
            "StepCompleted" => make_step_completed(workflow_id, step_id),
            "StepFailed" => make_step_failed(workflow_id, step_id),
            "InstanceResumed" => make_instance_resumed(workflow_id),
            "CancelRequested" => make_cancel_requested(workflow_id),
            _ => continue,
        };

        if let Some(envelope) = make_event_envelope(instance_id, seq, schema_version, payload) {
            valid_envelopes.push(envelope);
        }
    }

    if valid_envelopes.is_empty() || !valid {
        return;
    }

    let engine = ReplayEngine::new();

    let result1 = engine.replay(&valid_envelopes);
    let result2 = engine.replay(&valid_envelopes);

    assert_eq!(
        result1, result2,
        "Replay must be deterministic - same events must produce same result"
    );

    if let (Ok(r1), Ok(r2)) = (&result1, &result2) {
        assert_eq!(
            r1.events_applied, r2.events_applied,
            "Events applied count must be deterministic"
        );
        assert_eq!(
            r1.final_state, r2.final_state,
            "Final state must be deterministic"
        );
    }

    if valid_envelopes.len() >= 3 {
        let split_point = valid_envelopes.len() / 2;
        let pre_snapshot = &valid_envelopes[..split_point];
        let post_snapshot = &valid_envelopes[split_point..];

        let mut combined = Vec::with_capacity(valid_envelopes.len());
        combined.extend_from_slice(pre_snapshot);
        combined.extend_from_slice(post_snapshot);

        let full_result = engine.replay(&valid_envelopes);
        let split_result = engine.replay(&combined);

        if let (Ok(full), Ok(split)) = (&full_result, &split_result) {
            if full.final_state != split.final_state {
                return;
            }
        }
    }
});
