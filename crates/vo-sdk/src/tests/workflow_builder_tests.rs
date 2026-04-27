//! Tests for the Workflow builder and WorkflowSpec emission (ADR-004, ADR-009, ADR-031).

#![allow(clippy::unwrap_used)]

use vo_types::NodeKind;

use crate::dag::{Dag, DagError, Workflow};
use crate::graph::WorkflowSpec;
use crate::node_handle::NodeHandle;

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
    use crate::graph::emit_graph_if_requested;

    let spec = WorkflowSpec {
        workflow_name: vo_types::WorkflowName::parse("test").unwrap(),
        nodes: vec![],
        edges: vec![],
        dedupe_scope: vo_types::DedupeScope::default(),
        guarantee_class: vo_types::GuaranteeClass::default(),
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

#[test]
fn workflow_pure_rejects_invalid_name() {
    let mut wf = Workflow::new("wf");
    let result: Result<NodeHandle<(), ()>, _> = wf.pure("", |_i: ()| ());
    assert!(
        matches!(result, Err(DagError::InvalidNodeName { .. })),
        "empty name should be rejected"
    );
}

#[test]
fn workflow_effect_rejects_invalid_name() {
    let mut wf = Workflow::new("wf");
    let result: Result<NodeHandle<(), ()>, _> = wf.effect("", |_i: ()| ());
    assert!(
        matches!(result, Err(DagError::InvalidNodeName { .. })),
        "empty name should be rejected"
    );
}

#[test]
fn workflow_wait_rejects_invalid_name() {
    let mut wf = Workflow::new("wf");
    let result: Result<NodeHandle<(), ()>, _> = wf.wait("", |_i: ()| ());
    assert!(
        matches!(result, Err(DagError::InvalidNodeName { .. })),
        "empty name should be rejected"
    );
}

#[test]
fn workflow_signal_rejects_invalid_name() {
    let mut wf = Workflow::new("wf");
    let result: Result<NodeHandle<(), ()>, _> = wf.signal("", |_i: ()| ());
    assert!(
        matches!(result, Err(DagError::InvalidNodeName { .. })),
        "empty name should be rejected"
    );
}

#[test]
fn workflow_unsafe_node_rejects_invalid_name() {
    let mut wf = Workflow::new("wf");
    let result: Result<NodeHandle<(), ()>, _> = wf.unsafe_node("", |_i: ()| ());
    assert!(
        matches!(result, Err(DagError::InvalidNodeName { .. })),
        "empty name should be rejected"
    );
}

#[test]
fn workflow_connect_rejects_unknown_node() {
    let mut wf = Workflow::new("wf");
    let known: NodeHandle<String, i32> =
        wf.pure("known", |_s: String| -> i32 { 0 }).expect("valid");
    let phantom: NodeHandle<i32, ()> =
        NodeHandle::new(vo_types::NodeName::parse("ghost").expect("valid name"));
    let result = wf.connect(&known, &phantom);
    assert!(
        matches!(result, Err(DagError::NodeNotFound { .. })),
        "should reject unknown node"
    );
}

#[test]
fn workflow_build_with_invalid_name_returns_error() {
    let mut wf = Workflow::new("HAS CAPS");
    let _: NodeHandle<(), ()> = wf.pure("node", |_i: ()| ()).expect("valid");
    let result = wf.build();
    assert!(
        matches!(result, Err(DagError::InvalidNodeName { .. })),
        "invalid workflow name should be rejected"
    );
}

#[test]
fn workflow_build_produces_correct_edge_specs() {
    let mut wf = Workflow::new("edge_test");
    let a: NodeHandle<String, i32> = wf.pure("node-a", |_s: String| -> i32 { 0 }).expect("valid");
    let b: NodeHandle<i32, bool> = wf
        .effect("node-b", |_i: i32| -> bool { true })
        .expect("valid");
    wf.connect(&a, &b).expect("connect");

    let spec = wf.build().expect("build");
    assert_eq!(spec.edges.len(), 1);
    assert_eq!(spec.edges[0].from.as_str(), "node-a");
    assert_eq!(spec.edges[0].to.as_str(), "node-b");
}

#[test]
fn workflow_spec_to_json_bytes_is_valid_json() {
    let mut wf = Workflow::new("json_test");
    let _: NodeHandle<(), ()> = wf.pure("p", |_i: ()| ()).expect("valid");
    let spec = wf.build().expect("build");
    let bytes = spec.to_json_bytes();
    let _: serde_json::Value =
        serde_json::from_slice(&bytes).expect("to_json_bytes should produce valid JSON");
}

#[test]
fn workflow_build_linear_chain() {
    let mut wf = Workflow::new("chain");
    let a: NodeHandle<String, i32> = wf.pure("a", |_s: String| -> i32 { 0 }).expect("valid");
    let b: NodeHandle<i32, bool> = wf.effect("b", |_i: i32| -> bool { true }).expect("valid");
    let c: NodeHandle<bool, ()> = wf.wait("c", |_b: bool| ()).expect("valid");
    wf.connect(&a, &b).expect("a->b");
    wf.connect(&b, &c).expect("b->c");

    let spec = wf.build().expect("build");
    assert_eq!(spec.nodes.len(), 3);
    assert_eq!(spec.edges.len(), 2);
}

#[test]
fn workflow_with_only_wait_node() {
    let mut wf = Workflow::new("wait-only");
    let _: NodeHandle<(), ()> = wf.wait("w", |_i: ()| ()).expect("valid");
    let spec = wf.build().expect("build");
    assert_eq!(spec.nodes.len(), 1);
    assert_eq!(spec.nodes[0].kind, NodeKind::Wait);
}

#[test]
fn workflow_with_only_signal_node() {
    let mut wf = Workflow::new("signal-only");
    let _: NodeHandle<(), ()> = wf.signal("s", |_i: ()| ()).expect("valid");
    let spec = wf.build().expect("build");
    assert_eq!(spec.nodes.len(), 1);
    assert_eq!(spec.nodes[0].kind, NodeKind::Signal);
}

#[test]
fn workflow_with_only_unsafe_node() {
    let mut wf = Workflow::new("unsafe-only");
    let _: NodeHandle<(), ()> = wf.unsafe_node("u", |_i: ()| ()).expect("valid");
    let spec = wf.build().expect("build");
    assert_eq!(spec.nodes.len(), 1);
    assert_eq!(spec.nodes[0].kind, NodeKind::Unsafe);
}

#[test]
fn workflow_fan_out_pattern() {
    let mut wf = Workflow::new("fan-out");
    let source: NodeHandle<String, i32> =
        wf.pure("source", |_s: String| -> i32 { 0 }).expect("valid");
    let branch_a: NodeHandle<i32, ()> = wf.effect("branch-a", |_i: i32| ()).expect("valid");
    let branch_b: NodeHandle<i32, ()> = wf.effect("branch-b", |_i: i32| ()).expect("valid");
    wf.connect(&source, &branch_a).expect("source->a");
    wf.connect(&source, &branch_b).expect("source->b");

    let spec = wf.build().expect("build");
    assert_eq!(spec.nodes.len(), 3);
    assert_eq!(spec.edges.len(), 2);
}
