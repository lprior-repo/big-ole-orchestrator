//! Tests for the Dag struct (ADR-010: compile-time type-safe workflow graph construction).

#![allow(clippy::unwrap_used)]

use crate::dag::{Dag, DagError};
use crate::node_handle::NodeHandle;

#[test]
fn dag_error_is_std_error() {
    let err = DagError::InvalidNodeName {
        name: "bad name!".to_string(),
    };
    let _: &dyn std::error::Error = &err;
    assert!(err.to_string().contains("bad name!"), "display: {err}");
}

#[test]
fn dag_error_display_shows_node_not_found() {
    let err = DagError::NodeNotFound {
        name: "ghost".to_string(),
    };
    assert!(err.to_string().contains("ghost"), "display: {err}");
}

#[test]
fn new_creates_empty_dag() {
    let dag = Dag::new();
    assert!(dag.node_count() == 0, "new dag should have zero nodes");
    assert!(dag.edge_count() == 0, "new dag should have zero edges");
}

#[test]
fn add_node_returns_handle_with_correct_name() {
    let mut dag = Dag::new();
    let handle: NodeHandle<String, i32> = dag
        .add_node("validate", |_input: String| -> i32 { 0 })
        .expect("valid name");
    assert_eq!(handle.name(), "validate");
    assert_eq!(
        dag.node_count(),
        1,
        "dag should have one node after add_node"
    );
}

#[test]
fn add_node_rejects_empty_name() {
    let mut dag = Dag::new();
    let result = dag.add_node::<(), (), _>("", |_input: ()| {});
    assert!(
        matches!(result, Err(DagError::InvalidNodeName { .. })),
        "empty name should be rejected: {result:?}"
    );
}

#[test]
fn connect_links_two_type_compatible_nodes() {
    let mut dag = Dag::new();
    let validate: NodeHandle<String, i32> = dag
        .add_node("validate", |_input: String| -> i32 { 0 })
        .expect("valid");
    let charge: NodeHandle<i32, bool> = dag
        .add_node("charge", |_input: i32| -> bool { true })
        .expect("valid");
    dag.connect(&validate, &charge)
        .expect("connect should succeed");
    assert_eq!(
        dag.edge_count(),
        1,
        "dag should have one edge after connect"
    );
    let edges = dag.edges();
    assert_eq!(edges, vec![("validate", "charge")]);
}

#[test]
fn connect_rejects_unknown_from_node() {
    let mut dag = Dag::new();
    let _known: NodeHandle<String, i32> = dag
        .add_node("known", |_s: String| -> i32 { 0 })
        .expect("valid");
    // phantom outputs String, known accepts String → types align, but phantom not in dag
    let phantom: NodeHandle<(), String> =
        NodeHandle::new(vo_types::NodeName::parse("ghost").expect("valid name"));
    let result = dag.connect(&phantom, &_known);
    assert!(
        matches!(result, Err(DagError::NodeNotFound { .. })),
        "should reject unknown from node: {result:?}"
    );
}

#[test]
fn connect_rejects_unknown_to_node() {
    let mut dag = Dag::new();
    let _known: NodeHandle<String, i32> = dag
        .add_node("known", |_s: String| -> i32 { 0 })
        .expect("valid");
    // known outputs i32, phantom accepts i32 → types align, but phantom not in dag
    let phantom: NodeHandle<i32, ()> =
        NodeHandle::new(vo_types::NodeName::parse("ghost").expect("valid name"));
    let result = dag.connect(&_known, &phantom);
    assert!(
        matches!(result, Err(DagError::NodeNotFound { .. })),
        "should reject unknown to node: {result:?}"
    );
}

#[test]
fn build_detects_simple_cycle() {
    let mut dag = Dag::new();
    let a: NodeHandle<i32, i32> = dag.add_node("a", |_i: i32| -> i32 { 0 }).expect("valid");
    let b: NodeHandle<i32, i32> = dag.add_node("b", |_i: i32| -> i32 { 0 }).expect("valid");
    dag.connect(&a, &b).expect("a->b");
    dag.connect(&b, &a).expect("b->a");

    let result = dag.build("cycle_workflow");
    assert!(
        matches!(result, Err(DagError::CycleDetected)),
        "should detect cycle: {result:?}"
    );
}

#[test]
fn build_detects_self_loop() {
    let mut dag = Dag::new();
    let a: NodeHandle<i32, i32> = dag.add_node("a", |_i: i32| -> i32 { 0 }).expect("valid");
    dag.connect(&a, &a).expect("a->a");

    let result = dag.build("self_loop_workflow");
    assert!(
        matches!(result, Err(DagError::CycleDetected)),
        "should detect self-loop: {result:?}"
    );
}

#[test]
fn build_allows_diamond_no_cycle() {
    let mut dag = Dag::new();
    let a: NodeHandle<(), i32> = dag.add_node("a", |_i: ()| -> i32 { 0 }).expect("valid");
    let b: NodeHandle<i32, i32> = dag.add_node("b", |_i: i32| -> i32 { 0 }).expect("valid");
    let c: NodeHandle<i32, i32> = dag.add_node("c", |_i: i32| -> i32 { 0 }).expect("valid");
    let d: NodeHandle<i32, ()> = dag.add_node("d", |_i: i32| {}).expect("valid");
    dag.connect(&a, &b).expect("a->b");
    dag.connect(&a, &c).expect("a->c");
    dag.connect(&b, &d).expect("b->d");
    dag.connect(&c, &d).expect("c->d");

    let result = dag.build("diamond_workflow");
    assert!(result.is_ok(), "diamond pattern has no cycle: {result:?}");
}

#[test]
fn edges_returns_all_edges_after_multiple_connects() {
    let mut dag = Dag::new();
    let step_a: NodeHandle<String, i32> = dag
        .add_node("step-a", |_s: String| -> i32 { 0 })
        .expect("valid");
    let step_b: NodeHandle<i32, bool> = dag
        .add_node("step-b", |_i: i32| -> bool { true })
        .expect("valid");
    let step_c: NodeHandle<bool, ()> = dag.add_node("step-c", |_b: bool| {}).expect("valid");
    dag.connect(&step_a, &step_b).expect("connect a->b");
    dag.connect(&step_b, &step_c).expect("connect b->c");
    assert_eq!(dag.edge_count(), 2);
    let edges = dag.edges();
    assert_eq!(edges, vec![("step-a", "step-b"), ("step-b", "step-c")]);
}
