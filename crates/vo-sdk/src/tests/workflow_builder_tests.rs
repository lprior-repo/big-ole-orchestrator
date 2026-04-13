//! Tests for the Workflow builder and WorkflowSpec emission (ADR-004, ADR-009, ADR-031).

#![allow(clippy::unwrap_used)]

use vo_types::NodeKind;

use crate::dag::{Dag, DagError, Workflow};
use crate::graph_args::WorkflowSpec;

#[test]
fn workflow_build_produces_workflow_spec() {
    let mut wf = Workflow::new("checkout_flow");
    let validate = wf
        .pure("validate", |_input: String| -> i32 { 0 })
        .expect("valid");
    let charge = wf
        .effect("charge", |_input: i32| -> bool { true })
        .expect("valid");
    wf.connect(&validate, &charge)
        .expect("connect should succeed");

    let spec = wf.build().expect("build should succeed");
    assert_eq!(spec.workflow_name.as_str(), "checkout_flow");
    assert_eq!(spec.nodes.len(), 2);
    assert_eq!(spec.edges.len(), 1);
}

#[test]
fn workflow_build_empty_returns_error() {
    let wf = Workflow::new("empty_workflow");
    let result = wf.build();
    assert!(
        matches!(result, Err(DagError::EmptyWorkflow)),
        "build should fail with EmptyWorkflow for empty workflow"
    );
}

#[test]
fn workflow_spec_serializes_to_json() {
    let mut wf = Workflow::new("test_flow");
    let _validate = wf
        .pure("validate", |_input: String| -> i32 { 0 })
        .expect("valid");

    let spec = wf.build().expect("build should succeed");
    let json = spec.to_json_bytes();
    let json_str = String::from_utf8(json).expect("json should be valid utf8");

    assert!(
        json_str.contains("\"workflow_name\""),
        "json should contain workflow_name"
    );
    assert!(json_str.contains("\"nodes\""), "json should contain nodes");
    assert!(json_str.contains("\"edges\""), "json should contain edges");
}

#[test]
fn dag_build_with_kind_produces_correct_node_kinds() {
    let mut dag = Dag::new();
    let _pure_node: crate::node_handle::NodeHandle<String, i32> = dag
        .add_node_with_kind("pure-task", NodeKind::Pure, |_i: String| -> i32 { 0 })
        .expect("valid");
    let _effect_node: crate::node_handle::NodeHandle<i32, bool> = dag
        .add_node_with_kind("effect-task", NodeKind::ManagedEffect, |_i: i32| -> bool {
            true
        })
        .expect("valid");

    let spec = dag.build("test_workflow").expect("build should succeed");

    let pure_node = spec.nodes.iter().find(|n| n.name.as_str() == "pure-task");
    let effect_node = spec.nodes.iter().find(|n| n.name.as_str() == "effect-task");

    assert_eq!(
        pure_node.expect("pure_node should exist").kind,
        NodeKind::Pure
    );
    assert_eq!(
        effect_node.expect("effect_node should exist").kind,
        NodeKind::ManagedEffect
    );
}

#[test]
fn dag_build_validates_workflow_name() {
    let mut dag = Dag::new();
    let _node: crate::node_handle::NodeHandle<String, i32> = dag
        .add_node_with_kind("task", NodeKind::Pure, |_i: String| -> i32 { 0 })
        .expect("valid");

    let result = dag.build("");
    assert!(
        matches!(result, Err(DagError::InvalidNodeName { .. })),
        "empty workflow name should be rejected"
    );
}

#[test]
fn emit_graph_if_requested_does_nothing_without_graph_flag() {
    use crate::graph_args::emit_graph_if_requested;

    let spec = WorkflowSpec {
        workflow_name: vo_types::WorkflowName::parse("test").unwrap(),
        nodes: vec![],
        edges: vec![],
    };

    let args = vec!["binary".to_string()];
    let result = emit_graph_if_requested(&args, &spec);
    assert!(result.is_ok(), "should return Ok when no --graph flag");
}

#[test]
fn workflow_all_node_kinds_work() {
    let mut wf = Workflow::new("kinds_test");
    let _n1: crate::node_handle::NodeHandle<(), ()> = wf.pure("p", |_i: ()| ()).expect("valid");
    let _n2: crate::node_handle::NodeHandle<(), ()> = wf.effect("e", |_i: ()| ()).expect("valid");
    let _n3: crate::node_handle::NodeHandle<(), ()> = wf.wait("w", |_i: ()| ()).expect("valid");
    let _n4: crate::node_handle::NodeHandle<(), ()> = wf.signal("s", |_i: ()| ()).expect("valid");
    let _n5: crate::node_handle::NodeHandle<(), ()> =
        wf.unsafe_node("u", |_i: ()| ()).expect("valid");

    let spec = wf.build().expect("build should succeed");
    assert_eq!(spec.nodes.len(), 5);

    let kinds: Vec<NodeKind> = spec.nodes.iter().map(|n| n.kind).collect();
    assert!(kinds.contains(&NodeKind::Pure));
    assert!(kinds.contains(&NodeKind::ManagedEffect));
    assert!(kinds.contains(&NodeKind::Wait));
    assert!(kinds.contains(&NodeKind::Signal));
    assert!(kinds.contains(&NodeKind::Unsafe));
}
