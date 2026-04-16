use vo_sdk::dag::{Dag, DagError, Workflow};
use vo_sdk::graph::{EdgeSpec, NodeSpec, WorkflowSpec};
use vo_types::{NodeKind, NodeName, WorkflowName};

// --- Fluent builder API ---

#[test]
fn workflow_builder_pure_node() {
    let mut wf = Workflow::new("qa-pure");
    let h = wf.pure("step", |_: String| -> i32 { 0 }).unwrap();
    assert_eq!(h.name(), "step");
    let spec = wf.build().unwrap();
    assert_eq!(spec.nodes.len(), 1);
    assert_eq!(spec.nodes[0].kind, NodeKind::Pure);
}

#[test]
fn workflow_builder_all_node_kinds() {
    let mut wf = Workflow::new("qa-kinds");
    wf.pure("p", |_: ()| ()).unwrap();
    wf.effect("e", |_: ()| ()).unwrap();
    wf.wait("w", |_: ()| ()).unwrap();
    wf.signal("s", |_: ()| ()).unwrap();
    wf.unsafe_node("u", |_: ()| ()).unwrap();
    let spec = wf.build().unwrap();
    let kinds: Vec<_> = spec.nodes.iter().map(|n| n.kind).collect();
    assert_eq!(kinds, vec![NodeKind::Pure, NodeKind::ManagedEffect, NodeKind::Wait, NodeKind::Signal, NodeKind::Unsafe]);
}

#[test]
fn workflow_builder_rejects_invalid_name() {
    let mut wf = Workflow::new("qa-bad");
    let err = wf.pure("bad name", |_: ()| ()).unwrap_err();
    assert!(matches!(err, DagError::InvalidNodeName { .. }));
}

#[test]
fn workflow_builder_empty_rejected() {
    let wf = Workflow::new("qa-empty");
    let err = wf.build().unwrap_err();
    assert_eq!(err, DagError::EmptyWorkflow);
}

// --- DAG construction ---

#[test]
fn dag_connect_type_safe_chain() {
    let mut wf = Workflow::new("qa-chain");
    let a = wf.pure("a", |_: String| -> i32 { 0 }).unwrap();
    let b = wf.effect("b", |_: i32| -> bool { true }).unwrap();
    wf.connect(&a, &b).unwrap();
    let spec = wf.build().unwrap();
    assert_eq!(spec.edges.len(), 1);
    assert_eq!(spec.edges[0].from.as_str(), "a");
    assert_eq!(spec.edges[0].to.as_str(), "b");
}

#[test]
fn dag_connect_phantom_rejected() {
    let mut dag = Dag::new();
    let a: vo_sdk::node_handle::NodeHandle<(), ()> =
        dag.add_node_with_kind("a", NodeKind::Pure, |_: ()| ()).unwrap();
    let b: vo_sdk::node_handle::NodeHandle<(), ()> =
        dag.add_node_with_kind("b", NodeKind::Pure, |_: ()| ()).unwrap();
    let ghost: vo_sdk::node_handle::NodeHandle<(), ()> =
        vo_sdk::node_handle::NodeHandle::new(NodeName::parse("ghost").unwrap());
    assert!(matches!(dag.connect(&a, &ghost), Err(DagError::NodeNotFound { .. })));
    dag.connect(&a, &b).unwrap();
    assert_eq!(dag.edge_count(), 1);
}

#[test]
fn dag_cycle_detected() {
    let mut wf = Workflow::new("qa-cycle");
    let a = wf.pure("a", |_: ()| ()).unwrap();
    let b = wf.pure("b", |_: ()| ()).unwrap();
    wf.connect(&a, &b).unwrap();
    wf.connect(&b, &a).unwrap();
    let err = wf.build().unwrap_err();
    assert!(matches!(err, DagError::CycleDetected { .. }));
}

#[test]
fn dag_fan_out_edges() {
    let mut wf = Workflow::new("qa-fan");
    let src = wf.pure("src", |_: ()| -> i32 { 0 }).unwrap();
    let a = wf.pure("a", |_: i32| ()).unwrap();
    let b = wf.pure("b", |_: i32| ()).unwrap();
    wf.connect(&src, &a).unwrap();
    wf.connect(&src, &b).unwrap();
    let spec = wf.build().unwrap();
    assert_eq!(spec.edges.len(), 2);
}

// --- WorkflowSpec ---

#[test]
fn spec_direct_construction() {
    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("qa-spec").unwrap(),
        nodes: vec![
            NodeSpec { name: NodeName::parse("x").unwrap(), kind: NodeKind::Pure },
            NodeSpec { name: NodeName::parse("y").unwrap(), kind: NodeKind::ManagedEffect },
        ],
        edges: vec![EdgeSpec { from: NodeName::parse("x").unwrap(), to: NodeName::parse("y").unwrap() }],
    };
    assert_eq!(spec.workflow_name.as_str(), "qa-spec");
    assert_eq!(spec.nodes.len(), 2);
    assert_eq!(spec.edges.len(), 1);
}

#[test]
fn spec_json_roundtrip() {
    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("qa-json").unwrap(),
        nodes: vec![NodeSpec { name: NodeName::parse("solo").unwrap(), kind: NodeKind::Wait }],
        edges: vec![],
    };
    let json = spec.to_json_bytes();
    let back: WorkflowSpec = serde_json::from_slice(&json).unwrap();
    assert_eq!(spec, back);
}
