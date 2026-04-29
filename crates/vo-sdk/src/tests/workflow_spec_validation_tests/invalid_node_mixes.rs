//! Section 2: Invalid node mixes rejected

use crate::dag::{Dag, DagError};
use crate::node_handle::NodeHandle;
use vo_types::{NodeKind, NodeName};

#[test]
fn dag_rejects_node_with_digit_prefix() {
    let mut dag = Dag::new();
    let result: Result<NodeHandle<(), ()>, _> =
        dag.add_node_with_kind("1valid-per-grammar", NodeKind::Pure, |_: ()| ());
    assert!(
        result.is_err(),
        "node name starting with digit should be rejected"
    );
}

#[test]
fn dag_rejects_node_with_spaces_in_name() {
    let mut dag = Dag::new();
    let result: Result<NodeHandle<(), ()>, _> =
        dag.add_node_with_kind("bad name", NodeKind::Pure, |_: ()| ());
    assert!(
        matches!(result, Err(DagError::InvalidNodeName { .. })),
        "node name with spaces should be rejected"
    );
}

#[test]
fn dag_accepts_node_with_uppercase_in_name_as_valid() {
    let mut dag = Dag::new();
    let result: Result<NodeHandle<(), ()>, _> =
        dag.add_node_with_kind("BadName", NodeKind::Pure, |_: ()| ());
    assert!(
        result.is_ok(),
        "uppercase in node name is accepted by current grammar"
    );
}

#[test]
fn dag_rejects_connect_to_nonexistent_node() {
    let mut dag = Dag::new();
    let a: NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let ghost: NodeHandle<(), ()> = NodeHandle::new(NodeName::parse("ghost").expect("valid name"));
    let result = dag.connect(&a, &ghost);
    assert!(
        matches!(result, Err(DagError::NodeNotFound { .. })),
        "connecting to nonexistent node should fail"
    );
}

#[test]
fn dag_rejects_connect_from_nonexistent_node() {
    let mut dag = Dag::new();
    let b: NodeHandle<(), ()> = dag
        .add_node_with_kind("b", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let ghost: NodeHandle<(), ()> = NodeHandle::new(NodeName::parse("ghost").expect("valid name"));
    let result = dag.connect(&ghost, &b);
    assert!(
        matches!(result, Err(DagError::NodeNotFound { .. })),
        "connecting from nonexistent node should fail"
    );
}

#[test]
fn dag_rejects_build_with_no_nodes() {
    let dag = Dag::new();
    let result = dag.build("empty");
    assert!(
        matches!(result, Err(DagError::EmptyWorkflow)),
        "empty dag should be rejected"
    );
}

#[test]
fn dag_accepts_workflow_name_with_caps_as_valid() {
    let mut dag = Dag::new();
    let _: NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let result = dag.build("MyWorkflow");
    assert!(
        result.is_ok(),
        "uppercase workflow name is accepted by current grammar"
    );
}

#[test]
fn dag_rejects_build_with_empty_workflow_name() {
    let mut dag = Dag::new();
    let _: NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let result = dag.build("");
    assert!(
        matches!(result, Err(DagError::InvalidNodeName { .. })),
        "empty workflow name should be rejected"
    );
}

#[test]
fn serde_rejects_spec_with_null_node_in_nodes_array() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [null],
        "edges": []
    }"#;
    let result: Result<crate::WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err(), "null node in array should be rejected");
}

#[test]
fn serde_rejects_spec_with_number_as_node() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [42],
        "edges": []
    }"#;
    let result: Result<crate::WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err(), "number as node should be rejected");
}
