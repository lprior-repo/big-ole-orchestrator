//! Red Queen tests: json-attacks dimension.
//!
//! Tests malformed, wrong types, extra fields, nulls, and empty inputs.

use crate::*;

// RQ-09: Extra fields in JSON are silently ignored
#[test]
fn rq_extra_json_fields_ignored() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}, "extra_field": "ignored"}],
        "edges": [],
        "bogus_field": 42,
        "another_one": true
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    result.expect("extra JSON fields should be silently ignored");
}

// RQ-10: Wrong type for workflow_name (number instead of string)
#[test]
fn rq_wrong_type_workflow_name_rejected() {
    let json = serde_json::json!({
        "workflow_name": 123,
        "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}],
        "edges": []
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
}

// RQ-11: Wrong type for node_name (number instead of string)
#[test]
fn rq_wrong_type_node_name_rejected() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [{"node_name": 42, "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}],
        "edges": []
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
}

// RQ-12: Wrong type for max_attempts (string instead of number)
#[test]
fn rq_wrong_type_max_attempts_rejected() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": "three", "backoff_ms": 0, "backoff_multiplier": 1.0}}],
        "edges": []
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
}

// RQ-13: Wrong type for edge condition (number instead of string)
#[test]
fn rq_wrong_type_edge_condition_rejected() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}],
        "edges": [{"source_node": "a", "target_node": "a", "condition": 42}]
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
}

// RQ-14: Null for workflow_name
#[test]
fn rq_null_workflow_name_rejected() {
    let json = serde_json::json!({
        "workflow_name": null,
        "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}],
        "edges": []
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
}

// RQ-15: Empty bytes input
#[test]
fn rq_empty_bytes_rejected() {
    let bytes: &[u8] = b"";
    let result = WorkflowDefinition::parse(bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
}

// RQ-16: Array instead of object
#[test]
fn rq_array_instead_of_object_rejected() {
    let bytes = b"[]";
    let result = WorkflowDefinition::parse(bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
}

// RQ-17: Null for retry_policy
#[test]
fn rq_null_retry_policy_rejected() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [{"node_name": "a", "retry_policy": null}],
        "edges": []
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
}

// RQ-18: String "NaN" for backoff_multiplier (not actual NaN token)
#[test]
fn rq_string_nan_for_multiplier_rejected() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": "NaN"}}],
        "edges": []
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
}

// RQ-19: Boolean for edge condition
#[test]
fn rq_boolean_edge_condition_rejected() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}],
        "edges": [{"source_node": "a", "target_node": "a", "condition": true}]
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
}

// RQ-20: Invalid edge condition string
#[test]
fn rq_invalid_edge_condition_string_rejected() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}],
        "edges": [{"source_node": "a", "target_node": "a", "condition": "Sometimes"}]
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
}
