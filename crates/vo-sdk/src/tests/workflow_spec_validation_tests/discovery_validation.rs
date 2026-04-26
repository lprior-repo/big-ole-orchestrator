//! Test Coverage: Discovery validation — version compatibility, schema evolution, upgrade paths.
//!
//! bead_id: ve-jm7n

use crate::WorkflowSpec;
use vo_types::NodeName;

#[test]
fn schema_version_zero_is_accepted_for_backward_compat() {
    let payload = serde_json::json!({"version": 0});
    let spec: vo_types::WorkflowSpec =
        serde_json::from_value(payload).expect("version 0 accepted for backward compat");
    assert_eq!(spec.version(), 0);
}

#[test]
fn schema_version_rejects_string_version() {
    let payload = serde_json::json!({"version": "1"});
    let result: Result<vo_types::WorkflowSpec, _> = serde_json::from_value(payload);
    assert!(
        result.is_err(),
        "string version should be rejected as invalid format"
    );
}

#[test]
fn schema_version_rejects_float_version() {
    let payload = serde_json::json!({"version": 1.5});
    let result: Result<vo_types::WorkflowSpec, _> = serde_json::from_value(payload);
    assert!(
        result.is_err(),
        "float version should be rejected as invalid format"
    );
}

#[test]
fn schema_version_rejects_negative_version() {
    let payload = serde_json::json!({"version": -1});
    let result: Result<vo_types::WorkflowSpec, _> = serde_json::from_value(payload);
    assert!(
        result.is_err(),
        "negative version should be rejected as invalid format"
    );
}

#[test]
fn schema_version_rejects_null_version() {
    let payload = serde_json::json!({"version": null});
    let result: Result<vo_types::WorkflowSpec, _> = serde_json::from_value(payload);
    assert!(
        result.is_err(),
        "null version should be rejected as invalid format"
    );
}

#[test]
fn schema_version_rejects_boolean_version() {
    let payload = serde_json::json!({"version": true});
    let result: Result<vo_types::WorkflowSpec, _> = serde_json::from_value(payload);
    assert!(
        result.is_err(),
        "boolean version should be rejected as invalid format"
    );
}

#[test]
fn schema_version_rejects_u16_overflow() {
    let payload = serde_json::json!({"version": 65536});
    let result: Result<vo_types::WorkflowSpec, _> = serde_json::from_value(payload);
    assert!(
        result.is_err(),
        "version exceeding u16 range should be rejected"
    );
}

#[test]
fn schema_version_rejects_large_future_version() {
    let payload = serde_json::json!({"version": 999});
    let result: Result<vo_types::WorkflowSpec, _> = serde_json::from_value(payload);
    assert!(result.is_err(), "large future version should be rejected");
}

#[test]
fn upgrade_path_version_0_to_1_is_valid() {
    let v0_payload = serde_json::json!({"version": 0});
    let v1_payload = serde_json::json!({"version": 1});

    let spec_v0: vo_types::WorkflowSpec =
        serde_json::from_value(v0_payload).expect("version 0 accepted");
    let spec_v1: vo_types::WorkflowSpec =
        serde_json::from_value(v1_payload).expect("version 1 accepted");

    assert!(spec_v1.version() >= spec_v0.version());
}

#[test]
fn schema_evolution_extra_fields_are_ignored() {
    let payload = serde_json::json!({
        "version": 1,
        "future_field": "some value",
        "another_new_field": 42,
        "nested": {"deep": true}
    });
    let spec: vo_types::WorkflowSpec =
        serde_json::from_value(payload).expect("extra fields should be ignored");
    assert_eq!(spec.version(), 1);
}

#[test]
fn discovery_sdk_spec_with_extra_fields_ignored() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [{"name": "a", "kind": "pure"}],
        "edges": [],
        "discovery_metadata": {"version": 2}
    }"#;
    let spec: WorkflowSpec = serde_json::from_str(json).expect("extra discovery fields ignored");
    assert_eq!(spec.nodes.len(), 1);
}
