//! Black Hat adversarial tests for vo-sdk WorkflowSpec model (ADR-004/031).
//!
//! bead_id: ve-qv0e
//! type: task
//! phase: red-queen
//!
//! Attack dimensions:
//!   - spec-injection: malformed JSON, extra fields, nulls, type confusion
//!   - validation-bypass: edges referencing non-existent nodes via direct deserialization
//!   - cycle-injection: DAG cycles via direct deserialization bypassing Dag::build
//!   - unbounded-recursion: large DAG causing stack overflow in traversal
//!   - type-confusion: unknown NodeKind variants

use vo_sdk::graph_args::{EdgeSpec, NodeSpec, WorkflowSpec};
use vo_types::{NodeKind, NodeName, WorkflowName};

#[test]
fn rq_workflowspec_rejects_unknown_node_kind() {
    let json = r#"{
        "workflow_name": "test_flow",
        "nodes": [
            {"name": "node1", "kind": "nonexistent_kind"}
        ],
        "edges": []
    }"#;

    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "Unknown NodeKind should be rejected by serde"
    );
}

#[test]
fn rq_workflowspec_accepts_edges_to_nonexistent_nodes() {
    let json = r#"{
        "workflow_name": "test_flow",
        "nodes": [
            {"name": "real_node", "kind": "pure"}
        ],
        "edges": [
            {"from": "real_node", "to": "ghost_node"},
            {"from": "nonexistent_source", "to": "real_node"}
        ]
    }"#;

    let spec: WorkflowSpec = serde_json::from_str(json)
        .expect("Deserialization succeeds but edges reference non-existent nodes!");
    assert_eq!(spec.nodes.len(), 1);
    assert_eq!(spec.edges.len(), 2);
}

#[test]
fn rq_workflowspec_accepts_self_loop_edge() {
    let json = r#"{
        "workflow_name": "test_flow",
        "nodes": [
            {"name": "self_node", "kind": "pure"}
        ],
        "edges": [
            {"from": "self_node", "to": "self_node"}
        ]
    }"#;

    let spec: WorkflowSpec =
        serde_json::from_str(json).expect("Self-loop accepted! Dag::build would reject this.");
    assert_eq!(spec.edges.len(), 1);
    assert_eq!(spec.edges[0].from.as_str(), "self_node");
    assert_eq!(spec.edges[0].to.as_str(), "self_node");
}

#[test]
fn rq_workflowspec_accepts_cycle_via_direct_deserialization() {
    let json = r#"{
        "workflow_name": "cycled_flow",
        "nodes": [
            {"name": "node_a", "kind": "pure"},
            {"name": "node_b", "kind": "pure"},
            {"name": "node_c", "kind": "pure"}
        ],
        "edges": [
            {"from": "node_a", "to": "node_b"},
            {"from": "node_b", "to": "node_c"},
            {"from": "node_c", "to": "node_a"}
        ]
    }"#;

    let spec: WorkflowSpec = serde_json::from_str(json)
        .expect("Cycle accepted via direct deserialization! Dag::build would reject this.");
    assert_eq!(spec.edges.len(), 3);
}

#[test]
fn rq_workflowspec_accepts_duplicate_node_names() {
    let json = r#"{
        "workflow_name": "dup_flow",
        "nodes": [
            {"name": "duplicate", "kind": "pure"},
            {"name": "duplicate", "kind": "managed_effect"}
        ],
        "edges": []
    }"#;

    let spec: WorkflowSpec = serde_json::from_str(json).expect("Duplicate node names accepted!");
    assert_eq!(spec.nodes.len(), 2);
}

#[test]
fn rq_workflowspec_accepts_orphaned_edges() {
    let json = r#"{
        "workflow_name": "orphan_flow",
        "nodes": [
            {"name": "orphan_receiver", "kind": "pure"}
        ],
        "edges": [
            {"from": "orphan_receiver", "to": "totally_ghost"},
            {"from": "completely_made_up", "to": "orphan_receiver"}
        ]
    }"#;

    let spec: WorkflowSpec = serde_json::from_str(json).expect("Orphaned edges accepted!");
    assert_eq!(spec.nodes.len(), 1);
    assert_eq!(spec.edges.len(), 2);
}

#[test]
fn rq_workflowspec_accepts_disconnected_nodes() {
    let json = r#"{
        "workflow_name": "disconnected_flow",
        "nodes": [
            {"name": "isolated_node", "kind": "pure"},
            {"name": "another_island", "kind": "wait"},
            {"name": "yet_another", "kind": "signal"}
        ],
        "edges": []
    }"#;

    let spec: WorkflowSpec = serde_json::from_str(json).expect("Disconnected nodes accepted!");
    assert_eq!(spec.nodes.len(), 3);
    assert!(spec.edges.is_empty());
}

#[test]
fn rq_workflowspec_null_kind_becomes_nonexistent() {
    let json = r#"{
        "workflow_name": "null_kind_flow",
        "nodes": [
            {"name": "node1", "kind": null}
        ],
        "edges": []
    }"#;

    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err(), "Null kind should fail deserialization");
}

#[test]
fn rq_workflowspec_empty_nodes_array_is_valid() {
    let json = r#"{
        "workflow_name": "empty_flow",
        "nodes": [],
        "edges": []
    }"#;

    let spec: WorkflowSpec = serde_json::from_str(json)
        .expect("Empty nodes array accepted! Dag::build would reject this.");
    assert!(spec.nodes.is_empty());
    assert!(spec.edges.is_empty());
}

#[test]
fn rq_workflowspec_extra_fields_are_ignored() {
    let json = r#"{
        "workflow_name": "test_flow",
        "nodes": [
            {"name": "node1", "kind": "pure", "extra_field": "injected", "trusted": false}
        ],
        "edges": [],
        "malicious_payload": "<script>alert('xss')</script>"
    }"#;

    let spec: WorkflowSpec =
        serde_json::from_str(json).expect("Extra fields ignored - potential spec injection!");
    assert_eq!(spec.nodes[0].name.as_str(), "node1");
}

#[test]
fn rq_workflowspec_large_workflow_could_cause_stack_overflow() {
    let num_nodes = 100_000;
    let mut json = format!(r#"{{"workflow_name": "massive_flow", "nodes": ["#);
    for i in 0..num_nodes {
        if i > 0 {
            json.push(',');
        }
        json.push_str(&format!(r#"{{"name": "node{}","kind":"pure"}}"#, i));
    }
    json.push_str("], \"edges\": []}");
    let spec: WorkflowSpec = serde_json::from_str(&json).expect("Massive workflow accepted!");
    assert_eq!(spec.nodes.len(), num_nodes);
}

#[test]
fn rq_workflowspec_invalid_workflow_name_characters() {
    let json = r#"{
        "workflow_name": "invalid--name--with--dashes",
        "nodes": [
            {"name": "node1", "kind": "pure"}
        ],
        "edges": []
    }"#;

    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "WorkflowName with consecutive hyphens should be rejected"
    );
}

#[test]
fn rq_workflowspec_invalid_node_name_characters() {
    let json = r#"{
        "workflow_name": "valid_flow",
        "nodes": [
            {"name": "invalid__node__name", "kind": "pure"}
        ],
        "edges": []
    }"#;

    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "NodeName with consecutive underscores should be rejected"
    );
}

#[test]
fn rq_nodespec_type_confusion_via_integer_kind() {
    let json = r#"{
        "workflow_name": "confusion_flow",
        "nodes": [
            {"name": "node1", "kind": 999}
        ],
        "edges": []
    }"#;

    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "Integer kind should be rejected - type confusion attack"
    );
}

#[test]
fn rq_edgespec_type_confusion_via_integer_from() {
    let json = r#"{
        "workflow_name": "confusion_flow",
        "nodes": [
            {"name": "valid_node", "kind": "pure"}
        ],
        "edges": [
            {"from": 123, "to": "valid_node"}
        ]
    }"#;

    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "Integer edge 'from' should be rejected - type confusion attack"
    );
}

#[test]
fn rq_workflowspec_empty_workflow_name() {
    let json = r#"{
        "workflow_name": "",
        "nodes": [
            {"name": "node1", "kind": "pure"}
        ],
        "edges": []
    }"#;

    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err(), "Empty workflow name should be rejected");
}

#[test]
fn rq_workflowspec_node_name_too_long() {
    let long_name = "a".repeat(129);
    let json = format!(
        r#"{{
        "workflow_name": "valid_flow",
        "nodes": [
            {{"name": "{}", "kind": "pure"}}
        ],
        "edges": []
    }}"#,
        long_name
    );

    let result: Result<WorkflowSpec, _> = serde_json::from_str(&json);
    assert!(
        result.is_err(),
        "NodeName exceeding 128 chars should be rejected"
    );
}
