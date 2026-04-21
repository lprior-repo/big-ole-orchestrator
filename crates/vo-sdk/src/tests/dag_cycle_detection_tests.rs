//! BDD tests for DAG cycle detection preventing infinite loops (ve-2bisb).
//!
//! Verifies that cycle detection at multiple gates prevents infinite execution:
//! 1. Dag::build() — Kahn's algorithm rejects cyclic graphs at build time
//! 2. WorkflowSpec deserialization — rejects cyclic specs during parsing
//!
//! Scenarios:
//! - Self-loop: node depends on itself
//! - Mutual dependency: A→B→A
//! - Complex cycle: multi-node cycles with DAG branches

use crate::dag::{Dag, DagError};
use crate::graph::WorkflowSpec;
use crate::node_handle::NodeHandle;
use vo_types::NodeKind;

// Helper: add a node with String→String (allows self-connect and mutual cycles).
fn add_node(dag: &mut Dag, name: &str) -> NodeHandle<String, String> {
    dag.add_node_with_kind(name, NodeKind::Pure, |s: String| s)
        .expect("add node")
}

// ========================================================================
// Self-Loop Detection
// ========================================================================

/// Given: A DAG where a single node has a self-connect
/// When: Dag::build() is called
/// Then: CycleDetected error is returned (prevents infinite self-execution)
#[test]
fn dag_build_rejects_self_loop_single_node() {
    let mut dag = Dag::new();
    let a = add_node(&mut dag, "a");
    dag.connect(&a, &a).expect("self-connect allowed at add time");

    let result = dag.build("self_loop");
    assert!(
        matches!(result, Err(DagError::CycleDetected { .. })),
        "self-loop must be rejected: {result:?}"
    );
}

/// Given: A DAG with a valid chain A→B→C plus a self-loop on B
/// When: Dag::build() is called
/// Then: CycleDetected error is returned
#[test]
fn dag_build_rejects_self_loop_in_larger_graph() {
    let mut dag = Dag::new();
    let a = add_node(&mut dag, "a");
    let b = add_node(&mut dag, "b");
    let c = add_node(&mut dag, "c");
    dag.connect(&a, &b).expect("a->b");
    dag.connect(&b, &c).expect("b->c");
    dag.connect(&b, &b).expect("b->b self-loop");

    let result = dag.build("partial_self_loop");
    assert!(
        matches!(result, Err(DagError::CycleDetected { .. })),
        "self-loop in larger graph must be rejected: {result:?}"
    );
}

/// Given: A WorkflowSpec JSON with a self-loop edge A→A
/// When: Deserialized
/// Then: Deserialization fails with cycle error
#[test]
fn workflow_spec_deserialization_rejects_self_loop() {
    let json = serde_json::json!({
        "workflow_name": "self-loop-spec",
        "nodes": [{"name": "A", "kind": "pure"}],
        "edges": [{"from": "A", "to": "A"}]
    });
    let result: Result<WorkflowSpec, _> = serde_json::from_value(json);
    assert!(result.is_err(), "self-loop in WorkflowSpec must be rejected");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("cycle") || err_msg.contains("self-loop"),
        "error should mention cycle: {err_msg}"
    );
}

// ========================================================================
// Mutual Dependency Detection (A→B→A)
// ========================================================================

/// Given: Two nodes with mutual edges A→B and B→A
/// When: Dag::build() is called
/// Then: CycleDetected error is returned
#[test]
fn dag_build_rejects_mutual_dependency() {
    let mut dag = Dag::new();
    let a = add_node(&mut dag, "a");
    let b = add_node(&mut dag, "b");
    dag.connect(&a, &b).expect("a->b");
    dag.connect(&b, &a).expect("b->a");

    let result = dag.build("mutual_dep");
    assert!(
        matches!(result, Err(DagError::CycleDetected { .. })),
        "mutual dependency must be rejected: {result:?}"
    );
}

/// Given: A WorkflowSpec JSON with mutual edges A→B and B→A
/// When: Deserialized
/// Then: Deserialization fails with cycle error
#[test]
fn workflow_spec_deserialization_rejects_mutual_dependency() {
    let json = serde_json::json!({
        "workflow_name": "mutual-dep",
        "nodes": [{"name": "A", "kind": "pure"}, {"name": "B", "kind": "pure"}],
        "edges": [{"from": "A", "to": "B"}, {"from": "B", "to": "A"}]
    });
    let result: Result<WorkflowSpec, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "mutual dependency in WorkflowSpec must be rejected"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("cycle"),
        "error should mention cycle: {err_msg}"
    );
}

// ========================================================================
// Complex Cycle Detection
// ========================================================================

/// Given: Three nodes forming a cycle A→B→C→A
/// When: Dag::build() is called
/// Then: CycleDetected error is returned with cycle path
#[test]
fn dag_build_rejects_three_node_cycle() {
    let mut dag = Dag::new();
    let a = add_node(&mut dag, "a");
    let b = add_node(&mut dag, "b");
    let c = add_node(&mut dag, "c");
    dag.connect(&a, &b).expect("a->b");
    dag.connect(&b, &c).expect("b->c");
    dag.connect(&c, &a).expect("c->a back-edge");

    let result = dag.build("three_node_cycle");
    assert!(
        matches!(result, Err(DagError::CycleDetected { .. })),
        "three-node cycle must be rejected: {result:?}"
    );
    if let Err(DagError::CycleDetected { cycle }) = result {
        assert!(
            cycle.contains("a") || cycle.contains("b") || cycle.contains("c"),
            "cycle path should reference cycle members: {cycle}"
        );
    }
}

/// Given: A diamond DAG (A→B, A→C, B→D, C→D) with a back-edge D→A
/// When: Dag::build() is called
/// Then: CycleDetected error is returned
#[test]
fn dag_build_rejects_diamond_with_back_edge() {
    let mut dag = Dag::new();
    let a = add_node(&mut dag, "a");
    let b = add_node(&mut dag, "b");
    let c = add_node(&mut dag, "c");
    let d = add_node(&mut dag, "d");
    dag.connect(&a, &b).expect("a->b");
    dag.connect(&a, &c).expect("a->c");
    dag.connect(&b, &d).expect("b->d");
    dag.connect(&c, &d).expect("c->d");
    dag.connect(&d, &a).expect("d->a back-edge");

    let result = dag.build("diamond_back_edge");
    assert!(
        matches!(result, Err(DagError::CycleDetected { .. })),
        "diamond with back-edge must be rejected: {result:?}"
    );
}

/// Given: A DAG where a cycle exists in one branch but other branches are acyclic
/// When: Dag::build() is called
/// Then: CycleDetected error is returned (partial cycle still detected)
#[test]
fn dag_build_rejects_partial_cycle_in_branch() {
    // A → B → D (valid branch)
    // A → C → E → C (cyclic branch)
    let mut dag = Dag::new();
    let a = add_node(&mut dag, "a");
    let b = add_node(&mut dag, "b");
    let c = add_node(&mut dag, "c");
    let d = add_node(&mut dag, "d");
    let e = add_node(&mut dag, "e");
    dag.connect(&a, &b).expect("a->b");
    dag.connect(&a, &c).expect("a->c");
    dag.connect(&b, &d).expect("b->d");
    dag.connect(&c, &e).expect("c->e");
    dag.connect(&e, &c).expect("e->c back-edge (cycle)");

    let result = dag.build("partial_cycle");
    assert!(
        matches!(result, Err(DagError::CycleDetected { .. })),
        "partial cycle in branch must be rejected: {result:?}"
    );
}

/// Given: A valid linear DAG A→B→C
/// When: Dag::build() is called
/// Then: Build succeeds (no false positives)
#[test]
fn dag_build_accepts_valid_linear_chain() {
    let mut dag = Dag::new();
    let a = add_node(&mut dag, "a");
    let b = add_node(&mut dag, "b");
    let c = add_node(&mut dag, "c");
    dag.connect(&a, &b).expect("a->b");
    dag.connect(&b, &c).expect("b->c");

    let spec = dag.build("valid_chain").expect("valid chain should build");
    assert_eq!(spec.nodes.len(), 3);
    assert_eq!(spec.edges.len(), 2);
}

/// Given: A valid diamond DAG A→B, A→C, B→D, C→D
/// When: Dag::build() is called
/// Then: Build succeeds (diamond is a DAG, not a cycle)
#[test]
fn dag_build_accepts_valid_diamond() {
    let mut dag = Dag::new();
    let a = add_node(&mut dag, "a");
    let b = add_node(&mut dag, "b");
    let c = add_node(&mut dag, "c");
    let d = add_node(&mut dag, "d");
    dag.connect(&a, &b).expect("a->b");
    dag.connect(&a, &c).expect("a->c");
    dag.connect(&b, &d).expect("b->d");
    dag.connect(&c, &d).expect("c->d");

    let spec = dag.build("valid_diamond").expect("diamond should build");
    assert_eq!(spec.nodes.len(), 4);
    assert_eq!(spec.edges.len(), 4);
}

// ========================================================================
// WorkflowSpec Deserialization — Complex Cycles
// ========================================================================

/// Given: A WorkflowSpec JSON with a three-node cycle A→B→C→A
/// When: Deserialized
/// Then: Deserialization fails (prevents cyclic workflow loading)
#[test]
fn workflow_spec_deserialization_rejects_three_node_cycle() {
    let json = serde_json::json!({
        "workflow_name": "cycle-abc",
        "nodes": [
            {"name": "A", "kind": "pure"},
            {"name": "B", "kind": "pure"},
            {"name": "C", "kind": "pure"}
        ],
        "edges": [
            {"from": "A", "to": "B"},
            {"from": "B", "to": "C"},
            {"from": "C", "to": "A"}
        ]
    });
    let result: Result<WorkflowSpec, _> = serde_json::from_value(json);
    assert!(result.is_err(), "three-node cycle must be rejected");
}

/// Given: A WorkflowSpec JSON with a diamond + back-edge (A→B, A→C, B→D, C→D, D→A)
/// When: Deserialized
/// Then: Deserialization fails
#[test]
fn workflow_spec_deserialization_rejects_diamond_with_back_edge() {
    let json = serde_json::json!({
        "workflow_name": "diamond-back",
        "nodes": [
            {"name": "A", "kind": "pure"},
            {"name": "B", "kind": "pure"},
            {"name": "C", "kind": "pure"},
            {"name": "D", "kind": "pure"}
        ],
        "edges": [
            {"from": "A", "to": "B"},
            {"from": "A", "to": "C"},
            {"from": "B", "to": "D"},
            {"from": "C", "to": "D"},
            {"from": "D", "to": "A"}
        ]
    });
    let result: Result<WorkflowSpec, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "diamond with back-edge must be rejected"
    );
}

/// Given: A valid WorkflowSpec JSON with a diamond DAG
/// When: Deserialized
/// Then: Deserialization succeeds (no false positive)
#[test]
fn workflow_spec_deserialization_accepts_valid_diamond() {
    let json = serde_json::json!({
        "workflow_name": "valid-diamond",
        "nodes": [
            {"name": "A", "kind": "pure"},
            {"name": "B", "kind": "pure"},
            {"name": "C", "kind": "pure"},
            {"name": "D", "kind": "pure"}
        ],
        "edges": [
            {"from": "A", "to": "B"},
            {"from": "A", "to": "C"},
            {"from": "B", "to": "D"},
            {"from": "C", "to": "D"}
        ]
    });
    let spec: WorkflowSpec =
        serde_json::from_value(json).expect("valid diamond should deserialize");
    assert_eq!(spec.nodes.len(), 4);
    assert_eq!(spec.edges.len(), 4);
}

/// Given: A WorkflowSpec JSON with a 10-node chain (valid)
/// When: Deserialized
/// Then: Deserialization succeeds
#[test]
fn workflow_spec_deserialization_accepts_long_valid_chain() {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for i in 0..10 {
        nodes.push(serde_json::json!({"name": format!("n{i}"), "kind": "pure"}));
    }
    for i in 0..9 {
        edges.push(serde_json::json!({"from": format!("n{i}"), "to": format!("n{}", i + 1)}));
    }
    let json = serde_json::json!({
        "workflow_name": "long-chain",
        "nodes": nodes,
        "edges": edges
    });
    let spec: WorkflowSpec =
        serde_json::from_value(json).expect("10-node chain should deserialize");
    assert_eq!(spec.nodes.len(), 10);
    assert_eq!(spec.edges.len(), 9);
}

/// Given: A WorkflowSpec JSON with edges referencing non-existent nodes
/// When: Deserialized
/// Then: Deserialization fails (edge integrity check)
#[test]
fn workflow_spec_deserialization_rejects_unknown_node_in_edge() {
    let json = serde_json::json!({
        "workflow_name": "bad-edges",
        "nodes": [{"name": "A", "kind": "pure"}],
        "edges": [{"from": "A", "to": "Ghost"}]
    });
    let result: Result<WorkflowSpec, _> = serde_json::from_value(json);
    assert!(result.is_err(), "unknown node in edge must be rejected");
}
