//! Red Queen tests: parse-determinism and parse-error-priority dimensions.
//!
//! Tests parse determinism with complex workflows and error priority ordering.

use crate::*;

// ===========================================================================
// DIMENSION: parse-determinism
// ===========================================================================

// RQ-51: parse is deterministic with complex workflow
#[test]
fn rq_parse_deterministic_complex_workflow() {
    let json = serde_json::json!({
        "workflow_name": "complex",
        "nodes": [
            {"node_name": "a", "retry_policy": {"max_attempts": 3, "backoff_ms": 1000, "backoff_multiplier": 2.5}},
            {"node_name": "b", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "c", "retry_policy": {"max_attempts": 5, "backoff_ms": 500, "backoff_multiplier": 1.5}},
            {"node_name": "d", "retry_policy": {"max_attempts": 10, "backoff_ms": 2000, "backoff_multiplier": 3.0}}
        ],
        "edges": [
            {"source_node": "a", "target_node": "b", "condition": "OnSuccess"},
            {"source_node": "a", "target_node": "c", "condition": "OnFailure"},
            {"source_node": "b", "target_node": "d", "condition": "Always"},
            {"source_node": "c", "target_node": "d", "condition": "Always"}
        ]
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let r1 = WorkflowDefinition::parse(&bytes).unwrap();
    let r2 = WorkflowDefinition::parse(&bytes).unwrap();
    assert_eq!(r1, r2);
}

// RQ-52: parse error determinism -- same input always produces same error
#[test]
fn rq_parse_error_deterministic() {
    let bytes = b"not valid json{{{";
    let r1 = WorkflowDefinition::parse(bytes);
    let r2 = WorkflowDefinition::parse(bytes);
    // Both should be DeserializationFailed (can't compare inner String easily)
    assert!(matches!(
        r1,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
    assert!(matches!(
        r2,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
    // Compare the display messages for determinism
    assert_eq!(r1.unwrap_err().to_string(), r2.unwrap_err().to_string());
}

// ===========================================================================
// DIMENSION: parse-error-priority
// Verify the documented error priority order
// ===========================================================================

// RQ-53: Both invalid retry policy AND unknown edge -> retry policy wins (priority 3 > 4)
#[test]
fn rq_error_priority_retry_policy_before_unknown_node() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [{"node_name": "bad", "retry_policy": {"max_attempts": 0, "backoff_ms": 0, "backoff_multiplier": 0.5}}],
        "edges": [{"source_node": "bad", "target_node": "ghost", "condition": "Always"}]
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::InvalidRetryPolicy { .. })
    ));
}

// RQ-54: Empty nodes AND invalid retry policy -> empty wins (priority 2 > 3)
#[test]
fn rq_error_priority_empty_before_invalid_retry() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [],
        "edges": []
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert_eq!(result, Err(WorkflowDefinitionError::EmptyWorkflow));
}

// RQ-55: Unknown node AND cycle -> unknown wins (priority 4 > 5)
#[test]
fn rq_error_priority_unknown_before_cycle() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}],
        "edges": [
            {"source_node": "a", "target_node": "ghost", "condition": "Always"},
            {"source_node": "a", "target_node": "a", "condition": "Always"}
        ]
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::UnknownNode { .. })
    ));
}
