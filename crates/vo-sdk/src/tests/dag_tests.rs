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

#[test]
fn dag_error_display_shows_cycle_detected() {
    let err = DagError::CycleDetected;
    assert!(err.to_string().contains("cycle"), "display: {err}");
}

#[test]
fn dag_build_rejects_self_loop_cycle() {
    let mut dag = Dag::new();
    let node: NodeHandle<(), ()> = dag.add_node("self-loop", |_i: ()| ()).expect("valid");
    dag.connect(&node, &node).expect("connect should succeed");
    let result = dag.build("cyclic_workflow");
    assert!(
        matches!(result, Err(DagError::CycleDetected)),
        "self-loop should be detected as cycle: {result:?}"
    );
}

#[test]
fn dag_build_rejects_two_node_cycle() {
    let mut dag = Dag::new();
    let a: NodeHandle<(), ()> = dag.add_node("a", |_i: ()| ()).expect("valid");
    let b: NodeHandle<(), ()> = dag.add_node("b", |_i: ()| ()).expect("valid");
    dag.connect(&a, &b).expect("connect a->b");
    dag.connect(&b, &a).expect("connect b->a creates cycle");
    let result = dag.build("cyclic_workflow");
    assert!(
        matches!(result, Err(DagError::CycleDetected)),
        "two-node cycle should be detected: {result:?}"
    );
}

#[test]
fn dag_build_rejects_three_node_cycle() {
    let mut dag = Dag::new();
    let a: NodeHandle<(), ()> = dag.add_node("a", |_i: ()| ()).expect("valid");
    let b: NodeHandle<(), ()> = dag.add_node("b", |_i: ()| ()).expect("valid");
    let c: NodeHandle<(), ()> = dag.add_node("c", |_i: ()| ()).expect("valid");
    dag.connect(&a, &b).expect("connect a->b");
    dag.connect(&b, &c).expect("connect b->c");
    dag.connect(&c, &a).expect("connect c->a creates cycle");
    let result = dag.build("cyclic_workflow");
    assert!(
        matches!(result, Err(DagError::CycleDetected)),
        "three-node cycle should be detected: {result:?}"
    );
}

#[test]
fn dag_build_accepts_linear_chain_without_cycle() {
    let mut dag = Dag::new();
    let a: NodeHandle<(), ()> = dag.add_node("a", |_i: ()| ()).expect("valid");
    let b: NodeHandle<(), ()> = dag.add_node("b", |_i: ()| ()).expect("valid");
    let c: NodeHandle<(), ()> = dag.add_node("c", |_i: ()| ()).expect("valid");
    dag.connect(&a, &b).expect("connect a->b");
    dag.connect(&b, &c).expect("connect b->c");
    let result = dag.build("linear_workflow");
    assert!(
        result.is_ok(),
        "linear chain should not be a cycle: {result:?}"
    );
}

#[test]
fn dag_build_accepts_diamond_graph_without_cycle() {
    let mut dag = Dag::new();
    let start: NodeHandle<(), ()> = dag.add_node("start", |_i: ()| ()).expect("valid");
    let left: NodeHandle<(), ()> = dag.add_node("left", |_i: ()| ()).expect("valid");
    let right: NodeHandle<(), ()> = dag.add_node("right", |_i: ()| ()).expect("valid");
    let end: NodeHandle<(), ()> = dag.add_node("end", |_i: ()| ()).expect("valid");
    dag.connect(&start, &left).expect("connect start->left");
    dag.connect(&start, &right).expect("connect start->right");
    dag.connect(&left, &end).expect("connect left->end");
    dag.connect(&right, &end).expect("connect right->end");
    let result = dag.build("diamond_workflow");
    assert!(
        result.is_ok(),
        "diamond graph should not be a cycle: {result:?}"
    );
}

#[test]
fn dag_build_accepts_multiple_disconnected_components_without_cycle() {
    let mut dag = Dag::new();
    let a1: NodeHandle<(), ()> = dag.add_node("a1", |_i: ()| ()).expect("valid");
    let a2: NodeHandle<(), ()> = dag.add_node("a2", |_i: ()| ()).expect("valid");
    let b1: NodeHandle<(), ()> = dag.add_node("b1", |_i: ()| ()).expect("valid");
    let b2: NodeHandle<(), ()> = dag.add_node("b2", |_i: ()| ()).expect("valid");
    dag.connect(&a1, &a2).expect("connect a1->a2");
    dag.connect(&b1, &b2).expect("connect b1->b2");
    let result = dag.build("disconnected_workflow");
    assert!(
        result.is_ok(),
        "disconnected components should not be a cycle: {result:?}"
    );
}
