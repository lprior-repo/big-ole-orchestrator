use super::*;
use crate::events::error::Error;
use crate::events::payload::EventPayload;

#[test]
fn payload_try_from_json_returns_workflow_started_when_type_is_workflow_started() {
    let json = serde_json::json!({"type": "WorkflowStarted", "workflow_id": "wf-123", "dag_topology": {}, "binary_hash": "abc123", "version": 1});
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Ok(EventPayload::WorkflowStarted {
            workflow_id: "wf-123".to_string(),
            dag_topology: serde_json::json!({}),
            binary_hash: "abc123".to_string()
        })
    );
}

#[test]
fn payload_try_from_json_returns_workflow_completed_when_type_is_workflow_completed() {
    let json = serde_json::json!({"type": "WorkflowCompleted", "workflow_id": "wf-123", "completion_time_ms": 1000, "version": 1});
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Ok(EventPayload::WorkflowCompleted {
            workflow_id: "wf-123".to_string(),
            completion_time_ms: 1000
        })
    );
}

#[test]
fn payload_try_from_json_returns_workflow_failed_when_type_is_workflow_failed() {
    let json = serde_json::json!({"type": "WorkflowFailed", "workflow_id": "wf-123", "failure_reason": "timeout", "version": 1});
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Ok(EventPayload::WorkflowFailed {
            workflow_id: "wf-123".to_string(),
            failure_reason: "timeout".to_string()
        })
    );
}

#[test]
fn payload_try_from_json_returns_workflow_cancelled_when_type_is_workflow_cancelled() {
    let json = serde_json::json!({"type": "WorkflowCancelled", "workflow_id": "wf-123", "cancelled_by": "user", "version": 1});
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Ok(EventPayload::WorkflowCancelled {
            workflow_id: "wf-123".to_string(),
            cancelled_by: "user".to_string()
        })
    );
}

#[test]
fn payload_try_from_json_returns_step_scheduled_when_type_is_step_scheduled() {
    let json = serde_json::json!({"type": "StepScheduled", "workflow_id": "wf-123", "step_id": "step-1", "attempt": 1, "execution_id": "inst::step::1", "version": 1});
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Ok(EventPayload::StepScheduled {
            workflow_id: "wf-123".to_string(),
            step_id: "step-1".to_string(),
            attempt: 1,
            execution_id: "inst::step::1".to_string()
        })
    );
}

#[test]
fn payload_try_from_json_returns_step_started_when_type_is_step_started() {
    let json = serde_json::json!({"type": "StepStarted", "workflow_id": "wf-123", "step_id": "step-1", "started_at_ms": 1000, "version": 1});
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Ok(EventPayload::StepStarted {
            workflow_id: "wf-123".to_string(),
            step_id: "step-1".to_string(),
            started_at_ms: 1000
        })
    );
}

#[test]
fn payload_try_from_json_returns_step_completed_when_type_is_step_completed() {
    let json = serde_json::json!({"type": "StepCompleted", "workflow_id": "wf-123", "step_id": "step-1", "completed_at_ms": 1000, "output": null, "version": 1});
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Ok(EventPayload::StepCompleted {
            workflow_id: "wf-123".to_string(),
            step_id: "step-1".to_string(),
            completed_at_ms: 1000,
            output: serde_json::Value::Null
        })
    );
}

#[test]
fn payload_try_from_json_returns_step_failed_when_type_is_step_failed() {
    let json = serde_json::json!({"type": "StepFailed", "workflow_id": "wf-123", "step_id": "step-1", "failure_reason": "error", "attempt": 1, "version": 1});
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Ok(EventPayload::StepFailed {
            workflow_id: "wf-123".to_string(),
            step_id: "step-1".to_string(),
            failure_reason: "error".to_string(),
            attempt: 1
        })
    );
}

#[test]
fn payload_try_from_json_returns_timer_set_when_type_is_timer_set() {
    let json = serde_json::json!({"type": "TimerSet", "workflow_id": "wf-123", "timer_id": "timer-1", "fire_at_ms": 1000, "version": 1});
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Ok(EventPayload::TimerSet {
            workflow_id: "wf-123".to_string(),
            timer_id: "timer-1".to_string(),
            fire_at_ms: 1000
        })
    );
}

#[test]
fn payload_try_from_json_returns_timer_fired_when_type_is_timer_fired() {
    let json = serde_json::json!({"type": "TimerFired", "workflow_id": "wf-123", "timer_id": "timer-1", "fired_at_ms": 1000, "version": 1});
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Ok(EventPayload::TimerFired {
            workflow_id: "wf-123".to_string(),
            timer_id: "timer-1".to_string(),
            fired_at_ms: 1000
        })
    );
}

#[test]
fn payload_try_from_json_returns_cancel_requested_when_type_is_cancel_requested() {
    let json = serde_json::json!({"type": "CancelRequested", "workflow_id": "wf-123", "requested_by": "user", "version": 1});
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Ok(EventPayload::CancelRequested {
            workflow_id: "wf-123".to_string(),
            requested_by: "user".to_string()
        })
    );
}

#[test]
fn payload_try_from_json_returns_instance_resumed_when_type_is_instance_resumed() {
    let json = serde_json::json!({"type": "InstanceResumed", "workflow_id": "wf-123", "resumed_at_ms": 1000, "version": 1});
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Ok(EventPayload::InstanceResumed {
            workflow_id: "wf-123".to_string(),
            resumed_at_ms: 1000
        })
    );
}
