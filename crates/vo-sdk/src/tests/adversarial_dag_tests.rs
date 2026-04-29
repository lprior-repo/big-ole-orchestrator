//! Adversarial tests for vo-sdk (bead ve-z32z).
//!
//! DIMENSION: Dag builder validation edge cases.

use crate::dag::{Dag, DagError};
use crate::node_handle::NodeHandle;
use vo_types::NodeKind;

#[test]
fn dag_add_node_rejects_name_with_only_hyphens() {
    let mut dag = Dag::new();
    let result: Result<NodeHandle<(), ()>, DagError> =
        dag.add_node_with_kind("---", NodeKind::Pure, |_: ()| ());

    assert!(matches!(result, Err(DagError::InvalidNodeName { .. })));
}

#[test]
fn dag_add_node_rejects_name_starting_with_number() {
    let mut dag = Dag::new();
    let result: Result<NodeHandle<(), ()>, DagError> =
        dag.add_node_with_kind("123node", NodeKind::Pure, |_: ()| ());

    assert!(matches!(result, Err(DagError::InvalidNodeName { .. })));
}

#[test]
fn dag_add_node_rejects_consecutive_underscores() {
    let mut dag = Dag::new();
    let result: Result<NodeHandle<(), ()>, DagError> =
        dag.add_node_with_kind("node__bad", NodeKind::Pure, |_: ()| ());

    assert!(matches!(result, Err(DagError::InvalidNodeName { .. })));
}

#[test]
fn dag_add_node_accepts_name_at_max_length() {
    let name: String = "a".repeat(128);
    let mut dag = Dag::new();
    let result: Result<NodeHandle<(), ()>, DagError> =
        dag.add_node_with_kind(&name, NodeKind::Pure, |_: ()| ());

    assert!(result.is_ok(), "128-char name should be accepted");
}

#[test]
fn dag_add_node_rejects_name_over_max_length() {
    let name: String = "a".repeat(129);
    let mut dag = Dag::new();
    let result: Result<NodeHandle<(), ()>, DagError> =
        dag.add_node_with_kind(&name, NodeKind::Pure, |_: ()| ());

    assert!(matches!(result, Err(DagError::InvalidNodeName { .. })));
}

#[test]
fn dag_connect_rejects_handle_from_different_dag() {
    let mut dag1 = Dag::new();
    let mut dag2 = Dag::new();
    let a: NodeHandle<(), ()> = dag1
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .unwrap();
    let b: NodeHandle<(), ()> = dag2
        .add_node_with_kind("b", NodeKind::Pure, |_: ()| ())
        .unwrap();

    let result = dag1.connect(&a, &b);

    assert!(matches!(result, Err(DagError::NodeNotFound { name })))
}

#[test]
fn dag_build_preserves_edge_insertion_order() {
    let mut dag = Dag::new();
    let a: NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .unwrap();
    let b: NodeHandle<(), ()> = dag
        .add_node_with_kind("b", NodeKind::Pure, |_: ()| ())
        .unwrap();
    let c: NodeHandle<(), ()> = dag
        .add_node_with_kind("c", NodeKind::Pure, |_: ()| ())
        .unwrap();
    let d: NodeHandle<(), ()> = dag
        .add_node_with_kind("d", NodeKind::Pure, |_: ()| ())
        .unwrap();
    dag.connect(&a, &b).unwrap();
    dag.connect(&a, &c).unwrap();
    dag.connect(&b, &d).unwrap();

    let edges = dag.edges();
    assert_eq!(edges, vec![("a", "b"), ("a", "c"), ("b", "d")]);
}

#[test]
fn dag_build_preserves_node_insertion_order() {
    let mut dag = Dag::new();
    let _: NodeHandle<(), ()> = dag
        .add_node_with_kind("first", NodeKind::Pure, |_: ()| ())
        .unwrap();
    let _: NodeHandle<(), ()> = dag
        .add_node_with_kind("second", NodeKind::Pure, |_: ()| ())
        .unwrap();
    let _: NodeHandle<(), ()> = dag
        .add_node_with_kind("third", NodeKind::Pure, |_: ()| ())
        .unwrap();

    let spec = dag.build("order_test").unwrap();
    let names: Vec<&str> = spec.nodes.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, vec!["first", "second", "third"]);
}

#[test]
fn dag_build_rejects_workflow_name_with_special_chars() {
    let mut dag = Dag::new();
    let _: NodeHandle<(), ()> = dag
        .add_node_with_kind("node", NodeKind::Pure, |_: ()| ())
        .unwrap();

    let result = dag.build("bad name!");

    assert!(matches!(result, Err(DagError::InvalidNodeName { .. })));
}

#[test]
fn dag_build_rejects_workflow_name_with_only_numbers() {
    let mut dag = Dag::new();
    let _: NodeHandle<(), ()> = dag
        .add_node_with_kind("node", NodeKind::Pure, |_: ()| ())
        .unwrap();

    let result = dag.build("12345");

    assert!(matches!(result, Err(DagError::InvalidNodeName { .. })));
}

#[test]
fn dag_node_and_edge_count_are_consistent() {
    let mut dag = Dag::new();
    let a: NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .unwrap();
    let b: NodeHandle<(), ()> = dag
        .add_node_with_kind("b", NodeKind::Pure, |_: ()| ())
        .unwrap();
    let c: NodeHandle<(), ()> = dag
        .add_node_with_kind("c", NodeKind::Pure, |_: ()| ())
        .unwrap();
    dag.connect(&a, &b).unwrap();
    dag.connect(&b, &c).unwrap();

    assert_eq!(dag.node_count(), 3);
    assert_eq!(dag.edge_count(), 2);
}

#[test]
fn dag_default_matches_new() {
    let default_dag = Dag::default();
    let new_dag = Dag::new();
    assert_eq!(default_dag.node_count(), new_dag.node_count());
    assert_eq!(default_dag.edge_count(), new_dag.edge_count());
}
