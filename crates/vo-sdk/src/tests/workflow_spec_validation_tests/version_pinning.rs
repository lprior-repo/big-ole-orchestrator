//! Test Coverage: Version pinning enforcement for WorkflowSpec.
//!
//! bead_id: ve-jm7n
//!
//! Tests schema version validation at both vo-types and SDK layers.

use crate::graph::default_retry_policy;
use crate::WorkflowSpec;
use vo_types::{NodeKind, NodeName};

#[test]
fn vo_types_workflow_spec_accepts_current_version() {
    let payload = serde_json::json!({"version": 1});
    let spec: vo_types::WorkflowSpec = serde_json::from_value(payload).expect("version 1 accepted");
    assert_eq!(spec.version(), 1);
}

#[test]
fn vo_types_workflow_spec_rejects_future_version() {
    let payload = serde_json::json!({"version": 2});
    let result: Result<vo_types::WorkflowSpec, _> = serde_json::from_value(payload);
    assert!(result.is_err(), "future version should be rejected");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("Unsupported") || err.contains("unsupported"),
        "error should mention unsupported: {err}"
    );
}

#[test]
fn vo_types_workflow_spec_rejects_zero_version_when_missing_fallback() {
    let payload = serde_json::json!({});
    let result: Result<vo_types::WorkflowSpec, _> = serde_json::from_value(payload);
    assert!(
        result.is_err(),
        "missing version without fallback should be rejected"
    );
}

#[test]
fn vo_types_workflow_spec_default_uses_max_supported() {
    let spec = vo_types::WorkflowSpec::default();
    assert_eq!(
        spec.version(),
        vo_types::MAX_SUPPORTED_SCHEMA_VERSION,
        "default should use MAX_SUPPORTED_SCHEMA_VERSION"
    );
}

#[test]
fn vo_types_workflow_spec_round_trips_version() {
    let spec = vo_types::WorkflowSpec { version: 1 };
    let json = serde_json::to_string(&spec).expect("serialize");
    let restored: vo_types::WorkflowSpec = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.version(), 1);
}

#[test]
fn sdk_workflow_spec_has_no_version_field() {
    let spec = WorkflowSpec {
        workflow_name: vo_types::WorkflowName::parse("test").expect("valid"),
        nodes: vec![],
        edges: vec![],
        dedupe_scope: Default::default(),
        guarantee_class: Default::default(),
    };
    let json = serde_json::to_string(&spec).expect("serialize");
    assert!(
        !json.contains("version"),
        "SDK WorkflowSpec should not have version field: {json}"
    );
}

#[test]
fn sdk_workflow_spec_schema_is_stable_across_round_trips() {
    let spec = WorkflowSpec {
        workflow_name: vo_types::WorkflowName::parse("stability-test").expect("valid"),
        nodes: vec![
            crate::NodeSpec {
                name: vo_types::NodeName::parse("a").expect("valid"),
                kind: NodeKind::Pure,
                retry_policy: default_retry_policy(),
            },
            crate::NodeSpec {
                name: vo_types::NodeName::parse("b").expect("valid"),
                kind: NodeKind::ManagedEffect,
                retry_policy: default_retry_policy(),
            },
        ],
        edges: vec![crate::EdgeSpec {
            from: vo_types::NodeName::parse("a").expect("valid"),
            to: vo_types::NodeName::parse("b").expect("valid"),
        }],
        dedupe_scope: Default::default(),
        guarantee_class: Default::default(),
    };

    let json1 = serde_json::to_string(&spec).expect("serialize");
    let json2 = serde_json::to_string(&spec).expect("serialize");
    assert_eq!(json1, json2, "serialization should be deterministic");

    let restored: WorkflowSpec = serde_json::from_str(&json1).expect("deserialize");
    let json3 = serde_json::to_string(&restored).expect("re-serialize");
    assert_eq!(json1, json3, "round-trip should be bit-identical");
}
