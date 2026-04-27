//! Tests for the Dag struct (ADR-010: compile-time type-safe workflow graph construction).

#![allow(clippy::unwrap_used)]

use crate::dag::{Dag, DagError, Workflow};
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
fn dag_error_display_duplicate_node_name() {
    let err = DagError::DuplicateNodeName {
        name: "dup-node".to_string(),
    };
    assert!(
        err.to_string().contains("dup-node"),
        "DuplicateNodeName display should contain name: {err}"
    );
}

#[test]
fn dag_error_display_self_loop() {
    let err = DagError::SelfLoop {
        name: "loop-node".to_string(),
    };
    assert!(
        err.to_string().contains("loop-node"),
        "SelfLoop display should contain name: {err}"
    );
    assert!(
        err.to_string().contains("self-loop"),
        "SelfLoop display should mention self-loop: {err}"
    );
}

#[test]
fn dag_error_display_orphan_node() {
    let err = DagError::OrphanNode {
        name: "orphan".to_string(),
    };
    assert!(
        err.to_string().contains("orphan"),
        "OrphanNode display should contain name: {err}"
    );
    assert!(
        err.to_string().contains("no edges"),
        "OrphanNode display should mention no edges: {err}"
    );
}

#[test]
fn add_node_with_kind_rejects_duplicate_name() {
    let mut dag = Dag::new();
    let _: NodeHandle<String, i32> = dag
        .add_node_with_kind("same-name", NodeKind::Pure, |_s: String| -> i32 { 0 })
        .expect("first add should succeed");
    let result: Result<NodeHandle<String, i32>, _> =
        dag.add_node_with_kind("same-name", NodeKind::Pure, |_s: String| -> i32 { 0 });
    assert!(
        matches!(result, Err(DagError::DuplicateNodeName { .. })),
        "duplicate name should be rejected: {result:?}"
    );
}

#[test]
fn build_rejects_cycle_a_to_b_to_a() {
    let mut dag = Dag::new();
    let a: NodeHandle<String, i32> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_s: String| -> i32 { 0 })
        .expect("valid");
    let b: NodeHandle<i32, bool> = dag
        .add_node_with_kind("b", NodeKind::Pure, |_i: i32| -> bool { true })
        .expect("valid");
    dag.connect(&a, &b).expect("connect a->b should work");
    dag.connect(&b, &a).expect("connect b->a creates cycle");
    let result = dag.build("cycle-workflow");
    assert!(
        matches!(result, Err(DagError::CycleDetected { .. })),
        "cycle should be detected: {result:?}"
    );
}

#[test]
fn build_rejects_self_loop() {
    let mut dag = Dag::new();
    let a: NodeHandle<String, String> = dag
        .add_node_with_kind("self-node", NodeKind::Pure, |s: String| s)
        .expect("valid");
    dag.connect(&a, &a).expect("connect self-loop should work");
    let result = dag.build("self-loop-workflow");
    assert!(
        matches!(result, Err(DagError::CycleDetected { .. })),
        "self-loop should be detected as cycle: {result:?}"
    );
}

#[test]
fn build_rejects_orphan_node() {
    let mut dag = Dag::new();
    let _a: NodeHandle<String, i32> = dag
        .add_node_with_kind("orphan", NodeKind::Pure, |_s: String| -> i32 { 0 })
        .expect("valid");
    let result = dag.build("orphan-workflow");
    assert!(
        matches!(result, Err(DagError::OrphanNode { .. })),
        "orphan node should be rejected: {result:?}"
    );
}

#[test]
fn build_rejects_multiple_orphans() {
    let mut dag = Dag::new();
    let _a: NodeHandle<String, i32> = dag
        .add_node_with_kind("orphan-a", NodeKind::Pure, |_s: String| -> i32 { 0 })
        .expect("valid");
    let _b: NodeHandle<String, i32> = dag
        .add_node_with_kind("orphan-b", NodeKind::Pure, |_s: String| -> i32 { 0 })
        .expect("valid");
    let result = dag.build("multi-orphan-workflow");
    let err = result.unwrap_err();
    assert!(
        matches!(&err, DagError::OrphanNode { name } if name.contains("orphan-a") && name.contains("orphan-b")),
        "multiple orphans should be reported: {:?}",
        err
    );
}

#[test]
fn build_accepts_linear_chain() {
    let mut dag = Dag::new();
    let a: NodeHandle<String, i32> = dag
        .add_node_with_kind("start", NodeKind::Pure, |_s: String| -> i32 { 0 })
        .expect("valid");
    let b: NodeHandle<i32, bool> = dag
        .add_node_with_kind("middle", NodeKind::Pure, |_i: i32| -> bool { true })
        .expect("valid");
    let c: NodeHandle<bool, ()> = dag
        .add_node_with_kind("end", NodeKind::Pure, |_b: bool| {})
        .expect("valid");
    dag.connect(&a, &b).expect("a->b");
    dag.connect(&b, &c).expect("b->c");
    let spec = dag.build("linear-workflow").expect("linear chain should build");
    assert_eq!(spec.nodes.len(), 3);
    assert_eq!(spec.edges.len(), 2);
}

#[test]
fn build_accepts_diamond_dag() {
    let mut dag = Dag::new();
    let start: NodeHandle<(), i32> = dag
        .add_node_with_kind("start", NodeKind::Pure, |_: ()| -> i32 { 0 })
        .expect("valid");
    let left: NodeHandle<i32, String> = dag
        .add_node_with_kind("left", NodeKind::Pure, |_i: i32| -> String { "left".to_string() })
        .expect("valid");
    let right: NodeHandle<i32, String> = dag
        .add_node_with_kind("right", NodeKind::Pure, |_i: i32| -> String { "right".to_string() })
        .expect("valid");
    let end: NodeHandle<String, ()> = dag
        .add_node_with_kind("end", NodeKind::Pure, |_s: String| {})
        .expect("valid");
    dag.connect(&start, &left).expect("start->left");
    dag.connect(&start, &right).expect("start->right");
    dag.connect(&left, &end).expect("left->end");
    dag.connect(&right, &end).expect("right->end");
    let spec = dag.build("diamond-workflow").expect("diamond should build");
    assert_eq!(spec.nodes.len(), 4);
    assert_eq!(spec.edges.len(), 4);
}

#[test]
fn workflow_new_creates_empty_dag() {
    let wf = Workflow::new("test-workflow");
    assert_eq!(wf.build(), Err(DagError::EmptyWorkflow));
}

#[test]
fn workflow_pure_adds_node_with_pure_kind() {
    let mut wf = Workflow::new("test");
    let handle: NodeHandle<String, i32> = wf
        .pure("step", |_s: String| -> i32 { 0 })
        .expect("pure node should be added");
    assert_eq!(handle.name(), "step");
    let spec = wf.build().expect("build should succeed");
    assert_eq!(spec.nodes.len(), 1);
}

#[test]
fn workflow_effect_adds_node_with_managed_effect_kind() {
    let mut wf = Workflow::new("test");
    let handle: NodeHandle<String, i32> = wf
        .effect("effect-step", |_s: String| -> i32 { 0 })
        .expect("effect node should be added");
    assert_eq!(handle.name(), "effect-step");
}

#[test]
fn workflow_wait_adds_node_with_wait_kind() {
    let mut wf = Workflow::new("test");
    let handle: NodeHandle<String, i32> = wf
        .wait("wait-step", |_s: String| -> i32 { 0 })
        .expect("wait node should be added");
    assert_eq!(handle.name(), "wait-step");
}

#[test]
fn workflow_signal_adds_node_with_signal_kind() {
    let mut wf = Workflow::new("test");
    let handle: NodeHandle<String, i32> = wf
        .signal("signal-step", |_s: String| -> i32 { 0 })
        .expect("signal node should be added");
    assert_eq!(handle.name(), "signal-step");
}

#[test]
fn workflow_unsafe_node_adds_node_with_unsafe_kind() {
    let mut wf = Workflow::new("test");
    let handle: NodeHandle<String, i32> = wf
        .unsafe_node("unsafe-step", |_s: String| -> i32 { 0 })
        .expect("unsafe node should be added");
    assert_eq!(handle.name(), "unsafe-step");
}

#[test]
fn workflow_connect_links_nodes() {
    let mut wf = Workflow::new("test");
    let a: NodeHandle<String, i32> = wf
        .pure("a", |_s: String| -> i32 { 0 })
        .expect("a");
    let b: NodeHandle<i32, ()> = wf
        .effect("b", |_i: i32| {})
        .expect("b");
    wf.connect(&a, &b).expect("connect should succeed");
    let spec = wf.build().expect("build should succeed");
    assert_eq!(spec.edges.len(), 1);
}

#[test]
fn workflow_connect_rejects_unknown_nodes() {
    let mut wf = Workflow::new("test");
    let a: NodeHandle<String, i32> = wf
        .pure("a", |_s: String| -> i32 { 0 })
        .expect("a");
    let phantom: NodeHandle<(), String> =
        NodeHandle::new(vo_types::NodeName::parse("ghost").expect("valid"));
    let result = wf.connect(&phantom, &a);
    assert!(
        matches!(result, Err(DagError::NodeNotFound { .. })),
        "unknown from node should be rejected: {result:?}"
    );
}

#[test]
fn workflow_build_rejects_cycle() {
    let mut wf = Workflow::new("test");
    let a: NodeHandle<String, i32> = wf.pure("a", |_s: String| -> i32 { 0 }).expect("a");
    let b: NodeHandle<i32, String> = wf.pure("b", |_i: i32| -> String { "b".to_string() }).expect("b");
    wf.connect(&a, &b).expect("a->b");
    wf.connect(&b, &a).expect("b->a creates cycle");
    let result = wf.build();
    assert!(
        matches!(result, Err(DagError::CycleDetected { .. })),
        "cycle should be detected: {result:?}"
    );
}

#[test]
fn workflow_build_rejects_orphan() {
    let mut wf = Workflow::new("test");
    let _orphan: NodeHandle<String, i32> = wf
        .pure("orphan", |_s: String| -> i32 { 0 })
        .expect("orphan");
    let result = wf.build();
    assert!(
        matches!(result, Err(DagError::OrphanNode { .. })),
        "orphan should be rejected: {result:?}"
    );
}

#[test]
fn build_rejects_three_node_cycle() {
    let mut dag = Dag::new();
    let a: NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    let b: NodeHandle<(), ()> = dag
        .add_node_with_kind("b", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    let c: NodeHandle<(), ()> = dag
        .add_node_with_kind("c", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    dag.connect(&a, &b).expect("a->b");
    dag.connect(&b, &c).expect("b->c");
    dag.connect(&c, &a).expect("c->a creates cycle");
    let result = dag.build("cycle3");
    assert!(
        matches!(result, Err(DagError::CycleDetected { .. })),
        "three-node cycle should be detected: {result:?}"
    );
}

#[test]
fn build_reports_all_nodes_in_complex_cycle() {
    let mut dag = Dag::new();
    let a: NodeHandle<(), ()> = dag
        .add_node_with_kind("node-a", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    let b: NodeHandle<(), ()> = dag
        .add_node_with_kind("node-b", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    let c: NodeHandle<(), ()> = dag
        .add_node_with_kind("node-c", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    dag.connect(&a, &b).expect("a->b");
    dag.connect(&b, &c).expect("b->c");
    dag.connect(&c, &a).expect("c->a");
    let result = dag.build("cycle3");
    let err = result.unwrap_err();
    let err_str = err.to_string();
    assert!(
        err_str.contains("node-a") || err_str.contains("node-b") || err_str.contains("node-c"),
        "cycle error should mention cycle nodes: {err_str}"
    );
}

#[test]
fn build_with_mixed_orphans_and_connected_nodes() {
    let mut dag = Dag::new();
    let _orphan1: NodeHandle<(), ()> = dag
        .add_node_with_kind("orphan-1", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    let _orphan2: NodeHandle<(), ()> = dag
        .add_node_with_kind("orphan-2", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    let root: NodeHandle<(), ()> = dag
        .add_node_with_kind("root", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    let child: NodeHandle<(), ()> = dag
        .add_node_with_kind("child", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    dag.connect(&root, &child).expect("root->child");
    let result = dag.build("mixed");
    let err = result.unwrap_err();
    assert!(
        matches!(&err, DagError::OrphanNode { name } if name.contains("orphan-1") && name.contains("orphan-2")),
        "should report multiple orphans: {err:?}"
    );
}

#[test]
fn build_with_single_orphan_reports_correctly() {
    let mut dag = Dag::new();
    let _connected1: NodeHandle<(), ()> = dag
        .add_node_with_kind("conn-1", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    let _connected2: NodeHandle<(), ()> = dag
        .add_node_with_kind("conn-2", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    let _orphan: NodeHandle<(), ()> = dag
        .add_node_with_kind("solo", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    dag.connect(&_connected1, &_connected2).expect("conn1->conn2");
    let result = dag.build("single-orphan");
    let err = result.unwrap_err();
    assert!(
        matches!(&err, DagError::OrphanNode { name } if name.contains("solo")),
        "should report solo orphan: {err:?}"
    );
}

#[test]
fn build_rejects_disconnected_components() {
    let mut dag = Dag::new();
    let a: NodeHandle<(), ()> = dag
        .add_node_with_kind("comp-a1", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    let _a2: NodeHandle<(), ()> = dag
        .add_node_with_kind("comp-a2", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    let b: NodeHandle<(), ()> = dag
        .add_node_with_kind("comp-b1", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    let _b2: NodeHandle<(), ()> = dag
        .add_node_with_kind("comp-b2", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    dag.connect(&a, &_a2).expect("comp-a1->comp-a2");
    dag.connect(&b, &_b2).expect("comp-b1->comp-b2");
    let result = dag.build("disconnected");
    let err = result.unwrap_err();
    assert!(
        matches!(&err, DagError::OrphanNode { .. }),
        "disconnected components should be orphans: {err:?}"
    );
}

#[test]
fn build_accepts_star_dag() {
    let mut dag = Dag::new();
    let center: NodeHandle<(), i32> = dag
        .add_node_with_kind("center", NodeKind::Pure, |_: ()| -> i32 { 0 })
        .expect("valid");
    let leaf1: NodeHandle<i32, ()> = dag
        .add_node_with_kind("leaf-1", NodeKind::Pure, |_: i32| {})
        .expect("valid");
    let leaf2: NodeHandle<i32, ()> = dag
        .add_node_with_kind("leaf-2", NodeKind::Pure, |_: i32| {})
        .expect("valid");
    let leaf3: NodeHandle<i32, ()> = dag
        .add_node_with_kind("leaf-3", NodeKind::Pure, |_: i32| {})
        .expect("valid");
    dag.connect(&center, &leaf1).expect("center->leaf1");
    dag.connect(&center, &leaf2).expect("center->leaf2");
    dag.connect(&center, &leaf3).expect("center->leaf3");
    let spec = dag.build("star").expect("star DAG should build");
    assert_eq!(spec.nodes.len(), 4);
    assert_eq!(spec.edges.len(), 3);
}

#[test]
fn build_accepts_deep_chain() {
    let mut dag = Dag::new();
    let mut prev: NodeHandle<(), ()> = dag
        .add_node_with_kind("step-0", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    for i in 1..10 {
        let next: NodeHandle<(), ()> = dag
            .add_node_with_kind(&format!("step-{}", i), NodeKind::Pure, |_: ()| {})
            .expect("valid");
        dag.connect(&prev, &next).expect(&format!("step-{}->step-{}", i - 1, i));
        prev = next;
    }
    let spec = dag.build("deep-chain").expect("deep chain should build");
    assert_eq!(spec.nodes.len(), 10);
    assert_eq!(spec.edges.len(), 9);
}

#[test]
fn build_accepts_parallel_branches_merging() {
    let mut dag = Dag::new();
    let split: NodeHandle<(), i32> = dag
        .add_node_with_kind("split", NodeKind::Pure, |_: ()| -> i32 { 0 })
        .expect("valid");
    let left1: NodeHandle<i32, String> = dag
        .add_node_with_kind("left-1", NodeKind::Pure, |i: i32| -> String { format!("l1:{}", i) })
        .expect("valid");
    let left2: NodeHandle<String, ()> = dag
        .add_node_with_kind("left-2", NodeKind::Pure, |_: String| {})
        .expect("valid");
    let right1: NodeHandle<i32, String> = dag
        .add_node_with_kind("right-1", NodeKind::Pure, |i: i32| -> String { format!("r1:{}", i) })
        .expect("valid");
    let right2: NodeHandle<String, ()> = dag
        .add_node_with_kind("right-2", NodeKind::Pure, |_: String| {})
        .expect("valid");
    let merge: NodeHandle<String, ()> = dag
        .add_node_with_kind("merge", NodeKind::Pure, |_: String| {})
        .expect("valid");
    dag.connect(&split, &left1).expect("split->left1");
    dag.connect(&left1, &left2).expect("left1->left2");
    dag.connect(&split, &right1).expect("split->right1");
    dag.connect(&right1, &right2).expect("right1->right2");
    dag.connect(&left2, &merge).expect("left2->merge");
    dag.connect(&right2, &merge).expect("right2->merge");
    let spec = dag.build("merge-dag").expect("merge DAG should build");
    assert_eq!(spec.nodes.len(), 6);
    assert_eq!(spec.edges.len(), 6);
}

#[test]
fn build_rejects_cycle_in_subgraph() {
    let mut dag = Dag::new();
    let a: NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    let b: NodeHandle<(), ()> = dag
        .add_node_with_kind("b", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    let c: NodeHandle<(), ()> = dag
        .add_node_with_kind("c", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    let d: NodeHandle<(), ()> = dag
        .add_node_with_kind("d", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    dag.connect(&a, &b).expect("a->b");
    dag.connect(&b, &c).expect("b->c");
    dag.connect(&c, &b).expect("c->b creates cycle in subgraph");
    dag.connect(&a, &d).expect("a->d");
    let result = dag.build("subgraph-cycle");
    assert!(
        matches!(result, Err(DagError::CycleDetected { .. })),
        "cycle in subgraph should be detected: {result:?}"
    );
}

#[test]
fn workflow_signal_with_meta_sets_signal_metadata() {
    let mut dag = Dag::new();
    let meta = crate::graph::SignalNodeMeta {
        signal_name: Some("test-signal".to_string()),
        timeout_ms: Some(5000),
    };
    let handle: NodeHandle<(), ()> = dag
        .add_node_with_kind("wait-node", NodeKind::Wait, |_: ()| {})
        .expect("valid");
    dag.set_signal_meta(meta.clone());
    assert_eq!(dag.node_count(), 1);
}

#[test]
fn workflow_wait_with_meta_sets_signal_metadata() {
    let mut dag = Dag::new();
    let meta = crate::graph::SignalNodeMeta {
        signal_name: Some("wait-signal".to_string()),
        timeout_ms: Some(3000),
    };
    let handle: NodeHandle<(), ()> = dag
        .add_node_with_kind("signal-node", NodeKind::Signal, |_: ()| {})
        .expect("valid");
    dag.set_signal_meta(meta.clone());
    assert_eq!(dag.node_count(), 1);
}

#[test]
fn dag_set_signal_meta_on_non_signal_node_is_noop() {
    let mut dag = Dag::new();
    let meta = crate::graph::SignalNodeMeta {
        signal_name: Some("test".to_string()),
        timeout_ms: Some(1000),
    };
    let _handle: NodeHandle<(), ()> = dag
        .add_node_with_kind("pure-node", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    dag.set_signal_meta(meta);
    assert_eq!(dag.node_count(), 1);
}

#[test]
fn build_accepts_single_node_with_self_loop_rejected() {
    let mut dag = Dag::new();
    let node: NodeHandle<(), ()> = dag
        .add_node_with_kind("self-node", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    dag.connect(&node, &node).expect("self-loop should be allowed in connect");
    let result = dag.build("self-loop-workflow");
    assert!(
        matches!(result, Err(DagError::CycleDetected { .. })),
        "self-loop should be detected as cycle: {result:?}"
    );
}

#[test]
fn dag_node_kind_defaults_to_pure_on_add_node() {
    let mut dag = Dag::new();
    #[allow(deprecated)]
    let _handle: NodeHandle<(), ()> = dag.add_node("test", |_: ()| {}).expect("valid");
    let spec = dag.build("default-kind").expect("build should succeed");
    assert_eq!(spec.nodes[0].kind, NodeKind::Pure);
}

#[test]
fn workflow_wait_adds_wait_kind() {
    let mut wf = Workflow::new("wait-kind-test");
    let handle: NodeHandle<(), ()> = wf.wait("wait-node", |_: ()| {}).expect("valid");
    assert_eq!(handle.name(), "wait-node");
    let spec = wf.build().expect("build should succeed");
    assert_eq!(spec.nodes[0].kind, NodeKind::Wait);
}

#[test]
fn workflow_signal_adds_signal_kind() {
    let mut wf = Workflow::new("signal-kind-test");
    let handle: NodeHandle<(), ()> = wf.signal("signal-node", |_: ()| {}).expect("valid");
    assert_eq!(handle.name(), "signal-node");
    let spec = wf.build().expect("build should succeed");
    assert_eq!(spec.nodes[0].kind, NodeKind::Signal);
}

#[test]
fn workflow_unsafe_node_adds_unsafe_kind() {
    let mut wf = Workflow::new("unsafe-kind-test");
    let handle: NodeHandle<(), ()> = wf.unsafe_node("unsafe-node", |_: ()| {}).expect("valid");
    assert_eq!(handle.name(), "unsafe-node");
    let spec = wf.build().expect("build should succeed");
    assert_eq!(spec.nodes[0].kind, NodeKind::Unsafe);
}

#[test]
fn workflow_connect_same_nodes_multiple_times() {
    let mut dag = Dag::new();
    let a: NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    let b: NodeHandle<(), ()> = dag
        .add_node_with_kind("b", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    dag.connect(&a, &b).expect("first connect");
    dag.connect(&a, &b).expect("second connect");
    dag.connect(&a, &b).expect("third connect");
    assert_eq!(dag.edge_count(), 3, "multiple edges between same nodes should be allowed");
}

#[test]
fn dag_edges_returns_all_registered_edges() {
    let mut dag = Dag::new();
    let a: NodeHandle<(), ()> = dag
        .add_node_with_kind("x", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    let b: NodeHandle<(), ()> = dag
        .add_node_with_kind("y", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    let c: NodeHandle<(), ()> = dag
        .add_node_with_kind("z", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    dag.connect(&a, &b).expect("a->b");
    dag.connect(&a, &c).expect("a->c");
    dag.connect(&b, &c).expect("b->c");
    let edges = dag.edges();
    assert_eq!(edges.len(), 3);
    assert!(edges.contains(&("x", "y")));
    assert!(edges.contains(&("x", "z")));
    assert!(edges.contains(&("y", "z")));
}

#[test]
fn dag_build_preserves_node_order() {
    let mut dag = Dag::new();
    let names = ["first", "second", "third", "fourth"];
    for name in &names {
        let _: NodeHandle<(), ()> = dag
            .add_node_with_kind(name, NodeKind::Pure, |_: ()| {})
            .expect("valid");
    }
    let spec = dag.build("order-test").expect("build should succeed");
    let node_names: Vec<_> = spec.nodes.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(node_names, names);
}

#[test]
fn dag_build_with_dedup_scope_default() {
    let mut dag = Dag::new();
    let _: NodeHandle<(), ()> = dag
        .add_node_with_kind("solo", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    let spec = dag.build("dedup-test").expect("build should succeed");
    assert_eq!(spec.dedupe_scope, vo_types::DedupeScope::default());
}

#[test]
fn dag_build_with_guarantee_class_default() {
    let mut dag = Dag::new();
    let _: NodeHandle<(), ()> = dag
        .add_node_with_kind("solo", NodeKind::Pure, |_: ()| {})
        .expect("valid");
    let spec = dag.build("guarantee-test").expect("build should succeed");
    assert_eq!(spec.guarantee_class, vo_types::GuaranteeClass::default());
}
