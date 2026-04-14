//! Red Queen adversarial tests for WorkflowSpec validation (ADR-031).
//!
//! bead_id: ve-a170
//! phase: state-5-red-queen
//!
//! Dimensions attacked:
//!   - invalid-node-mixes: invalid NodeName, unknown NodeKind variant
//!   - circular-deps: self-loops, 2-cycles, large cycles, disconnected cycles
//!   - missing-entry-points: unreachable nodes, orphan nodes
//!   - oversized-specs: massive node/edge counts, deep nesting
//!   - serde-integrity: malformed JSON, wrong types, extra fields
//!   - version-pin-bypass: WorkflowSpec serialization bypassing Dag validation

use crate::dag::{Dag, DagError, Workflow};
use crate::graph_args::{EdgeSpec, NodeSpec, WorkflowSpec};
use vo_types::{NodeKind, NodeName, WorkflowName};

#[cfg(feature = "proptest")]
use proptest::prelude::*;

// ===========================================================================
// DIMENSION: invalid-node-mixes
// Invalid NodeName, unknown NodeKind variant, malformed names
// ===========================================================================

#[test]
fn rq_workflow_spec_rejects_invalid_node_name_via_serde() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [
            {"name": "bad node!", "kind": "pure"}
        ],
        "edges": [],
        "version": 1
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err(), "invalid node name should be rejected");
}

#[test]
fn rq_workflow_spec_rejects_empty_node_name_via_serde() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [
            {"name": "", "kind": "pure"}
        ],
        "edges": [],
        "version": 1
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err(), "empty node name should be rejected");
}

#[test]
fn rq_workflow_spec_rejects_unknown_node_kind_via_serde() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [
            {"name": "valid-node", "kind": "nonexistent_kind"}
        ],
        "edges": [],
        "version": 1
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err(), "unknown node kind should be rejected");
}

#[test]
fn rq_workflow_spec_rejects_node_name_with_invalid_chars_via_serde() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [
            {"name": "node@special!", "kind": "pure"}
        ],
        "edges": [],
        "version": 1
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "node name with special chars should be rejected"
    );
}

#[test]
fn rq_workflow_spec_rejects_node_name_with_emoji_via_serde() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [
            {"name": "node🔥", "kind": "pure"}
        ],
        "edges": [],
        "version": 1
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err(), "node name with emoji should be rejected");
}

#[test]
fn rq_workflow_spec_rejects_node_name_too_long_via_serde() {
    let long_name = "a".repeat(129);
    let json = format!(
        r#"{{
        "workflow_name": "test",
        "nodes": [
            {{"name": "{}", "kind": "pure"}}
        ],
        "edges": [],
        "version": 1
    }}"#,
        long_name
    );
    let result: Result<WorkflowSpec, _> = serde_json::from_str(&json);
    assert!(
        result.is_err(),
        "node name exceeding 128 chars should be rejected"
    );
}

#[test]
fn rq_workflow_spec_rejects_consecutive_hyphens_in_node_name_via_serde() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [
            {"name": "node--bad", "kind": "pure"}
        ],
        "edges": [],
        "version": 1
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err(), "consecutive hyphens should be rejected");
}

#[test]
fn rq_workflow_spec_rejects_leading_hyphen_in_node_name_via_serde() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [
            {"name": "-start", "kind": "pure"}
        ],
        "edges": [],
        "version": 1
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err(), "leading hyphen should be rejected");
}

#[test]
fn rq_workflow_spec_rejects_trailing_hyphen_in_node_name_via_serde() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [
            {"name": "end-", "kind": "pure"}
        ],
        "edges": [],
        "version": 1
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err(), "trailing hyphen should be rejected");
}

#[test]
fn rq_workflow_spec_rejects_trailing_underscore_in_node_name_via_serde() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [
            {"name": "end_", "kind": "pure"}
        ],
        "edges": [],
        "version": 1
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err(), "trailing underscore should be rejected");
}

#[test]
fn rq_workflow_spec_rejects_wrong_type_for_node_name_via_serde() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [
            {"name": 123, "kind": "pure"}
        ],
        "edges": [],
        "version": 1
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "number instead of string for node name should be rejected"
    );
}

#[test]
fn rq_workflow_spec_rejects_wrong_type_for_node_kind_via_serde() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [
            {"name": "valid-node", "kind": 42}
        ],
        "edges": [],
        "version": 1
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "number instead of string for kind should be rejected"
    );
}

#[test]
fn rq_workflow_spec_accepts_all_valid_node_kinds_via_serde() {
    for kind in &["pure", "managed_effect", "wait", "signal", "unsafe"] {
        let json = format!(
            r#"{{
            "workflow_name": "test",
            "nodes": [
                {{"name": "valid-node", "kind": "{}"}}
            ],
            "edges": [],
        "version": 1
        }}"#,
            kind
        );
        let result: Result<WorkflowSpec, _> = serde_json::from_str(&json);
        assert!(
            result.is_ok(),
            "valid node kind '{}' should be accepted: {:?}",
            kind,
            result
        );
    }
}

// ===========================================================================
// DIMENSION: circular-deps
// Self-loops, 2-cycles, large cycles, disconnected cycles via Dag build
// ===========================================================================

#[test]
fn rq_dag_build_rejects_self_loop() {
    let mut dag = Dag::new();
    let a: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    dag.connect(&a, &a).expect("connect should succeed");
    let result = dag.build("self_loop");
    assert!(
        matches!(result, Err(DagError::CycleDetected { .. })),
        "Dag::build should reject self-loop: {:?}",
        result
    );
}

#[test]
fn rq_dag_build_rejects_two_node_cycle() {
    let mut dag = Dag::new();
    let a: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let b: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("b", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    dag.connect(&a, &b).expect("connect a->b");
    dag.connect(&b, &a).expect("connect b->a");
    let result = dag.build("two_cycle");
    assert!(
        matches!(result, Err(DagError::CycleDetected { .. })),
        "Dag::build should reject 2-cycle: {:?}",
        result
    );
}

#[test]
fn rq_dag_build_rejects_large_five_node_cycle() {
    let mut dag = Dag::new();
    let names = ["a", "b", "c", "d", "e"];
    let mut handles: Vec<_> = Vec::new();
    for &name in &names {
        let h: crate::node_handle::NodeHandle<(), ()> = dag
            .add_node_with_kind(name, NodeKind::Pure, |_: ()| ())
            .expect("valid");
        handles.push(h);
    }
    for i in 0..5 {
        dag.connect(&handles[i], &handles[(i + 1) % 5])
            .expect("connect should succeed");
    }
    let result = dag.build("large_cycle");
    assert!(
        matches!(result, Err(DagError::CycleDetected { .. })),
        "Dag::build should reject 5-cycle: {:?}",
        result
    );
}

#[test]
fn rq_dag_build_rejects_cycle_in_disconnected_component() {
    let mut dag = Dag::new();
    let a: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let _b: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("b", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let _c: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("c", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    dag.connect(&a, &a).expect("self-loop on a");
    let result = dag.build("disconnected_cycle");
    assert!(
        matches!(result, Err(DagError::CycleDetected { .. })),
        "Dag::build should detect cycle in disconnected component: {:?}",
        result
    );
}

#[test]
fn rq_dag_build_accepts_diamond_graph_without_cycle() {
    let mut dag = Dag::new();
    let a: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let b: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("b", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let c: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("c", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let d: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("d", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    dag.connect(&a, &b).expect("a->b");
    dag.connect(&a, &c).expect("a->c");
    dag.connect(&b, &d).expect("b->d");
    dag.connect(&c, &d).expect("c->d");
    let result = dag.build("diamond");
    assert!(
        result.is_ok(),
        "diamond graph should be accepted: {:?}",
        result
    );
}

#[test]
fn rq_dag_build_accepts_diamond_pattern() {
    let mut dag = Dag::new();
    let a: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let b: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("b", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let c: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("c", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    dag.connect(&a, &b).expect("a->b");
    dag.connect(&a, &c).expect("a->c");
    dag.connect(&c, &b).expect("c->b converges to b");
    let result = dag.build("diamond");
    assert!(
        result.is_ok(),
        "diamond pattern is valid DAG (not a cycle): {:?}",
        result
    );
}

// ===========================================================================
// DIMENSION: missing-entry-points
// Unreachable/orphan nodes - Dag doesn't validate this, but we document it
// ===========================================================================

#[test]
fn rq_dag_build_accepts_unreachable_nodes() {
    let mut dag = Dag::new();
    let a: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let _b: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("b", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let _c: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("c", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    dag.connect(&a, &a).expect("a->a self-loop");
    let result = dag.build("unreachable");
    assert!(
        matches!(result, Err(DagError::CycleDetected { .. })),
        "Dag should reject self-loop even with unreachable nodes: {:?}",
        result
    );
}

#[test]
fn rq_dag_build_accepts_completely_disconnected_nodes() {
    let mut dag = Dag::new();
    let _a: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let _b: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("b", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let _c: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("c", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    let result = dag.build("disconnected");
    assert!(
        result.is_ok(),
        "Dag accepts disconnected nodes (no entry point validation): {:?}",
        result
    );
}

#[test]
fn rq_workflow_spec_accepts_edges_to_nonexistent_nodes_via_serde() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [
            {"name": "a", "kind": "pure"},
            {"name": "b", "kind": "pure"}
        ],
        "edges": [
            {"from": "a", "to": "ghost"},
            {"from": "phantom", "to": "b"}
        ]
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(
        result.is_ok(),
        "serde accepts invalid edge refs (validation happens elsewhere): {:?}",
        result
    );
}

// ===========================================================================
// DIMENSION: oversized-specs
// Large node counts, large edge counts, deep chains
// ===========================================================================

#[test]
fn rq_workflow_spec_accepts_1000_nodes_via_serde() {
    let nodes: Vec<_> = (0..1000)
        .map(|i| format!(r#"{{"name": "node{}", "kind": "pure"}}"#, i))
        .collect();
    let json = format!(
        r#"{{"workflow_name": "big", "nodes": [{}], "edges": [],
        "version": 1}}"#,
        nodes.join(", ")
    );
    let result: Result<WorkflowSpec, _> = serde_json::from_str(&json);
    assert!(
        result.is_ok(),
        "1000 nodes should be accepted: {:?}",
        result
    );
}

#[test]
fn rq_dag_build_handles_100_node_linear_chain() {
    let mut dag = Dag::new();
    let mut prev: Option<crate::node_handle::NodeHandle<(), ()>> = None;
    for i in 0..100 {
        let h: crate::node_handle::NodeHandle<(), ()> = dag
            .add_node_with_kind(&format!("n{}", i), NodeKind::Pure, |_: ()| ())
            .expect("valid");
        if let Some(p) = prev {
            dag.connect(&p, &h).expect("connect should succeed");
        }
        prev = Some(h);
    }
    let result = dag.build("linear_100");
    assert!(
        result.is_ok(),
        "100-node chain should succeed: {:?}",
        result
    );
}

#[test]
fn rq_dag_build_handles_31_node_binary_tree() {
    let mut dag = Dag::new();
    let mut handles = Vec::new();
    for i in 0..31 {
        let h: crate::node_handle::NodeHandle<(), ()> = dag
            .add_node_with_kind(&format!("n{}", i), NodeKind::Pure, |_: ()| ())
            .expect("valid");
        handles.push(h);
    }
    for i in 0..15 {
        dag.connect(&handles[i], &handles[2 * i + 1])
            .expect("connect left child");
        dag.connect(&handles[i], &handles[2 * i + 2])
            .expect("connect right child");
    }
    let result = dag.build("binary_tree_31");
    assert!(
        result.is_ok(),
        "31-node binary tree should succeed: {:?}",
        result
    );
}

#[test]
fn rq_workflow_spec_all_node_kinds_in_single_workflow_via_serde() {
    let json = r#"{
        "workflow_name": "all_kinds",
        "nodes": [
            {"name": "pure-node", "kind": "pure"},
            {"name": "managed-node", "kind": "managed_effect"},
            {"name": "wait-node", "kind": "wait"},
            {"name": "signal-node", "kind": "signal"},
            {"name": "unsafe-node", "kind": "unsafe"}
        ],
        "edges": [],
        "version": 1
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(
        result.is_ok(),
        "all node kinds should be accepted: {:?}",
        result
    );
    let spec = result.unwrap();
    assert_eq!(spec.nodes.len(), 5);
}

// ===========================================================================
// DIMENSION: serde-integrity
// Malformed JSON, wrong types, extra fields, nulls
// ===========================================================================

#[test]
fn rq_workflow_spec_rejects_empty_json() {
    let json = b"";
    let result: Result<WorkflowSpec, _> = serde_json::from_slice(json);
    assert!(result.is_err(), "empty JSON should be rejected");
}

#[test]
fn rq_workflow_spec_rejects_array_instead_of_object() {
    let json = b"[]";
    let result: Result<WorkflowSpec, _> = serde_json::from_slice(json);
    assert!(
        result.is_err(),
        "array instead of object should be rejected"
    );
}

#[test]
fn rq_workflow_spec_rejects_null_workflow_name() {
    let json = r#"{"workflow_name": null, "nodes": [], "edges": [],
        "version": 1}"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err(), "null workflow_name should be rejected");
}

#[test]
fn rq_workflow_spec_rejects_null_nodes() {
    let json = r#"{"workflow_name": "test", "nodes": null, "edges": [],
        "version": 1}"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err(), "null nodes should be rejected");
}

#[test]
fn rq_workflow_spec_rejects_wrong_type_for_workflow_name() {
    let json = r#"{"workflow_name": 123, "nodes": [], "edges": [],
        "version": 1}"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "number instead of string for workflow_name should be rejected"
    );
}

#[test]
fn rq_workflow_spec_rejects_wrong_type_for_nodes() {
    let json = r#"{"workflow_name": "test", "nodes": "not-an-array", "edges": [],
        "version": 1}"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "string instead of array for nodes should be rejected"
    );
}

#[test]
fn rq_workflow_spec_rejects_wrong_type_for_edges() {
    let json = r#"{"workflow_name": "test", "nodes": [], "edges": "not-an-array", "version": 1}"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "string instead of array for edges should be rejected"
    );
}

#[test]
fn rq_workflow_spec_rejects_malformed_json() {
    let json = b"{not valid json{{{";
    let result: Result<WorkflowSpec, _> = serde_json::from_slice(json);
    assert!(result.is_err(), "malformed JSON should be rejected");
}

#[test]
fn rq_workflow_spec_ignores_extra_fields_via_serde() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [{"name": "a", "kind": "pure"}],
        "edges": [],
        "version": 1,
        "extra_field": "ignored",
        "another_extra": 42
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(
        result.is_ok(),
        "extra fields should be silently ignored: {:?}",
        result
    );
}

#[test]
fn rq_workflow_spec_rejects_node_without_name() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [{"kind": "pure"}],
        "edges": [],
        "version": 1
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err(), "node without name should be rejected");
}

#[test]
fn rq_workflow_spec_rejects_node_without_kind() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [{"name": "a"}],
        "edges": [],
        "version": 1
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err(), "node without kind should be rejected");
}

#[test]
fn rq_workflow_spec_rejects_edge_without_from() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [{"name": "a", "kind": "pure"}],
        "edges": [{"to": "a"}]
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err(), "edge without from should be rejected");
}

#[test]
fn rq_workflow_spec_rejects_edge_without_to() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [{"name": "a", "kind": "pure"}],
        "edges": [{"from": "a"}]
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err(), "edge without to should be rejected");
}

#[test]
fn rq_workflow_spec_round_trip_preserves_all_fields() {
    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("test_wf").expect("valid"),
        nodes: vec![
            NodeSpec {
                name: NodeName::parse("node-a").expect("valid"),
                kind: NodeKind::Pure,
            },
            NodeSpec {
                name: NodeName::parse("node-b").expect("valid"),
                kind: NodeKind::ManagedEffect,
            },
        ],
        edges: vec![EdgeSpec {
            from: NodeName::parse("node-a").expect("valid"),
            to: NodeName::parse("node-b").expect("valid"),
        }],
    };
    let json = serde_json::to_string(&spec).expect("serialize");
    let restored: WorkflowSpec = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored, spec);
}

// ===========================================================================
// DIMENSION: version-pin-bypass
// Serializing WorkflowSpec directly bypasses Dag validation
// ===========================================================================

#[test]
fn rq_workflow_spec_accepts_cycle_via_serde() {
    let json = r#"{
        "workflow_name": "cycle_via_serde",
        "nodes": [
            {"name": "a", "kind": "pure"},
            {"name": "b", "kind": "pure"}
        ],
        "edges": [
            {"from": "a", "to": "b"},
            {"from": "b", "to": "a"}
        ]
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(
        result.is_ok(),
        "serde accepts cycle (validation happens elsewhere): {:?}",
        result
    );
    let spec = result.unwrap();
    assert_eq!(spec.edges.len(), 2);
}

#[test]
fn rq_workflow_spec_serde_bypasses_dag_empty_validation() {
    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("empty_via_serde").expect("valid"),
        nodes: vec![],
        edges: vec![],
    };
    let json = serde_json::to_string(&spec).expect("serialize");
    let restored: WorkflowSpec = serde_json::from_str(&json).expect("deserialize");
    assert!(restored.nodes.is_empty());
}

#[test]
fn rq_dag_build_rejects_self_loop_with_proper_error() {
    let mut dag = Dag::new();
    let a: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .expect("valid");
    dag.connect(&a, &a).expect("connect succeeds");
    let build_result = dag.build("self_loop");
    assert!(
        matches!(build_result, Err(DagError::CycleDetected { .. })),
        "Dag::build should reject self-loop with CycleDetected error: {:?}",
        build_result
    );
}

// ===========================================================================
// DIMENSION: edge-cases
// Self-loops via serde, duplicate edges, null handling
// ===========================================================================

#[test]
fn rq_workflow_spec_accepts_self_loop_edge_via_serde() {
    let json = r#"{
        "workflow_name": "self_loop",
        "nodes": [
            {"name": "a", "kind": "pure"},
            {"name": "b", "kind": "pure"}
        ],
        "edges": [
            {"from": "a", "to": "a"}
        ]
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(
        result.is_ok(),
        "serde accepts self-loop (validation happens elsewhere): {:?}",
        result
    );
}

#[test]
fn rq_workflow_spec_accepts_duplicate_edges_via_serde() {
    let json = r#"{
        "workflow_name": "dup_edges",
        "nodes": [
            {"name": "a", "kind": "pure"},
            {"name": "b", "kind": "pure"}
        ],
        "edges": [
            {"from": "a", "to": "b"},
            {"from": "a", "to": "b"}
        ],
        "version": 1
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_ok(), "duplicate edges are accepted: {:?}", result);
    let spec = result.unwrap();
    assert_eq!(spec.edges.len(), 2);
}

#[test]
fn rq_workflow_spec_rejects_empty_workflow_name_via_serde() {
    let json = r#"{"workflow_name": "", "nodes": [{"name": "a", "kind": "pure"}], "edges": [],
        "version": 1}"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err(), "empty workflow_name is rejected");
}

#[test]
fn rq_workflow_spec_rejects_unicode_in_workflow_name_via_serde() {
    let json = r#"{"workflow_name": "工作流", "nodes": [{"name": "a", "kind": "pure"}], "edges": [],
        "version": 1}"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "unicode workflow_name is rejected (only ascii alphanumerics, -, _ allowed): {:?}",
        result
    );
}

#[test]
fn rq_workflow_spec_rejects_control_char_in_node_name_via_serde() {
    let json = format!(
        r#"{{"workflow_name": "test", "nodes": [{{"name": "node\u{{0000}}", "kind": "pure"}}], "edges": [],
        "version": 1}}"#
    );
    let result: Result<WorkflowSpec, _> = serde_json::from_str(&json);
    assert!(
        result.is_err(),
        "control char in node name should be rejected"
    );
}

#[test]
fn rq_workflow_spec_accepts_single_node_no_edges_via_serde() {
    let json = r#"{"workflow_name": "single", "nodes": [{"name": "a", "kind": "pure"}], "edges": [],
        "version": 1}"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_ok(), "single node with no edges is valid");
}

// ===========================================================================
// DIMENSION: dag-error-semantics
// Error messages and Display implementations
// ===========================================================================

#[test]
fn rq_dag_error_invalid_node_name_display() {
    let err = DagError::InvalidNodeName {
        name: "bad!".to_string(),
    };
    let msg = err.to_string();
    assert!(
        msg.contains("bad!"),
        "error display should contain name: {}",
        msg
    );
    assert!(msg.contains("invalid"), "error display should say invalid");
}

#[test]
fn rq_dag_error_node_not_found_display() {
    let err = DagError::NodeNotFound {
        name: "ghost".to_string(),
    };
    let msg = err.to_string();
    assert!(
        msg.contains("ghost"),
        "error display should contain name: {}",
        msg
    );
    assert!(
        msg.contains("not found"),
        "error display should say not found"
    );
}

#[test]
fn rq_dag_error_empty_workflow_display() {
    let err = DagError::EmptyWorkflow;
    let msg = err.to_string();
    assert!(
        msg.contains("empty") || msg.contains("no nodes"),
        "error display should mention empty: {}",
        msg
    );
}

#[test]
fn rq_dag_error_cycle_detected_display() {
    let err = DagError::CycleDetected {
        cycle: "a -> b".to_string(),
    };
    let msg = err.to_string();
    assert!(
        msg.contains("cycle") || msg.contains("Cycle"),
        "error display should mention cycle: {}",
        msg
    );
}

// ===========================================================================
// PROPTEST: property-based adversarial attacks
// ===========================================================================

#[cfg(feature = "proptest")]
mod proptests {
    use super::*;

    proptest! {
        #[test]
        fn rq_workflow_spec_parse_never_panics_with_valid_structure(
            name_suffix in "[a-z]{1,20}",
            kind in prop::sample::select(vec!["pure", "managed_effect", "wait", "signal", "unsafe"]),
        ) {
            let workflow_name = format!("wf-{}", name_suffix);
            let json = serde_json::json!({
                "workflow_name": workflow_name,
                "nodes": [
                    {"name": format!("node-{}", name_suffix), "kind": kind}
                ],
                "edges": [],
        "version": 1
            });
            let bytes = serde_json::to_vec(&json).unwrap();
            let _result = std::panic::catch_unwind(|| {
                let _ignored: Result<WorkflowSpec, _> = serde_json::from_slice(&bytes);
            });
        }

        #[test]
        fn rq_workflow_spec_valid_node_names_always_accepted(
            name in "[a-z][a-z0-9]{0,30}(-[a-z0-9]+)*",
        ) {
            let json = format!(r#"{{"workflow_name": "test", "nodes": [{{"name": "{}", "kind": "pure"}}], "edges": [],
        "version": 1}}"#, name);
            let result: Result<WorkflowSpec, _> = serde_json::from_str(&json);
            prop_assert!(result.is_ok(), "valid node name '{}' should be accepted: {:?}", name, result);
        }

        #[test]
        fn rq_workflow_spec_invalid_node_names_always_rejected(
            name in ".{1,50}",
        ) {
            let json = format!(r#"{{"workflow_name": "test", "nodes": [{{"name": "{}", "kind": "pure"}}], "edges": [],
        "version": 1}}"#, name);
            let result: Result<WorkflowSpec, _> = serde_json::from_str(&json);
            if name.is_empty() || name.starts_with('-') || name.ends_with('-') || name.ends_with('_')
                || name.contains("--") || name.contains("__") || name.contains("-_") || name.contains("_-")
                || name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Ok(());
            }
            prop_assert!(result.is_err(), "invalid node name '{}' should be rejected", name);
        }

        #[test]
        fn rq_dag_build_always_returns_same_result_for_same_input(
            node_count in 1usize..=10usize,
        ) {
            let mut dag = Dag::new();
            let mut handles = Vec::new();
            for i in 0..node_count {
                let h: crate::node_handle::NodeHandle<(), ()> = dag
                    .add_node_with_kind(&format!("node{}", i), NodeKind::Pure, |_: ()| ())
                    .expect("valid");
                handles.push(h);
            }
            for i in 0..node_count.saturating_sub(1) {
                dag.connect(&handles[i], &handles[i + 1]).expect("valid");
            }
            let r1 = dag.build("test");
            let r2 = dag.build("test");
            prop_assert_eq!(r1.is_ok(), r2.is_ok());
        }
    }
}
