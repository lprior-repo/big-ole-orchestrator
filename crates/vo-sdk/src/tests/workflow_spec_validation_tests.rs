//! Test Coverage: WorkflowSpec validation and discovery (ADR-003/017/022/031).
//!
//! bead_id: ve-jm7n
//!
//! Coverage areas:
//!   1. Valid specs accepted — complete workflow specs round-trip correctly
//!   2. Invalid node mixes rejected — node kind semantic constraint enforcement
//!   3. Version pinning enforced — schema version validation at both type layers
//!   4. Discovery validation — version compatibility, schema evolution, upgrade paths
//!   5. Node kind constraints — each kind's specific behavioral contracts

use crate::dag::{Dag, DagError, Workflow};
use crate::graph_args::{EdgeSpec, NodeSpec, WorkflowSpec};
use vo_types::{NodeKind, NodeName, WorkflowName};

// ===========================================================================
// SECTION 1: Valid WorkflowSpec acceptance
// ===========================================================================

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

// ===========================================================================
// SECTION 2: Invalid node mixes rejected
// ===========================================================================

#[test]
fn dag_accepts_node_with_digit_prefix_as_valid() {
    let mut dag = Dag::new();
    let result: Result<crate::node_handle::NodeHandle<(), ()>, _> =
        dag.add_node_with_kind("1valid-per-grammar", NodeKind::Pure, |_: ()| ());
    assert!(
        result.is_ok(),
        "node name starting with digit is accepted by current grammar"
    );
}

#[test]
fn dag_rejects_node_with_spaces_in_name() {
    let mut dag = Dag::new();
    let result: Result<crate::node_handle::NodeHandle<(), ()>, _> =
        dag.add_node_with_kind("bad name", NodeKind::Pure, |_: ()| ());
    assert!(
        matches!(result, Err(DagError::InvalidNodeName { .. })),
        "node name with spaces should be rejected"
    );
}

#[test]
fn dag_accepts_node_with_uppercase_in_name_as_valid() {
    let mut dag = Dag::new();
    let result: Result<crate::node_handle::NodeHandle<(), ()>, _> =
        dag.add_node_with_kind("BadName", NodeKind::Pure, |_: ()| ());
    assert!(
        result.is_ok(),
        "uppercase in node name is accepted by current grammar"
    );
}

#[test]
fn dag_rejects_connect_to_nonexistent_node() {
    let mut dag = Dag::new();
    let a: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let ghost: crate::node_handle::NodeHandle<(), ()> =
        crate::node_handle::NodeHandle::new(NodeName::parse("ghost").expect("valid name"));
    let result = dag.connect(&a, &ghost);
    assert!(
        matches!(result, Err(DagError::NodeNotFound { .. })),
        "connecting to nonexistent node should fail"
    );
}

#[test]
fn dag_rejects_connect_from_nonexistent_node() {
    let mut dag = Dag::new();
    let b: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("b", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let ghost: crate::node_handle::NodeHandle<(), ()> =
        crate::node_handle::NodeHandle::new(NodeName::parse("ghost").expect("valid name"));
    let result = dag.connect(&ghost, &b);
    assert!(
        matches!(result, Err(DagError::NodeNotFound { .. })),
        "connecting from nonexistent node should fail"
    );
}

#[test]
fn dag_rejects_build_with_no_nodes() {
    let dag = Dag::new();
    let result = dag.build("empty");
    assert!(
        matches!(result, Err(DagError::EmptyWorkflow)),
        "empty dag should be rejected"
    );
}

#[test]
fn dag_accepts_workflow_name_with_caps_as_valid() {
    let mut dag = Dag::new();
    let _: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let result = dag.build("MyWorkflow");
    assert!(
        result.is_ok(),
        "uppercase workflow name is accepted by current grammar"
    );
}

#[test]
fn dag_rejects_build_with_empty_workflow_name() {
    let mut dag = Dag::new();
    let _: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let result = dag.build("");
    assert!(
        matches!(result, Err(DagError::InvalidNodeName { .. })),
        "empty workflow name should be rejected"
    );
}

#[test]
fn serde_rejects_spec_with_null_node_in_nodes_array() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [null],
        "edges": []
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err(), "null node in array should be rejected");
}

#[test]
fn serde_rejects_spec_with_number_as_node() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [42],
        "edges": []
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err(), "number as node should be rejected");
}

// ===========================================================================
// SECTION 3: Version pinning enforced
// ===========================================================================

#[test]
fn vo_types_workflow_spec_accepts_current_version() {
    let payload = serde_json::json!({"version": 1});
    let spec: vo_types::WorkflowSpec = serde_json::from_value(payload).expect("version 1 accepted");
    assert_eq!(spec.version(), 1);
}

#[test]
fn vo_types_workflow_spec_rejects_future_version() {
    let payload = serde_json::json!({"version": 2});
    let result: Result<vo_types::WorkflowSpec, _> = serde_json::from_value(payload);
    assert!(result.is_err(), "future version should be rejected");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("Unsupported") || err.contains("unsupported"),
        "error should mention unsupported: {err}"
    );
}

#[test]
fn vo_types_workflow_spec_rejects_zero_version_when_missing_fallback() {
    let payload = serde_json::json!({});
    let result: Result<vo_types::WorkflowSpec, _> = serde_json::from_value(payload);
    assert!(
        result.is_err(),
        "missing version without fallback should be rejected"
    );
}

#[test]
fn vo_types_workflow_spec_default_uses_max_supported() {
    let spec = vo_types::WorkflowSpec::default();
    assert_eq!(
        spec.version(),
        vo_types::MAX_SUPPORTED_SCHEMA_VERSION,
        "default should use MAX_SUPPORTED_SCHEMA_VERSION"
    );
}

#[test]
fn vo_types_workflow_spec_round_trips_version() {
    let spec = vo_types::WorkflowSpec { version: 1 };
    let json = serde_json::to_string(&spec).expect("serialize");
    let restored: vo_types::WorkflowSpec = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.version(), 1);
}

#[test]
fn sdk_workflow_spec_has_no_version_field() {
    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("test").expect("valid"),
        nodes: vec![],
        edges: vec![],
    };
    let json = serde_json::to_string(&spec).expect("serialize");
    assert!(
        !json.contains("version"),
        "SDK WorkflowSpec should not have version field: {json}"
    );
}

#[test]
fn sdk_workflow_spec_schema_is_stable_across_round_trips() {
    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("stability-test").expect("valid"),
        nodes: vec![
            NodeSpec {
                name: NodeName::parse("a").expect("valid"),
                kind: NodeKind::Pure,
            },
            NodeSpec {
                name: NodeName::parse("b").expect("valid"),
                kind: NodeKind::ManagedEffect,
            },
        ],
        edges: vec![EdgeSpec {
            from: NodeName::parse("a").expect("valid"),
            to: NodeName::parse("b").expect("valid"),
        }],
    };

    let json1 = serde_json::to_string(&spec).expect("serialize");
    let json2 = serde_json::to_string(&spec).expect("serialize");
    assert_eq!(json1, json2, "serialization should be deterministic");

    let restored: WorkflowSpec = serde_json::from_str(&json1).expect("deserialize");
    let json3 = serde_json::to_string(&restored).expect("re-serialize");
    assert_eq!(json1, json3, "round-trip should be bit-identical");
}

// ===========================================================================
// SECTION 4: Discovery validation — version compatibility, upgrade paths
// ===========================================================================

#[test]
fn schema_version_zero_is_accepted_for_backward_compat() {
    let payload = serde_json::json!({"version": 0});
    let spec: vo_types::WorkflowSpec =
        serde_json::from_value(payload).expect("version 0 accepted for backward compat");
    assert_eq!(spec.version(), 0);
}

#[test]
fn schema_version_rejects_string_version() {
    let payload = serde_json::json!({"version": "1"});
    let result: Result<vo_types::WorkflowSpec, _> = serde_json::from_value(payload);
    assert!(
        result.is_err(),
        "string version should be rejected as invalid format"
    );
}

#[test]
fn schema_version_rejects_float_version() {
    let payload = serde_json::json!({"version": 1.5});
    let result: Result<vo_types::WorkflowSpec, _> = serde_json::from_value(payload);
    assert!(
        result.is_err(),
        "float version should be rejected as invalid format"
    );
}

#[test]
fn schema_version_rejects_negative_version() {
    let payload = serde_json::json!({"version": -1});
    let result: Result<vo_types::WorkflowSpec, _> = serde_json::from_value(payload);
    assert!(
        result.is_err(),
        "negative version should be rejected as invalid format"
    );
}

#[test]
fn schema_version_rejects_null_version() {
    let payload = serde_json::json!({"version": null});
    let result: Result<vo_types::WorkflowSpec, _> = serde_json::from_value(payload);
    assert!(
        result.is_err(),
        "null version should be rejected as invalid format"
    );
}

#[test]
fn schema_version_rejects_boolean_version() {
    let payload = serde_json::json!({"version": true});
    let result: Result<vo_types::WorkflowSpec, _> = serde_json::from_value(payload);
    assert!(
        result.is_err(),
        "boolean version should be rejected as invalid format"
    );
}

#[test]
fn schema_version_rejects_u16_overflow() {
    let payload = serde_json::json!({"version": 65536});
    let result: Result<vo_types::WorkflowSpec, _> = serde_json::from_value(payload);
    assert!(
        result.is_err(),
        "version exceeding u16 range should be rejected"
    );
}

#[test]
fn schema_version_rejects_large_future_version() {
    let payload = serde_json::json!({"version": 999});
    let result: Result<vo_types::WorkflowSpec, _> = serde_json::from_value(payload);
    assert!(result.is_err(), "large future version should be rejected");
}

#[test]
fn upgrade_path_version_0_to_1_is_valid() {
    let v0_payload = serde_json::json!({"version": 0});
    let v1_payload = serde_json::json!({"version": 1});

    let spec_v0: vo_types::WorkflowSpec =
        serde_json::from_value(v0_payload).expect("version 0 accepted");
    let spec_v1: vo_types::WorkflowSpec =
        serde_json::from_value(v1_payload).expect("version 1 accepted");

    assert!(spec_v1.version() >= spec_v0.version());
}

#[test]
fn schema_evolution_extra_fields_are_ignored() {
    let payload = serde_json::json!({
        "version": 1,
        "future_field": "some value",
        "another_new_field": 42,
        "nested": {"deep": true}
    });
    let spec: vo_types::WorkflowSpec =
        serde_json::from_value(payload).expect("extra fields should be ignored");
    assert_eq!(spec.version(), 1);
}

#[test]
fn discovery_sdk_spec_with_extra_fields_ignored() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [{"name": "a", "kind": "pure"}],
        "edges": [],
        "discovery_metadata": {"version": 2}
    }"#;
    let spec: WorkflowSpec = serde_json::from_str(json).expect("extra discovery fields ignored");
    assert_eq!(spec.nodes.len(), 1);
}

// ===========================================================================
// SECTION 5: Node kind constraint enforcement
// ===========================================================================

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
            }],
            edges: vec![],
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

// ===========================================================================
// SECTION 6: Dag build determinism and structural integrity
// ===========================================================================

#[test]
fn dag_build_produces_deterministic_spec() {
    let build_spec = || {
        let mut wf = Workflow::new("determinism");
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
    let mut wf = Workflow::new("count-test");
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
    let mut wf = Workflow::new("edge-count");
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
    let a: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let b: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("b", NodeKind::ManagedEffect, |_: ()| ())
        .expect("valid");
    let c: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("c", NodeKind::Wait, |_: ()| ())
        .expect("valid");
    dag.connect(&a, &b).expect("a->b");
    dag.connect(&b, &c).expect("b->c");
    dag.connect(&c, &a).expect("c->a");
    let result = dag.build("triangle-cycle");
    assert!(
        matches!(result, Err(DagError::CycleDetected)),
        "3-node cycle should be detected"
    );
}

#[test]
fn dag_build_rejects_cycle_with_mixed_node_kinds() {
    let mut dag = Dag::new();
    let a: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let b: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("b", NodeKind::ManagedEffect, |_: ()| ())
        .expect("valid");
    dag.connect(&a, &b).expect("a->b");
    dag.connect(&b, &a).expect("b->a");
    let result = dag.build("mixed-cycle");
    assert!(
        matches!(result, Err(DagError::CycleDetected)),
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
    let a: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("alpha", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let b: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("beta", NodeKind::ManagedEffect, |_: ()| ())
        .expect("valid");
    dag.connect(&a, &b).expect("connect");
    let edges = dag.edges();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0], ("alpha", "beta"));
}

// ===========================================================================
// SECTION 7: Edge spec and Node spec serde integrity
// ===========================================================================

#[test]
fn node_spec_round_trips_all_kinds() {
    for kind in NodeKind::all_variants() {
        let node = NodeSpec {
            name: NodeName::parse("test-node").expect("valid"),
            kind: *kind,
        };
        let json = serde_json::to_string(&node).expect("serialize");
        let restored: NodeSpec = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.kind, *kind, "round-trip failed for {:?}", kind);
        assert_eq!(restored.name, node.name);
    }
}

#[test]
fn edge_spec_round_trips() {
    let edge = EdgeSpec {
        from: NodeName::parse("source").expect("valid"),
        to: NodeName::parse("target").expect("valid"),
    };
    let json = serde_json::to_string(&edge).expect("serialize");
    let restored: EdgeSpec = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored, edge);
}

#[test]
fn node_spec_equality_works() {
    let a = NodeSpec {
        name: NodeName::parse("a").expect("valid"),
        kind: NodeKind::Pure,
    };
    let b = NodeSpec {
        name: NodeName::parse("a").expect("valid"),
        kind: NodeKind::Pure,
    };
    let c = NodeSpec {
        name: NodeName::parse("a").expect("valid"),
        kind: NodeKind::ManagedEffect,
    };
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn edge_spec_equality_works() {
    let a = EdgeSpec {
        from: NodeName::parse("x").expect("valid"),
        to: NodeName::parse("y").expect("valid"),
    };
    let b = EdgeSpec {
        from: NodeName::parse("x").expect("valid"),
        to: NodeName::parse("y").expect("valid"),
    };
    let c = EdgeSpec {
        from: NodeName::parse("y").expect("valid"),
        to: NodeName::parse("x").expect("valid"),
    };
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn to_json_bytes_produces_deterministic_output() {
    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("json-test").expect("valid"),
        nodes: vec![NodeSpec {
            name: NodeName::parse("a").expect("valid"),
            kind: NodeKind::Pure,
        }],
        edges: vec![],
    };
    let bytes1 = spec.to_json_bytes();
    let bytes2 = spec.to_json_bytes();
    assert_eq!(bytes1, bytes2, "to_json_bytes should be deterministic");
}

#[test]
fn workflow_spec_clone_is_equal() {
    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("clone-test").expect("valid"),
        nodes: vec![NodeSpec {
            name: NodeName::parse("a").expect("valid"),
            kind: NodeKind::Pure,
        }],
        edges: vec![EdgeSpec {
            from: NodeName::parse("a").expect("valid"),
            to: NodeName::parse("a").expect("valid"),
        }],
    };
    let cloned = spec.clone();
    assert_eq!(spec, cloned);
}

#[test]
fn workflow_spec_debug_format_includes_fields() {
    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("debug-test").expect("valid"),
        nodes: vec![],
        edges: vec![],
    };
    let debug = format!("{:?}", spec);
    assert!(
        debug.contains("debug-test"),
        "debug format should contain workflow name"
    );
}
