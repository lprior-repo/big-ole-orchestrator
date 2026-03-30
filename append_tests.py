import os

path = "crates/vo-types/src/events.rs"

tests = """
    #[rstest]
    #[case(r#"{"version": 1, "instance_id": "wf-123", "sequence": "bad", "timestamp_ms": 1000, "payload": {"type": "WorkflowStarted", "workflow_id": "w1"}, "metadata": {}}"#, Error::InvalidEnvelopeField("sequence must be an integer".to_string()))]
    #[case(r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": "bad", "payload": {"type": "WorkflowStarted", "workflow_id": "w1"}, "metadata": {}}"#, Error::InvalidEnvelopeField("timestamp_ms must be an integer".to_string()))]
    fn envelope_from_str_invalid_types(#[case] json: &str, #[case] expected: Error) {
        let result = EventEnvelope::from_str(json);
        assert_eq!(result, Err(expected));
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

    fn payload_invalid_fields(#[case] json: serde_json::Value, #[case] expected: Error) {
        let result = EventPayload::try_from_json(json);
        assert_eq!(result, Err(expected));
    }
}
"""

with open(path, 'r') as f:
    content = f.read()

# remove trailing }
content = content.rstrip()
if content.endswith("}"):
    content = content[:-1]

content += tests

with open(path, 'w') as f:
    f.write(content)
