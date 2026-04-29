//! Shared test helpers for replay engine tests.

use serde_json::json;
pub use vo_types::events::{EventEnvelope, EventMetadata};

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

/// Helper: create an EventEnvelope at schema version 2 for v1→v2 migration tests.
pub fn make_v2_event(
    instance_id: &str,
    sequence: u64,
    payload: serde_json::Value,
) -> EventEnvelope {
    EventEnvelope {
        schema_version: 2,
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
        "workflow_version_hash": "wvhash123",
        "dedupe_key_hash": null,
        "version": 1
    })
}

/// Helper: create a WorkflowStarted payload at v2 (for v1→v2 migration tests).
pub fn workflow_started_v2_payload(workflow_id: &str) -> serde_json::Value {
    json!({
        "type": "WorkflowStarted",
        "workflow_id": workflow_id,
        "dag_topology": null,
        "binary_hash": "sha256abc",
        "workflow_version_hash": "wvhash123",
        "dedupe_key_hash": null,
        "version": 2
    })
}

pub fn step_scheduled_payload(workflow_id: &str, step_id: &str) -> serde_json::Value {
    step_scheduled_payload_with_fence(workflow_id, step_id, 1)
}

pub fn step_scheduled_payload_with_fence(
    workflow_id: &str,
    step_id: &str,
    fence: u64,
) -> serde_json::Value {
    json!({
        "type": "StepScheduled",
        "workflow_id": workflow_id,
        "step_id": step_id,
        "attempt": 1,
        "fence": fence,
        "execution_id": format!("exec-{}", fence),
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
        "attempt": 1,
        "fence": 1,
        "routing_projection": null,
        "output_ref": null,
        "output_hash": null,
        "output": null,
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
        "fence": 1,
        "version": 1
    })
}

pub fn effect_prepared_payload(
    workflow_id: &str,
    step_id: &str,
    effect_id: &str,
) -> serde_json::Value {
    json!({
        "type": "EffectPrepared",
        "workflow_id": workflow_id,
        "step_id": step_id,
        "effect_id": effect_id,
        "sink_kind": "test-sink",
        "payload_hash": "payhash123",
        "fence": 1,
        "version": 1
    })
}

pub fn effect_committed_payload(
    workflow_id: &str,
    step_id: &str,
    effect_id: &str,
) -> serde_json::Value {
    json!({
        "type": "EffectCommitted",
        "workflow_id": workflow_id,
        "step_id": step_id,
        "effect_id": effect_id,
        "external_receipt": {
            "connector_id": "test-connector",
            "connector_version": "1.0.0",
            "sink_kind": "HttpCall",
            "receipt_payload": {}
        },
        "fence": 1,
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
