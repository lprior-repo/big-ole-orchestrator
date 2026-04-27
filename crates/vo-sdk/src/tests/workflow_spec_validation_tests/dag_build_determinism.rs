//! Section 6: Dag build determinism and structural integrity

use crate::dag::{Dag, DagError};
use crate::node_handle::NodeHandle;
use vo_types::NodeKind;

#[test]
fn dag_build_produces_deterministic_spec() {
    let build_spec = || {
        let mut wf = crate::dag::Workflow::new("determinism");
        let a = wf.pure("a", |_i: ()| ()).expect("valid");
        let b = wf.effect("b", |_i: ()| ()).expect("valid");
        let c = wf.wait("c", |_i: ()| ()).expect("valid");
        wf.connect(&a, &b).expect("a->b");
        wf.connect(&b, &c).expect("b->c");
        wf.build().expect("build")
    };

    let spec1 = build_spec();
    let spec2 = build_spec();

    let json1 = serde_json::to_string(&spec1).expect("serialize");
    let json2 = serde_json::to_string(&spec2).expect("serialize");
    assert_eq!(
        json1, json2,
        "identical workflows should produce identical JSON"
    );
}

#[test]
fn dag_node_count_matches_builder_calls() {
    let mut wf = crate::dag::Workflow::new("count-test");
    let _ = wf.pure("a", |_i: ()| ()).expect("valid");
    let _ = wf.effect("b", |_i: ()| ()).expect("valid");
    let _ = wf.wait("c", |_i: ()| ()).expect("valid");
    let _ = wf.signal("d", |_i: ()| ()).expect("valid");
    let _ = wf.unsafe_node("e", |_i: ()| ()).expect("valid");
    let spec = wf.build().expect("build");
    assert_eq!(spec.nodes.len(), 5);
}

#[test]
fn dag_edge_count_matches_connect_calls() {
    let mut wf = crate::dag::Workflow::new("edge-count");
    let a = wf.pure("a", |_i: ()| ()).expect("valid");
    let b = wf.effect("b", |_i: ()| ()).expect("valid");
    let c = wf.wait("c", |_i: ()| ()).expect("valid");
    wf.connect(&a, &b).expect("a->b");
    wf.connect(&b, &c).expect("b->c");
    let spec = wf.build().expect("build");
    assert_eq!(spec.edges.len(), 2);
}

#[test]
fn dag_build_rejects_three_node_cycle() {
    let mut dag = Dag::new();
    let a: NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let b: NodeHandle<(), ()> = dag
        .add_node_with_kind("b", NodeKind::ManagedEffect, |_: ()| ())
        .expect("valid");
    let c: NodeHandle<(), ()> = dag
        .add_node_with_kind("c", NodeKind::Wait, |_: ()| ())
        .expect("valid");
    dag.connect(&a, &b).expect("a->b");
    dag.connect(&b, &c).expect("b->c");
    dag.connect(&c, &a).expect("c->a");
    let result = dag.build("triangle-cycle");
    assert!(
        matches!(result, Err(DagError::CycleDetected { .. })),
        "3-node cycle should be detected"
    );
}

#[test]
fn dag_build_rejects_cycle_with_mixed_node_kinds() {
    let mut dag = Dag::new();
    let a: NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let b: NodeHandle<(), ()> = dag
        .add_node_with_kind("b", NodeKind::ManagedEffect, |_: ()| ())
        .expect("valid");
    dag.connect(&a, &b).expect("a->b");
    dag.connect(&b, &a).expect("b->a");
    let result = dag.build("mixed-cycle");
    assert!(
        matches!(result, Err(DagError::CycleDetected { .. })),
        "cycle with mixed kinds should be detected"
    );
}

#[test]
fn dag_default_is_empty() {
    let dag = Dag::new();
    assert_eq!(dag.node_count(), 0);
    assert_eq!(dag.edge_count(), 0);
}

#[test]
fn dag_edges_returns_names() {
    let mut dag = Dag::new();
    let a: NodeHandle<(), ()> = dag
        .add_node_with_kind("alpha", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let b: NodeHandle<(), ()> = dag
        .add_node_with_kind("beta", NodeKind::ManagedEffect, |_: ()| ())
        .expect("valid");
    dag.connect(&a, &b).expect("connect");
    let edges = dag.edges();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0], ("alpha", "beta"));
}