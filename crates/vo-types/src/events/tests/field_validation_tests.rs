
use crate::events::error::Error;
use crate::events::payload::EventPayload;
use rstest::rstest;

// -------------------------------------------------------------------------
// ADR-027: Error-path tests for new required fields (binary_hash, attempt,
// execution_id, dag_topology, output)
// -------------------------------------------------------------------------

#[test]
fn payload_try_from_json_returns_missing_payload_field_when_binary_hash_is_absent() {
    let json = serde_json::json!({"type": "WorkflowStarted", "workflow_id": "w1", "version": 1});
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Err(Error::MissingPayloadField("binary_hash".to_string()))
    );
}

#[test]
fn payload_try_from_json_returns_missing_payload_field_when_attempt_is_absent_for_step_scheduled() {
    let json = serde_json::json!({"type": "StepScheduled", "workflow_id": "w1", "step_id": "s1", "execution_id": "e1", "version": 1});
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Err(Error::MissingPayloadField("attempt".to_string()))
    );
}

#[test]
fn payload_try_from_json_returns_invalid_payload_field_when_attempt_is_not_integer_for_step_scheduled(
) {
    let json = serde_json::json!({"type": "StepScheduled", "workflow_id": "w1", "step_id": "s1", "attempt": "bad", "execution_id": "e1", "version": 1});
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Err(Error::InvalidPayloadField(
            "attempt must be an integer".to_string()
        ))
    );
}

#[test]
fn payload_try_from_json_returns_missing_payload_field_when_execution_id_is_absent() {
    let json = serde_json::json!({"type": "StepScheduled", "workflow_id": "w1", "step_id": "s1", "attempt": 1, "version": 1});
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Err(Error::MissingPayloadField("execution_id".to_string()))
    );
}

#[test]
fn payload_try_from_json_returns_missing_payload_field_when_attempt_is_absent_for_step_failed() {
    let json = serde_json::json!({"type": "StepFailed", "workflow_id": "w1", "step_id": "s1", "failure_reason": "err", "version": 1});
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Err(Error::MissingPayloadField("attempt".to_string()))
    );
}

#[test]
fn payload_try_from_json_defaults_dag_topology_to_null_when_absent() {
    let json = serde_json::json!({"type": "WorkflowStarted", "workflow_id": "w1", "binary_hash": "abc123", "version": 1});
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Ok(EventPayload::WorkflowStarted {
            workflow_id: "w1".into(),
            dag_topology: serde_json::Value::Null,
            binary_hash: "abc123".into(),
        })
    );
}

#[test]
fn payload_try_from_json_defaults_output_to_null_when_absent() {
    let json = serde_json::json!({"type": "StepCompleted", "workflow_id": "w1", "step_id": "s1", "completed_at_ms": 1000, "version": 1});
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Ok(EventPayload::StepCompleted {
            workflow_id: "w1".into(),
            step_id: "s1".into(),
            completed_at_ms: 1000,
            output: serde_json::Value::Null,
        })
    );
}

#[test]
fn payload_try_from_json_handles_attempt_at_u32_max() {
    let json = serde_json::json!({"type": "StepScheduled", "workflow_id": "w1", "step_id": "s1", "attempt": 4294967295_u64, "execution_id": "e1", "version": 1});
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Ok(EventPayload::StepScheduled {
            workflow_id: "w1".into(),
            step_id: "s1".into(),
            attempt: u32::MAX,
            execution_id: "e1".into(),
        })
    );
}

#[test]
fn payload_try_from_json_returns_invalid_payload_field_when_attempt_is_not_integer_for_step_failed()
{
    let json = serde_json::json!({"type": "StepFailed", "workflow_id": "w1", "step_id": "s1", "failure_reason": "err", "attempt": "bad", "version": 1});
    let result = EventPayload::try_from_json(&json);
    assert_eq!(
        result,
        Err(Error::InvalidPayloadField(
            "attempt must be an integer".to_string()
        ))
    );
}

#[rstest]
#[case(serde_json::json!({"type": "WorkflowStarted", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
#[case(serde_json::json!({"type": "WorkflowStarted", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
#[case(serde_json::json!({"type": "WorkflowCompleted", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
#[case(serde_json::json!({"type": "WorkflowCompleted", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
#[case(serde_json::json!({"type": "WorkflowCompleted", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("completion_time_ms".to_string()))]
#[case(serde_json::json!({"type": "WorkflowCompleted", "workflow_id": "w1", "completion_time_ms": "bad", "version": 1}), Error::InvalidPayloadField("completion_time_ms must be an integer".to_string()))]
#[case(serde_json::json!({"type": "WorkflowFailed", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
#[case(serde_json::json!({"type": "WorkflowFailed", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
#[case(serde_json::json!({"type": "WorkflowFailed", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("failure_reason".to_string()))]
#[case(serde_json::json!({"type": "WorkflowFailed", "workflow_id": "w1", "failure_reason": 123, "version": 1}), Error::InvalidPayloadField("failure_reason must be a string".to_string()))]
#[case(serde_json::json!({"type": "WorkflowCancelled", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
#[case(serde_json::json!({"type": "WorkflowCancelled", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
#[case(serde_json::json!({"type": "WorkflowCancelled", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("cancelled_by".to_string()))]
#[case(serde_json::json!({"type": "WorkflowCancelled", "workflow_id": "w1", "cancelled_by": 123, "version": 1}), Error::InvalidPayloadField("cancelled_by must be a string".to_string()))]
#[case(serde_json::json!({"type": "StepScheduled", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
#[case(serde_json::json!({"type": "StepScheduled", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
#[case(serde_json::json!({"type": "StepScheduled", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("step_id".to_string()))]
#[case(serde_json::json!({"type": "StepScheduled", "workflow_id": "w1", "step_id": 123, "version": 1}), Error::InvalidPayloadField("step_id must be a string".to_string()))]
#[case(serde_json::json!({"type": "StepStarted", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
#[case(serde_json::json!({"type": "StepStarted", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
#[case(serde_json::json!({"type": "StepStarted", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("step_id".to_string()))]
#[case(serde_json::json!({"type": "StepStarted", "workflow_id": "w1", "step_id": 123, "version": 1}), Error::InvalidPayloadField("step_id must be a string".to_string()))]
#[case(serde_json::json!({"type": "StepStarted", "workflow_id": "w1", "step_id": "s1", "version": 1}), Error::MissingPayloadField("started_at_ms".to_string()))]
#[case(serde_json::json!({"type": "StepStarted", "workflow_id": "w1", "step_id": "s1", "started_at_ms": "bad", "version": 1}), Error::InvalidPayloadField("started_at_ms must be an integer".to_string()))]
#[case(serde_json::json!({"type": "StepCompleted", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
#[case(serde_json::json!({"type": "StepCompleted", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
#[case(serde_json::json!({"type": "StepCompleted", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("step_id".to_string()))]
#[case(serde_json::json!({"type": "StepCompleted", "workflow_id": "w1", "step_id": 123, "version": 1}), Error::InvalidPayloadField("step_id must be a string".to_string()))]
#[case(serde_json::json!({"type": "StepCompleted", "workflow_id": "w1", "step_id": "s1", "version": 1}), Error::MissingPayloadField("completed_at_ms".to_string()))]
#[case(serde_json::json!({"type": "StepCompleted", "workflow_id": "w1", "step_id": "s1", "completed_at_ms": "bad", "version": 1}), Error::InvalidPayloadField("completed_at_ms must be an integer".to_string()))]
#[case(serde_json::json!({"type": "StepFailed", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
#[case(serde_json::json!({"type": "StepFailed", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
#[case(serde_json::json!({"type": "StepFailed", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("step_id".to_string()))]
#[case(serde_json::json!({"type": "StepFailed", "workflow_id": "w1", "step_id": 123, "version": 1}), Error::InvalidPayloadField("step_id must be a string".to_string()))]
#[case(serde_json::json!({"type": "StepFailed", "workflow_id": "w1", "step_id": "s1", "version": 1}), Error::MissingPayloadField("failure_reason".to_string()))]
#[case(serde_json::json!({"type": "StepFailed", "workflow_id": "w1", "step_id": "s1", "failure_reason": 123, "version": 1}), Error::InvalidPayloadField("failure_reason must be a string".to_string()))]
#[case(serde_json::json!({"type": "TimerSet", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
#[case(serde_json::json!({"type": "TimerSet", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
#[case(serde_json::json!({"type": "TimerSet", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("timer_id".to_string()))]
#[case(serde_json::json!({"type": "TimerSet", "workflow_id": "w1", "timer_id": 123, "version": 1}), Error::InvalidPayloadField("timer_id must be a string".to_string()))]
#[case(serde_json::json!({"type": "TimerSet", "workflow_id": "w1", "timer_id": "t1", "version": 1}), Error::MissingPayloadField("fire_at_ms".to_string()))]
#[case(serde_json::json!({"type": "TimerSet", "workflow_id": "w1", "timer_id": "t1", "fire_at_ms": "bad", "version": 1}), Error::InvalidPayloadField("fire_at_ms must be an integer".to_string()))]
#[case(serde_json::json!({"type": "TimerFired", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
#[case(serde_json::json!({"type": "TimerFired", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
#[case(serde_json::json!({"type": "TimerFired", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("timer_id".to_string()))]
#[case(serde_json::json!({"type": "TimerFired", "workflow_id": "w1", "timer_id": 123, "version": 1}), Error::InvalidPayloadField("timer_id must be a string".to_string()))]
#[case(serde_json::json!({"type": "TimerFired", "workflow_id": "w1", "timer_id": "t1", "version": 1}), Error::MissingPayloadField("fired_at_ms".to_string()))]
#[case(serde_json::json!({"type": "TimerFired", "workflow_id": "w1", "timer_id": "t1", "fired_at_ms": "bad", "version": 1}), Error::InvalidPayloadField("fired_at_ms must be an integer".to_string()))]
#[case(serde_json::json!({"type": "CancelRequested", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
#[case(serde_json::json!({"type": "CancelRequested", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
#[case(serde_json::json!({"type": "CancelRequested", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("requested_by".to_string()))]
#[case(serde_json::json!({"type": "CancelRequested", "workflow_id": "w1", "requested_by": 123, "version": 1}), Error::InvalidPayloadField("requested_by must be a string".to_string()))]
#[case(serde_json::json!({"type": "InstanceResumed", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
#[case(serde_json::json!({"type": "InstanceResumed", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
#[case(serde_json::json!({"type": "InstanceResumed", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("resumed_at_ms".to_string()))]
#[case(serde_json::json!({"type": "InstanceResumed", "workflow_id": "w1", "resumed_at_ms": "bad", "version": 1}), Error::InvalidPayloadField("resumed_at_ms must be an integer".to_string()))]
// ADR-027: new required-field missing cases for binary_hash, attempt, execution_id
#[case(serde_json::json!({"type": "WorkflowStarted", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("binary_hash".to_string()))]
#[case(serde_json::json!({"type": "StepScheduled", "workflow_id": "w1", "step_id": "s1", "execution_id": "e1", "version": 1}), Error::MissingPayloadField("attempt".to_string()))]
#[case(serde_json::json!({"type": "StepScheduled", "workflow_id": "w1", "step_id": "s1", "attempt": 1, "version": 1}), Error::MissingPayloadField("execution_id".to_string()))]
#[case(serde_json::json!({"type": "StepFailed", "workflow_id": "w1", "step_id": "s1", "failure_reason": "err", "version": 1}), Error::MissingPayloadField("attempt".to_string()))]

fn payload_invalid_fields(#[case] json: serde_json::Value, #[case] expected: Error) {
    let result = EventPayload::try_from_json(&json);
    assert_eq!(result, Err(expected));
}
