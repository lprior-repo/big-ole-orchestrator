//! DependencyGraphResolver: direct dependencies and dependents.
//!
//! Tests for predecessors (incoming edges), successors (outgoing edges),
//! and basic graph traversal.

use crate::{
    DagNode, DependencyGraphResolver, Edge, EdgeCondition, NodeName, NonEmptyVec, RetryPolicy,
    WorkflowDefinition,
};

fn make_workflow(
    name: &str,
    nodes: Vec<(&str, u8, u64, f64)>,
    edges: Vec<(&str, &str, EdgeCondition)>,
) -> WorkflowDefinition {
    WorkflowDefinition {
        workflow_name: crate::WorkflowName(name.into()),
        nodes: NonEmptyVec::new_unchecked(
            nodes
                .into_iter()
                .map(|(n, a, b, m)| DagNode {
                    node_name: NodeName(n.into()),
                    retry_policy: RetryPolicy {
                        max_attempts: a,
                        backoff_ms: b,
                        backoff_multiplier: m,
                        max_backoff_ms: u64::MAX,
                    },
                    compensation_policy: None,
                })
                .collect(),
        ),
        edges: edges
            .into_iter()
            .map(|(s, t, c)| Edge {
                source_node: NodeName(s.into()),
                target_node: NodeName(t.into()),
                condition: c,
            })
            .collect(),
    }
}

// ============================================================================
// Direct dependencies (predecessors)
// ============================================================================

// DGR-01: A node with no incoming edges has no dependencies
#[test]
fn dependencies_returns_empty_for_node_with_no_incoming_edges() {
    let workflow = make_workflow(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0), ("c", 1, 0, 1.0)],
        vec![("a", "b", EdgeCondition::Always)],
    );

    let deps = DependencyGraphResolver::dependencies(&workflow, &NodeName("c".into()));
    assert!(
        deps.is_empty(),
        "Node 'c' has no incoming edges, dependencies should be empty"
    );
}

// DGR-02: A node with one incoming edge has one dependency
#[test]
fn dependencies_returns_single_predecessor() {
    let workflow = make_workflow(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0)],
        vec![("a", "b", EdgeCondition::Always)],
    );

    let deps = DependencyGraphResolver::dependencies(&workflow, &NodeName("b".into()));
    assert_eq!(deps.len(), 1, "Node 'b' has one incoming edge from 'a'");
    assert!(deps.contains(&NodeName("a".into())));
}

// DGR-03: A node with multiple incoming edges has multiple dependencies
#[test]
fn dependencies_returns_all_predecessors() {
    let workflow = make_workflow(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0), ("c", 1, 0, 1.0)],
        vec![
            ("a", "c", EdgeCondition::Always),
            ("b", "c", EdgeCondition::Always),
        ],
    );

    let deps = DependencyGraphResolver::dependencies(&workflow, &NodeName("c".into()));
    assert_eq!(
        deps.len(),
        2,
        "Node 'c' has two incoming edges from 'a' and 'b'"
    );
    assert!(deps.contains(&NodeName("a".into())));
    assert!(deps.contains(&NodeName("b".into())));
}

// DGR-04: Transitive dependencies are not included (direct only)
#[test]
fn dependencies_returns_only_direct_predecessors() {
    // a -> b -> c
    let workflow = make_workflow(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0), ("c", 1, 0, 1.0)],
        vec![
            ("a", "b", EdgeCondition::Always),
            ("b", "c", EdgeCondition::Always),
        ],
    );

    // c depends on b (direct), not on a (transitive)
    let deps = DependencyGraphResolver::dependencies(&workflow, &NodeName("c".into()));
    assert_eq!(deps.len(), 1, "Node 'c' depends only on 'b', not on 'a'");
    assert!(deps.contains(&NodeName("b".into())));
}

// DGR-05: Returns empty for non-existent node
#[test]
fn dependencies_returns_empty_for_nonexistent_node() {
    let workflow = make_workflow("test", vec![("a", 1, 0, 1.0)], vec![]);

    let deps = DependencyGraphResolver::dependencies(&workflow, &NodeName("nonexistent".into()));
    assert!(deps.is_empty(), "Non-existent node has no dependencies");
}

// ============================================================================
// Dependents (successors)
// ============================================================================

// DGR-06: A node with no outgoing edges has no dependents
#[test]
fn dependents_returns_empty_for_node_with_no_outgoing_edges() {
    let workflow = make_workflow(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0)],
        vec![("a", "b", EdgeCondition::Always)],
    );

    let succs = DependencyGraphResolver::dependents(&workflow, &NodeName("b".into()));
    assert!(
        succs.is_empty(),
        "Node 'b' has no outgoing edges, dependents should be empty"
    );
}

// DGR-07: A node with one outgoing edge has one dependent
#[test]
fn dependents_returns_single_successor() {
    let workflow = make_workflow(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0)],
        vec![("a", "b", EdgeCondition::Always)],
    );

    let succs = DependencyGraphResolver::dependents(&workflow, &NodeName("a".into()));
    assert_eq!(succs.len(), 1, "Node 'a' has one outgoing edge to 'b'");
    assert!(succs.contains(&NodeName("b".into())));
}

// DGR-08: Diamond dependency - node with multiple direct dependents
#[test]
fn dependents_returns_all_direct_successors() {
    //    a
    //   / \
    //  b   c
    //   \ /
    //    d
    let workflow = make_workflow(
        "test",
        vec![
            ("a", 1, 0, 1.0),
            ("b", 1, 0, 1.0),
            ("c", 1, 0, 1.0),
            ("d", 1, 0, 1.0),
        ],
        vec![
            ("a", "b", EdgeCondition::Always),
            ("a", "c", EdgeCondition::Always),
            ("b", "d", EdgeCondition::Always),
            ("c", "d", EdgeCondition::Always),
        ],
    );

    let succs = DependencyGraphResolver::dependents(&workflow, &NodeName("a".into()));
    assert_eq!(succs.len(), 2, "Node 'a' directly depends on 'b' and 'c'");
    assert!(succs.contains(&NodeName("b".into())));
    assert!(succs.contains(&NodeName("c".into())));
}
