//! Section 5: Node kind constraint enforcement

use crate::dag::Workflow;
use crate::{DedupeScope, EdgeSpec, NodeSpec, WorkflowSpec};
use vo_types::{NodeKind, NodeName, WorkflowName};

#[test]
fn node_kind_pure_is_deterministic_tag() {
    let mut wf = Workflow::new("pure-test");
    let _: crate::node_handle::NodeHandle<(), ()> =
        wf.pure("pure-step", |_i: ()| ()).expect("valid");
    let spec = wf.build().expect("build");
    assert_eq!(spec.nodes[0].kind, NodeKind::Pure);

    let json = serde_json::to_string(&spec).expect("serialize");
    assert!(
        json.contains("\"kind\":\"pure\""),
        "pure should serialize as snake_case: {json}"
    );
}

#[test]
fn node_kind_managed_effect_is_tracked_tag() {
    let mut wf = Workflow::new("effect-test");
    let _: crate::node_handle::NodeHandle<(), ()> =
        wf.effect("effect-step", |_i: ()| ()).expect("valid");
    let spec = wf.build().expect("build");
    assert_eq!(spec.nodes[0].kind, NodeKind::ManagedEffect);

    let json = serde_json::to_string(&spec).expect("serialize");
    assert!(
        json.contains("\"kind\":\"managed_effect\""),
        "managed_effect should serialize as snake_case: {json}"
    );
}

#[test]
fn node_kind_wait_is_hibernate_tag() {
    let mut wf = Workflow::new("wait-test");
    let _: crate::node_handle::NodeHandle<(), ()> =
        wf.wait("wait-step", |_i: ()| ()).expect("valid");
    let spec = wf.build().expect("build");
    assert_eq!(spec.nodes[0].kind, NodeKind::Wait);

    let json = serde_json::to_string(&spec).expect("serialize");
    assert!(
        json.contains("\"kind\":\"wait\""),
        "wait should serialize as snake_case: {json}"
    );
}

#[test]
fn node_kind_signal_is_delivery_tag() {
    let mut wf = Workflow::new("signal-test");
    let _: crate::node_handle::NodeHandle<(), ()> =
        wf.signal("signal-step", |_i: ()| ()).expect("valid");
    let spec = wf.build().expect("build");
    assert_eq!(spec.nodes[0].kind, NodeKind::Signal);

    let json = serde_json::to_string(&spec).expect("serialize");
    assert!(
        json.contains("\"kind\":\"signal\""),
        "signal should serialize as snake_case: {json}"
    );
}

#[test]
fn node_kind_unsafe_is_escape_hatch_tag() {
    let mut wf = Workflow::new("unsafe-test");
    let _: crate::node_handle::NodeHandle<(), ()> =
        wf.unsafe_node("unsafe-step", |_i: ()| ()).expect("valid");
    let spec = wf.build().expect("build");
    assert_eq!(spec.nodes[0].kind, NodeKind::Unsafe);

    let json = serde_json::to_string(&spec).expect("serialize");
    assert!(
        json.contains("\"kind\":\"unsafe\""),
        "unsafe should serialize as snake_case: {json}"
    );
}

#[test]
fn all_node_kinds_survive_serde_round_trip_individually() {
    for kind in NodeKind::all_variants() {
        let spec = WorkflowSpec {
            workflow_name: WorkflowName::parse("kind-test").expect("valid"),
            nodes: vec![NodeSpec {
                name: NodeName::parse("node-a").expect("valid"),
                kind: *kind,
                retry_policy: None,
                signal_scope: None,
            }],
            edges: vec![],
            dedupe_scope: DedupeScope::default(),
        };
        let json = serde_json::to_string(&spec).expect("serialize");
        let restored: WorkflowSpec = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            restored.nodes[0].kind, *kind,
            "round-trip failed for {:?}",
            kind
        );
    }
}

#[test]
fn node_kind_rejects_unknown_variant_via_serde() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [{"name": "a", "kind": "unknown_kind"}],
        "edges": []
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err(), "unknown node kind should be rejected");
}

#[test]
fn node_kind_rejects_camel_case_via_serde() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [{"name": "a", "kind": "managedEffect"}],
        "edges": []
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "camelCase node kind should be rejected (expects snake_case)"
    );
}

#[test]
fn node_kind_rejects_uppercase_via_serde() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [{"name": "a", "kind": "PURE"}],
        "edges": []
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "UPPERCASE node kind should be rejected (expects snake_case)"
    );
}

#[test]
fn node_kind_rejects_integer_via_serde() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [{"name": "a", "kind": 0}],
        "edges": []
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err(), "integer node kind should be rejected");
}

#[test]
fn node_kind_pure_can_connect_to_managed_effect() {
    let mut wf = Workflow::new("cross-kind");
    let a = wf.pure("pure-node", |_i: ()| ()).expect("valid");
    let b = wf.effect("effect-node", |_i: ()| ()).expect("valid");
    wf.connect(&a, &b).expect("pure->effect should connect");
    let spec = wf.build().expect("build");
    assert_eq!(spec.edges.len(), 1);
}

#[test]
fn node_kind_managed_effect_can_connect_to_wait() {
    let mut wf = Workflow::new("effect-to-wait");
    let a = wf.effect("effect-node", |_i: ()| ()).expect("valid");
    let b = wf.wait("wait-node", |_i: ()| ()).expect("valid");
    wf.connect(&a, &b).expect("effect->wait should connect");
    let spec = wf.build().expect("build");
    assert_eq!(spec.edges.len(), 1);
}

#[test]
fn node_kind_wait_can_connect_to_signal() {
    let mut wf = Workflow::new("wait-to-signal");
    let a = wf.wait("wait-node", |_i: ()| ()).expect("valid");
    let b = wf.signal("signal-node", |_i: ()| ()).expect("valid");
    wf.connect(&a, &b).expect("wait->signal should connect");
    let spec = wf.build().expect("build");
    assert_eq!(spec.edges.len(), 1);
}

#[test]
fn node_kind_signal_can_connect_to_unsafe() {
    let mut wf = Workflow::new("signal-to-unsafe");
    let a = wf.signal("signal-node", |_i: ()| ()).expect("valid");
    let b = wf.unsafe_node("unsafe-node", |_i: ()| ()).expect("valid");
    wf.connect(&a, &b).expect("signal->unsafe should connect");
    let spec = wf.build().expect("build");
    assert_eq!(spec.edges.len(), 1);
}

#[test]
fn node_kind_unsafe_can_be_entry_point() {
    let mut wf = Workflow::new("unsafe-entry");
    let _entry = wf.unsafe_node("entry", |_i: ()| ()).expect("valid");
    let spec = wf.build().expect("build");
    assert_eq!(spec.nodes[0].kind, NodeKind::Unsafe);
}

#[test]
fn node_kind_wait_can_be_entry_point() {
    let mut wf = Workflow::new("wait-entry");
    let _entry = wf.wait("entry", |_i: ()| ()).expect("valid");
    let spec = wf.build().expect("build");
    assert_eq!(spec.nodes[0].kind, NodeKind::Wait);
}

#[test]
fn node_kind_signal_can_be_entry_point() {
    let mut wf = Workflow::new("signal-entry");
    let _entry = wf.signal("entry", |_i: ()| ()).expect("valid");
    let spec = wf.build().expect("build");
    assert_eq!(spec.nodes[0].kind, NodeKind::Signal);
}

#[test]
fn multiple_nodes_of_same_kind_allowed() {
    let mut wf = Workflow::new("multi-pure");
    let a = wf.pure("a", |_i: ()| ()).expect("valid");
    let b = wf.pure("b", |_i: ()| ()).expect("valid");
    let c = wf.pure("c", |_i: ()| ()).expect("valid");
    wf.connect(&a, &b).expect("a->b");
    wf.connect(&b, &c).expect("b->c");
    let spec = wf.build().expect("build");
    assert!(spec.nodes.iter().all(|n| n.kind == NodeKind::Pure));
}
