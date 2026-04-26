//! Test Coverage: Valid WorkflowSpec acceptance and round-trip behavior.
//!
//! bead_id: ve-jm7n
//!
//! Tests that complete workflow specs round-trip correctly via serde.

use crate::dag::{Dag, Workflow};
use crate::{EdgeSpec, NodeSpec, WorkflowSpec};
use vo_types::{NodeKind, NodeName, WorkflowName};

#[test]
fn valid_minimal_spec_with_single_pure_node_round_trips() {
    let mut wf = Workflow::new("minimal");
    let _: crate::node_handle::NodeHandle<(), ()> = wf.pure("step", |_i: ()| ()).expect("valid");
    let spec = wf.build().expect("build succeeds");

    let json = serde_json::to_string(&spec).expect("serialize");
    let restored: WorkflowSpec = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.workflow_name, spec.workflow_name);
    assert_eq!(restored.nodes.len(), 1);
    assert_eq!(restored.nodes[0].kind, NodeKind::Pure);
    assert!(restored.edges.is_empty());
}

#[test]
fn valid_spec_with_all_node_kinds_in_sequence_round_trips() {
    let mut wf = Workflow::new("all-kinds-chain");
    let a = wf.pure("pure-step", |_i: ()| ()).expect("valid");
    let b = wf.effect("effect-step", |_i: ()| ()).expect("valid");
    let c = wf.wait("wait-step", |_i: ()| ()).expect("valid");
    let d = wf.signal("signal-step", |_i: ()| ()).expect("valid");
    let e = wf.unsafe_node("unsafe-step", |_i: ()| ()).expect("valid");
    wf.connect(&a, &b).expect("a->b");
    wf.connect(&b, &c).expect("b->c");
    wf.connect(&c, &d).expect("c->d");
    wf.connect(&d, &e).expect("d->e");

    let spec = wf.build().expect("build succeeds");
    assert_eq!(spec.nodes.len(), 5);
    assert_eq!(spec.edges.len(), 4);

    let kinds: Vec<NodeKind> = spec.nodes.iter().map(|n| n.kind).collect();
    assert_eq!(kinds[0], NodeKind::Pure);
    assert_eq!(kinds[1], NodeKind::ManagedEffect);
    assert_eq!(kinds[2], NodeKind::Wait);
    assert_eq!(kinds[3], NodeKind::Signal);
    assert_eq!(kinds[4], NodeKind::Unsafe);
}

#[test]
fn valid_spec_with_fan_out_fan_in_pattern() {
    let mut wf = Workflow::new("fan-pattern");
    let source = wf.pure("source", |_i: ()| ()).expect("valid");
    let branch_a = wf.effect("branch-a", |_i: ()| ()).expect("valid");
    let branch_b = wf.effect("branch-b", |_i: ()| ()).expect("valid");
    let sink = wf.unsafe_node("sink", |_i: ()| ()).expect("valid");
    wf.connect(&source, &branch_a).expect("s->a");
    wf.connect(&source, &branch_b).expect("s->b");
    wf.connect(&branch_a, &sink).expect("a->sink");
    wf.connect(&branch_b, &sink).expect("b->sink");

    let spec = wf.build().expect("build succeeds");
    assert_eq!(spec.nodes.len(), 4);
    assert_eq!(spec.edges.len(), 4);

    let json = serde_json::to_string(&spec).expect("serialize");
    let restored: WorkflowSpec = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored, spec);
}

#[test]
fn valid_spec_preserves_node_ordering() {
    let mut wf = Workflow::new("ordered");
    let _a = wf.signal("z-signal", |_i: ()| ()).expect("valid");
    let _b = wf.pure("a-pure", |_i: ()| ()).expect("valid");
    let _c = wf.effect("m-effect", |_i: ()| ()).expect("valid");

    let spec = wf.build().expect("build");
    assert_eq!(spec.nodes[0].name.as_str(), "z-signal");
    assert_eq!(spec.nodes[1].name.as_str(), "a-pure");
    assert_eq!(spec.nodes[2].name.as_str(), "m-effect");
}

#[test]
fn valid_spec_preserves_edge_ordering() {
    let mut wf = Workflow::new("edge-order");
    let a = wf.pure("a", |_i: ()| ()).expect("valid");
    let b = wf.pure("b", |_i: ()| ()).expect("valid");
    let c = wf.pure("c", |_i: ()| ()).expect("valid");
    wf.connect(&a, &b).expect("a->b");
    wf.connect(&a, &c).expect("a->c");

    let spec = wf.build().expect("build");
    assert_eq!(spec.edges[0].from.as_str(), "a");
    assert_eq!(spec.edges[0].to.as_str(), "b");
    assert_eq!(spec.edges[1].from.as_str(), "a");
    assert_eq!(spec.edges[1].to.as_str(), "c");
}

#[test]
fn valid_spec_with_hyphenated_names_round_trips() {
    let mut wf = Workflow::new("my-workflow");
    let a = wf.pure("step-one", |_i: ()| ()).expect("valid");
    let b = wf.effect("step-two", |_i: ()| ()).expect("valid");
    wf.connect(&a, &b).expect("connect");

    let spec = wf.build().expect("build");
    let json = serde_json::to_string(&spec).expect("serialize");
    let restored: WorkflowSpec = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.workflow_name.as_str(), "my-workflow");
    assert_eq!(restored.nodes[0].name.as_str(), "step-one");
    assert_eq!(restored.nodes[1].name.as_str(), "step-two");
}
