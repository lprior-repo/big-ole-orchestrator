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

#[test]
fn payload_try_from_json_returns_continued_as_new_when_type_is_continued_as_new() {
    let json = serde_json::json!({"type": "ContinuedAsNew", "workflow_id": "wf-123", "lineage_id": "lin-456", "old_epoch": 1, "new_epoch": 2, "version": 1});
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Ok(EventPayload::ContinuedAsNew {
            workflow_id: "wf-123".to_string(),
            lineage_id: "lin-456".to_string(),
            old_epoch: 1,
            new_epoch: 2
        })
    );
}

#[test]
fn payload_all_variants_round_trip_via_serde() {
    let variants = [
        EventPayload::WorkflowStarted {
            workflow_id: "wf-123".to_string(),
            dag_topology: serde_json::json!({"nodes": []}),
            binary_hash: "abc123".to_string(),
        },
        EventPayload::WorkflowCompleted {
            workflow_id: "wf-123".to_string(),
            completion_time_ms: 1000,
        },
        EventPayload::WorkflowFailed {
            workflow_id: "wf-123".to_string(),
            failure_reason: "timeout".to_string(),
        },
        EventPayload::WorkflowCancelled {
            workflow_id: "wf-123".to_string(),
            cancelled_by: "user".to_string(),
        },
        EventPayload::StepScheduled {
            workflow_id: "wf-123".to_string(),
            step_id: "step-1".to_string(),
            attempt: 1,
            execution_id: "exec-1".to_string(),
        },
        EventPayload::StepStarted {
            workflow_id: "wf-123".to_string(),
            step_id: "step-1".to_string(),
            started_at_ms: 1000,
        },
        EventPayload::StepCompleted {
            workflow_id: "wf-123".to_string(),
            step_id: "step-1".to_string(),
            completed_at_ms: 2000,
            output: serde_json::json!({"result": "ok"}),
        },
        EventPayload::StepFailed {
            workflow_id: "wf-123".to_string(),
            step_id: "step-1".to_string(),
            failure_reason: "error".to_string(),
            attempt: 1,
        },
        EventPayload::TimerSet {
            workflow_id: "wf-123".to_string(),
            timer_id: "timer-1".to_string(),
            fire_at_ms: 1000,
        },
        EventPayload::TimerFired {
            workflow_id: "wf-123".to_string(),
            timer_id: "timer-1".to_string(),
            fired_at_ms: 1000,
        },
        EventPayload::CancelRequested {
            workflow_id: "wf-123".to_string(),
            requested_by: "user".to_string(),
        },
        EventPayload::InstanceResumed {
            workflow_id: "wf-123".to_string(),
            resumed_at_ms: 1000,
        },
        EventPayload::ContinuedAsNew {
            workflow_id: "wf-123".to_string(),
            lineage_id: "lin-456".to_string(),
            old_epoch: 1,
            new_epoch: 2,
        },
    ];

    for payload in variants {
        let json = match &payload {
            EventPayload::WorkflowStarted {
                workflow_id,
                dag_topology,
                binary_hash,
            } => {
                serde_json::json!({
                    "type": "WorkflowStarted",
                    "workflow_id": workflow_id,
                    "dag_topology": dag_topology,
                    "binary_hash": binary_hash,
                    "version": 1
                })
            }
            EventPayload::WorkflowCompleted {
                workflow_id,
                completion_time_ms,
            } => {
                serde_json::json!({
                    "type": "WorkflowCompleted",
                    "workflow_id": workflow_id,
                    "completion_time_ms": completion_time_ms,
                    "version": 1
                })
            }
            EventPayload::WorkflowFailed {
                workflow_id,
                failure_reason,
            } => {
                serde_json::json!({
                    "type": "WorkflowFailed",
                    "workflow_id": workflow_id,
                    "failure_reason": failure_reason,
                    "version": 1
                })
            }
            EventPayload::WorkflowCancelled {
                workflow_id,
                cancelled_by,
            } => {
                serde_json::json!({
                    "type": "WorkflowCancelled",
                    "workflow_id": workflow_id,
                    "cancelled_by": cancelled_by,
                    "version": 1
                })
            }
            EventPayload::StepScheduled {
                workflow_id,
                step_id,
                attempt,
                execution_id,
            } => {
                serde_json::json!({
                    "type": "StepScheduled",
                    "workflow_id": workflow_id,
                    "step_id": step_id,
                    "attempt": attempt,
                    "execution_id": execution_id,
                    "version": 1
                })
            }
            EventPayload::StepStarted {
                workflow_id,
                step_id,
                started_at_ms,
            } => {
                serde_json::json!({
                    "type": "StepStarted",
                    "workflow_id": workflow_id,
                    "step_id": step_id,
                    "started_at_ms": started_at_ms,
                    "version": 1
                })
            }
            EventPayload::StepCompleted {
                workflow_id,
                step_id,
                completed_at_ms,
                output,
            } => {
                serde_json::json!({
                    "type": "StepCompleted",
                    "workflow_id": workflow_id,
                    "step_id": step_id,
                    "completed_at_ms": completed_at_ms,
                    "output": output,
                    "version": 1
                })
            }
            EventPayload::StepFailed {
                workflow_id,
                step_id,
                failure_reason,
                attempt,
            } => {
                serde_json::json!({
                    "type": "StepFailed",
                    "workflow_id": workflow_id,
                    "step_id": step_id,
                    "failure_reason": failure_reason,
                    "attempt": attempt,
                    "version": 1
                })
            }
            EventPayload::TimerSet {
                workflow_id,
                timer_id,
                fire_at_ms,
            } => {
                serde_json::json!({
                    "type": "TimerSet",
                    "workflow_id": workflow_id,
                    "timer_id": timer_id,
                    "fire_at_ms": fire_at_ms,
                    "version": 1
                })
            }
            EventPayload::TimerFired {
                workflow_id,
                timer_id,
                fired_at_ms,
            } => {
                serde_json::json!({
                    "type": "TimerFired",
                    "workflow_id": workflow_id,
                    "timer_id": timer_id,
                    "fired_at_ms": fired_at_ms,
                    "version": 1
                })
            }
            EventPayload::CancelRequested {
                workflow_id,
                requested_by,
            } => {
                serde_json::json!({
                    "type": "CancelRequested",
                    "workflow_id": workflow_id,
                    "requested_by": requested_by,
                    "version": 1
                })
            }
            EventPayload::InstanceResumed {
                workflow_id,
                resumed_at_ms,
            } => {
                serde_json::json!({
                    "type": "InstanceResumed",
                    "workflow_id": workflow_id,
                    "resumed_at_ms": resumed_at_ms,
                    "version": 1
                })
            }
            EventPayload::ContinuedAsNew {
                workflow_id,
                lineage_id,
                old_epoch,
                new_epoch,
            } => {
                serde_json::json!({
                    "type": "ContinuedAsNew",
                    "workflow_id": workflow_id,
                    "lineage_id": lineage_id,
                    "old_epoch": old_epoch,
                    "new_epoch": new_epoch,
                    "version": 1
                })
            }
        };
        let round_tripped =
            EventPayload::try_from_json(&json).expect("deserialize should not fail");
        assert_eq!(
            round_tripped, payload,
            "round-trip failed for {:?}",
            payload
        );
    }
}
