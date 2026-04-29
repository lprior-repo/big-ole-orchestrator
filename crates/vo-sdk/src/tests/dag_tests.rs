//! Tests for the Dag struct (ADR-010: compile-time type-safe workflow graph construction).

#![allow(clippy::unwrap_used)]

use crate::dag::{Dag, DagError};
use crate::node_handle::NodeHandle;
use vo_types::NodeKind;

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
fn dag_error_display_empty_workflow() {
    let err = DagError::EmptyWorkflow;
    assert!(
        err.to_string().contains("no nodes"),
        "EmptyWorkflow display should mention no nodes: {err}"
    );
}

#[test]
fn dag_error_display_cycle_detected() {
    let err = DagError::CycleDetected {
        cycle: "a -> b".to_string(),
    };
    assert!(
        err.to_string().contains("cycle"),
        "CycleDetected display should mention cycle: {err}"
    );
}

#[test]
fn dag_error_display_invalid_node_name() {
    let err = DagError::InvalidNodeName {
        name: "bad!".to_string(),
    };
    assert!(
        err.to_string().contains("bad!"),
        "InvalidNodeName display should contain name: {err}"
    );
}

#[test]
fn dag_error_clone_and_partial_eq() {
    let err1 = DagError::EmptyWorkflow;
    let err2 = err1.clone();
    assert_eq!(err1, err2);
}

#[test]
fn dag_all_error_variants_display_non_empty() {
    let variants = vec![
        DagError::InvalidNodeName {
            name: "x".to_string(),
        },
        DagError::NodeNotFound {
            name: "y".to_string(),
        },
        DagError::EmptyWorkflow,
        DagError::CycleDetected {
            cycle: "x -> y".to_string(),
        },
    ];
    for v in &variants {
        assert!(
            !v.to_string().is_empty(),
            "Display should not be empty for {v:?}"
        );
    }
}

#[test]
fn new_creates_empty_dag() {
    let dag = Dag::new();
    assert!(dag.node_count() == 0, "new dag should have zero nodes");
    assert!(dag.edge_count() == 0, "new dag should have zero edges");
}

#[test]
fn default_creates_empty_dag() {
    let dag = Dag::default();
    assert_eq!(dag.node_count(), 0);
    assert_eq!(dag.edge_count(), 0);
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
fn add_node_with_kind_returns_handle_with_correct_name() {
    let mut dag = Dag::new();
    let handle: NodeHandle<String, i32> = dag
        .add_node_with_kind("validate", NodeKind::Pure, |_input: String| -> i32 { 0 })
        .expect("valid name");
    assert_eq!(handle.name(), "validate");
    assert_eq!(dag.node_count(), 1);
}

#[test]
fn add_node_with_kind_managed_effect() {
    let mut dag = Dag::new();
    let handle: NodeHandle<String, i32> = dag
        .add_node_with_kind(
            "effect-node",
            NodeKind::ManagedEffect,
            |_input: String| -> i32 { 0 },
        )
        .expect("valid name");
    assert_eq!(handle.name(), "effect-node");
}

#[test]
fn add_node_with_kind_wait() {
    let mut dag = Dag::new();
    let handle: NodeHandle<String, i32> = dag
        .add_node_with_kind("wait-node", NodeKind::Wait, |_input: String| -> i32 { 0 })
        .expect("valid name");
    assert_eq!(handle.name(), "wait-node");
}

#[test]
fn add_node_with_kind_signal() {
    let mut dag = Dag::new();
    let handle: NodeHandle<String, i32> = dag
        .add_node_with_kind("signal-node", NodeKind::Signal, |_input: String| -> i32 {
            0
        })
        .expect("valid name");
    assert_eq!(handle.name(), "signal-node");
}

#[test]
fn add_node_with_kind_unsafe() {
    let mut dag = Dag::new();
    let handle: NodeHandle<String, i32> = dag
        .add_node_with_kind("unsafe-node", NodeKind::Unsafe, |_input: String| -> i32 {
            0
        })
        .expect("valid name");
    assert_eq!(handle.name(), "unsafe-node");
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
fn add_node_with_kind_rejects_empty_name() {
    let mut dag = Dag::new();
    let result: Result<NodeHandle<(), ()>, _> =
        dag.add_node_with_kind("", NodeKind::Pure, |_input: ()| {});
    assert!(
        matches!(result, Err(DagError::InvalidNodeName { .. })),
        "empty name should be rejected: {result:?}"
    );
}

#[test]
fn add_node_rejects_whitespace_name() {
    let mut dag = Dag::new();
    let result = dag.add_node::<(), (), _>("  ", |_input: ()| {});
    assert!(
        matches!(result, Err(DagError::InvalidNodeName { .. })),
        "whitespace name should be rejected: {result:?}"
    );
}

#[test]
fn add_node_accepts_uppercase_name() {
    let mut dag = Dag::new();
    let result = dag.add_node::<(), (), _>("MyNode", |_input: ()| {});
    assert!(
        result.is_ok(),
        "uppercase name should be accepted: {result:?}"
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
    let phantom: NodeHandle<i32, ()> =
        NodeHandle::new(vo_types::NodeName::parse("ghost").expect("valid name"));
    let result = dag.connect(&_known, &phantom);
    assert!(
        matches!(result, Err(DagError::NodeNotFound { .. })),
        "should reject unknown to node: {result:?}"
    );
}

#[test]
fn connect_same_node_twice_creates_two_edges() {
    let mut dag = Dag::new();
    let a: NodeHandle<String, i32> = dag.add_node("a", |_s: String| -> i32 { 0 }).expect("valid");
    let b: NodeHandle<i32, bool> = dag
        .add_node("b", |_i: i32| -> bool { true })
        .expect("valid");
    dag.connect(&a, &b).expect("first connect");
    dag.connect(&a, &b).expect("second connect");
    assert_eq!(dag.edge_count(), 2, "duplicate edges should be allowed");
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
fn build_empty_dag_returns_empty_workflow() {
    let dag = Dag::new();
    let result = dag.build("some_workflow");
    assert_eq!(result, Err(DagError::EmptyWorkflow));
}

#[test]
fn build_with_invalid_workflow_name_returns_error() {
    let mut dag = Dag::new();
    let _: NodeHandle<String, i32> = dag
        .add_node_with_kind("task", NodeKind::Pure, |_s: String| -> i32 { 0 })
        .expect("valid");
    let result = dag.build("HAS CAPS");
    assert!(
        matches!(result, Err(DagError::InvalidNodeName { .. })),
        "uppercase workflow name should be rejected"
    );
}

#[test]
fn build_with_single_node_produces_spec_no_edges() {
    let mut dag = Dag::new();
    let _: NodeHandle<String, i32> = dag
        .add_node_with_kind("solo", NodeKind::Pure, |_s: String| -> i32 { 0 })
        .expect("valid");
    let spec = dag.build("solo_workflow").expect("should build");
    assert_eq!(spec.nodes.len(), 1);
    assert!(spec.edges.is_empty());
    assert_eq!(spec.nodes[0].name.as_str(), "solo");
    assert_eq!(spec.nodes[0].kind, NodeKind::Pure);
}

#[test]
fn build_preserves_edge_order() {
    let mut dag = Dag::new();
    let a: NodeHandle<String, String> = dag
        .add_node_with_kind("a", NodeKind::Pure, |s: String| s)
        .expect("valid");
    let b: NodeHandle<String, String> = dag
        .add_node_with_kind("b", NodeKind::Pure, |s: String| s)
        .expect("valid");
    let c: NodeHandle<String, String> = dag
        .add_node_with_kind("c", NodeKind::Pure, |s: String| s)
        .expect("valid");
    dag.connect(&a, &b).expect("a->b");
    dag.connect(&a, &c).expect("a->c");
    let spec = dag.build("edge-order").expect("build");
    assert_eq!(spec.edges.len(), 2);
    assert_eq!(spec.edges[0].from.as_str(), "a");
    assert_eq!(spec.edges[0].to.as_str(), "b");
    assert_eq!(spec.edges[1].from.as_str(), "a");
    assert_eq!(spec.edges[1].to.as_str(), "c");
}

#[test]
fn edges_on_empty_dag_returns_empty() {
    let dag = Dag::new();
    assert!(dag.edges().is_empty());
}

#[test]
fn node_count_tracks_multiple_adds() {
    let mut dag = Dag::new();
    for i in 0..5 {
        let name = format!("node-{i}");
        let _: NodeHandle<String, String> = dag
            .add_node_with_kind(&name, NodeKind::Pure, |s: String| s)
            .expect("valid");
    }
    assert_eq!(dag.node_count(), 5);
}

#[test]
fn dag_dynamic_add_to_front_new_source_nodes() {
    let mut dag = Dag::new();
    let b: NodeHandle<String, String> = dag
        .add_node_with_kind("b", NodeKind::Pure, |_s: String| -> String { _s })
        .expect("valid");
    let c: NodeHandle<String, bool> = dag
        .add_node_with_kind("c", NodeKind::Pure, |_s: String| -> bool { true })
        .expect("valid");
    dag.connect(&b, &c).expect("b->c");
    assert_eq!(dag.node_count(), 2);
    assert_eq!(dag.edge_count(), 1);
    let a: NodeHandle<String, String> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_s: String| -> String { _s })
        .expect("valid");
    dag.connect(&a, &b).expect("a->b");
    assert_eq!(dag.node_count(), 3);
    assert_eq!(dag.edge_count(), 2);
    let spec = dag.build("front_add").expect("should build");
    assert_eq!(spec.nodes.len(), 3);
    assert_eq!(spec.edges.len(), 2);
}

#[test]
fn dag_dynamic_add_to_back_new_terminal_nodes() {
    let mut dag = Dag::new();
    let a: NodeHandle<String, i32> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_s: String| -> i32 { 0 })
        .expect("valid");
    let b: NodeHandle<i32, bool> = dag
        .add_node_with_kind("b", NodeKind::Pure, |_i: i32| -> bool { true })
        .expect("valid");
    dag.connect(&a, &b).expect("a->b");
    assert_eq!(dag.node_count(), 2);
    assert_eq!(dag.edge_count(), 1);
    let c: NodeHandle<bool, ()> = dag
        .add_node_with_kind("c", NodeKind::Pure, |_b: bool| -> () { () })
        .expect("valid");
    dag.connect(&b, &c).expect("b->c");
    assert_eq!(dag.node_count(), 3);
    assert_eq!(dag.edge_count(), 2);
    let spec = dag.build("back_add").expect("should build");
    assert_eq!(spec.nodes.len(), 3);
    assert_eq!(spec.edges.len(), 2);
}

#[test]
fn dag_dynamic_add_mid_parallel_branch() {
    let mut dag = Dag::new();
    let a: NodeHandle<String, i32> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_s: String| -> i32 { 0 })
        .expect("valid");
    let b: NodeHandle<i32, bool> = dag
        .add_node_with_kind("b", NodeKind::Pure, |_i: i32| -> bool { true })
        .expect("valid");
    dag.connect(&a, &b).expect("a->b");
    assert_eq!(dag.node_count(), 2);
    assert_eq!(dag.edge_count(), 1);
    let c: NodeHandle<i32, String> = dag
        .add_node_with_kind("c", NodeKind::Pure, |_i: i32| -> String { String::new() })
        .expect("valid");
    dag.connect(&a, &c).expect("a->c (parallel branch)");
    assert_eq!(dag.node_count(), 3);
    assert_eq!(dag.edge_count(), 2);
    let spec = dag.build("mid_add").expect("should build");
    assert_eq!(spec.nodes.len(), 3);
    assert_eq!(spec.edges.len(), 2);
}

#[test]
fn dag_dynamic_add_mid_chain_extension() {
    let mut dag = Dag::new();
    let a: NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_i: ()| -> () { () })
        .expect("valid");
    let d: NodeHandle<(), ()> = dag
        .add_node_with_kind("d", NodeKind::Pure, |_i: ()| -> () { () })
        .expect("valid");
    dag.connect(&a, &d).expect("a->d");
    assert_eq!(dag.node_count(), 2);
    assert_eq!(dag.edge_count(), 1);
    let b: NodeHandle<(), ()> = dag
        .add_node_with_kind("b", NodeKind::Pure, |_i: ()| -> () { () })
        .expect("valid");
    let c: NodeHandle<(), ()> = dag
        .add_node_with_kind("c", NodeKind::Pure, |_i: ()| -> () { () })
        .expect("valid");
    dag.connect(&a, &b).expect("a->b");
    dag.connect(&b, &c).expect("b->c");
    dag.connect(&c, &d).expect("c->d");
    assert_eq!(dag.node_count(), 4);
    assert_eq!(dag.edge_count(), 4);
    let spec = dag.build("chain_extend").expect("should build");
    assert_eq!(spec.nodes.len(), 4);
    assert_eq!(spec.edges.len(), 4);
}

#[test]
fn dag_dynamic_add_multiple_waves_no_corruption() {
    let mut dag = Dag::new();
    let wave1_a: NodeHandle<(), ()> = dag
        .add_node_with_kind("w1-a", NodeKind::Pure, |_i: ()| -> () { () })
        .expect("valid");
    let wave1_b: NodeHandle<(), ()> = dag
        .add_node_with_kind("w1-b", NodeKind::Pure, |_i: ()| -> () { () })
        .expect("valid");
    dag.connect(&wave1_a, &wave1_b).expect("w1-a->w1-b");
    let wave2_a: NodeHandle<(), ()> = dag
        .add_node_with_kind("w2-a", NodeKind::Pure, |_i: ()| -> () { () })
        .expect("valid");
    let wave2_b: NodeHandle<(), ()> = dag
        .add_node_with_kind("w2-b", NodeKind::Pure, |_i: ()| -> () { () })
        .expect("valid");
    dag.connect(&wave2_a, &wave2_b).expect("w2-a->w2-b");
    dag.connect(&wave1_b, &wave2_a).expect("w1-b->w2-a");
    assert_eq!(dag.node_count(), 4);
    assert_eq!(dag.edge_count(), 3);
    let spec = dag.build("multi_wave").expect("should build");
    assert_eq!(spec.nodes.len(), 4);
    assert_eq!(spec.edges.len(), 3);
    let edge_names: Vec<(&str, &str)> = spec
        .edges
        .iter()
        .map(|e| (e.from.as_str(), e.to.as_str()))
        .collect();
    assert!(edge_names.contains(&("w1-a", "w1-b")));
    assert!(edge_names.contains(&("w2-a", "w2-b")));
    assert!(edge_names.contains(&("w1-b", "w2-a")));
}

#[test]
fn dag_dynamic_add_interleaved_build_linear_chain() {
    let mut dag = Dag::new();
    let nodes: Vec<NodeHandle<i32, i32>> = (0..5)
        .map(|i| {
            dag.add_node_with_kind(&format!("n{}", i), NodeKind::Pure, |x: i32| x)
                .expect("valid")
        })
        .collect();
    for i in 0..nodes.len() - 1 {
        dag.connect(&nodes[i], &nodes[i + 1]).expect("connect consecutive");
    }
    assert_eq!(dag.node_count(), 5);
    assert_eq!(dag.edge_count(), 4);
    let spec = dag.build("interleave").expect("should build");
    assert_eq!(spec.nodes.len(), 5);
    assert_eq!(spec.edges.len(), 4);
}
