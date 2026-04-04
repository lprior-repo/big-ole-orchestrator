//! Tests for the Dag struct (ADR-010: compile-time type-safe workflow graph construction).

use crate::dag::Dag;
use crate::node_handle::NodeHandle;

#[test]
fn new_creates_empty_dag() {
    let dag = Dag::new();
    assert!(dag.node_count() == 0, "new dag should have zero nodes");
    assert!(dag.edge_count() == 0, "new dag should have zero edges");
}

#[test]
fn add_node_returns_handle_with_correct_name() {
    let mut dag = Dag::new();
    let handle: NodeHandle<String, i32> = dag.add_node("validate", |_input: String| -> i32 { 0 });
    assert_eq!(handle.name(), "validate");
    assert_eq!(dag.node_count(), 1, "dag should have one node after add_node");
}

#[test]
fn connect_links_two_type_compatible_nodes() {
    let mut dag = Dag::new();
    let validate: NodeHandle<String, i32> = dag.add_node("validate", |_input: String| -> i32 { 0 });
    let charge: NodeHandle<i32, bool> = dag.add_node("charge", |_input: i32| -> bool { true });
    dag.connect(&validate, &charge);
    assert_eq!(dag.edge_count(), 1, "dag should have one edge after connect");
    let edges = dag.edges();
    assert_eq!(edges, vec![("validate", "charge")]);
}

#[test]
fn edges_returns_all_edges_after_multiple_connects() {
    let mut dag = Dag::new();
    let step_a: NodeHandle<String, i32> = dag.add_node("step-a", |_s: String| -> i32 { 0 });
    let step_b: NodeHandle<i32, bool> = dag.add_node("step-b", |_i: i32| -> bool { true });
    let step_c: NodeHandle<bool, ()> = dag.add_node("step-c", |_b: bool| {});
    dag.connect(&step_a, &step_b);
    dag.connect(&step_b, &step_c);
    assert_eq!(dag.edge_count(), 2);
    let edges = dag.edges();
    assert_eq!(edges, vec![("step-a", "step-b"), ("step-b", "step-c")]);
}
