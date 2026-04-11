//! Shared test helpers for replay engine tests.

use serde_json::json;
use vo_types::events::{EventEnvelope, EventMetadata};

/// Helper: create a valid EventEnvelope for testing.
pub fn make_event(instance_id: &str, sequence: u64, payload: serde_json::Value) -> EventEnvelope {
    EventEnvelope {
        schema_version: 1,
        instance_id: instance_id.to_string(),
        sequence,
        timestamp_ms: 1000 * sequence,
        payload,
        metadata: EventMetadata::default(),
    }
}

/// Helper: create an EventEnvelope at schema version 0 for upcaster tests.
pub fn make_v0_event(
    instance_id: &str,
    sequence: u64,
    payload: serde_json::Value,
) -> EventEnvelope {
    EventEnvelope {
        schema_version: 0,
        instance_id: instance_id.to_string(),
        sequence,
        timestamp_ms: 1000 * sequence,
        payload,
        metadata: EventMetadata::default(),
    }
}

pub fn workflow_started_payload(workflow_id: &str) -> serde_json::Value {
    json!({
        "type": "WorkflowStarted",
        "workflow_id": workflow_id,
        "binary_hash": "sha256abc",
        "version": 1
    })
}

pub fn step_scheduled_payload(workflow_id: &str, step_id: &str) -> serde_json::Value {
    json!({
        "type": "StepScheduled",
        "workflow_id": workflow_id,
        "step_id": step_id,
        "attempt": 1,
        "execution_id": "exec-1",
        "version": 1
    })
}

pub fn step_started_payload(workflow_id: &str, step_id: &str) -> serde_json::Value {
    json!({
        "type": "StepStarted",
        "workflow_id": workflow_id,
        "step_id": step_id,
        "started_at_ms": 2000,
        "version": 1
    })
}

pub fn step_completed_payload(workflow_id: &str, step_id: &str) -> serde_json::Value {
    json!({
        "type": "StepCompleted",
        "workflow_id": workflow_id,
        "step_id": step_id,
        "completed_at_ms": 3000,
        "version": 1
    })
}

pub fn step_failed_payload(workflow_id: &str, step_id: &str) -> serde_json::Value {
    json!({
        "type": "StepFailed",
        "workflow_id": workflow_id,
        "step_id": step_id,
        "failure_reason": "error",
        "attempt": 1,
        "version": 1
    })
}

pub fn timer_set_payload(workflow_id: &str, timer_id: &str) -> serde_json::Value {
    json!({
        "type": "TimerSet",
        "workflow_id": workflow_id,
        "timer_id": timer_id,
        "fire_at_ms": 5000,
        "version": 1
    })
}

pub fn timer_fired_payload(workflow_id: &str, timer_id: &str) -> serde_json::Value {
    json!({
        "type": "TimerFired",
        "workflow_id": workflow_id,
        "timer_id": timer_id,
        "fired_at_ms": 5000,
        "version": 1
    })
}

pub fn workflow_cancelled_payload(workflow_id: &str) -> serde_json::Value {
    json!({
        "type": "WorkflowCancelled",
        "workflow_id": workflow_id,
        "cancelled_by": "user",
        "version": 1
    })
}

pub fn cancel_requested_payload(workflow_id: &str) -> serde_json::Value {
    json!({
        "type": "CancelRequested",
        "workflow_id": workflow_id,
        "requested_by": "user",
        "version": 1
    })
}

pub fn workflow_failed_payload(workflow_id: &str) -> serde_json::Value {
    json!({
        "type": "WorkflowFailed",
        "workflow_id": workflow_id,
        "failure_reason": "fatal",
        "version": 1
    })
}

pub fn instance_resumed_payload(workflow_id: &str) -> serde_json::Value {
    json!({
        "type": "InstanceResumed",
        "workflow_id": workflow_id,
        "resumed_at_ms": 6000,
        "version": 1
    })
}

pub fn continued_as_new_payload(workflow_id: &str) -> serde_json::Value {
    json!({
        "type": "ContinuedAsNew",
        "workflow_id": workflow_id,
        "lineage_id": "lin-1",
        "old_epoch": 0,
        "new_epoch": 1,
        "version": 1
    })
}
