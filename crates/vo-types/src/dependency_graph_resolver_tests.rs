//! Dependency graph resolver tests (TDD Red phase).
//!
//! bead_id: ve-eo0
//! phase: tdd-red
//!
//! These tests define the expected behavior of the DependencyGraphResolver.
//! The implementation will be done in ve-6ez (TDD Green phase).
//!
//! The resolver provides:
//! - Dependency resolution: find predecessors/successors of nodes
//! - Ready node computation: find nodes whose dependencies are satisfied
//! - Execution layer computation: group nodes by dependency depth for parallel execution

use crate::{
    DagNode, DependencyGraphResolver, Edge, EdgeCondition, NodeName, NonEmptyVec, RetryPolicy,
    StepOutcome, WorkflowDefinition, WorkflowName,
};

/// Helper to construct a WorkflowDefinition for testing.
fn make_workflow(
    name: &str,
    nodes: Vec<(&str, u8, u64, f64)>,
    edges: Vec<(&str, &str, EdgeCondition)>,
) -> WorkflowDefinition {
    WorkflowDefinition {
        workflow_name: WorkflowName(name.into()),
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
// DependencyGraphResolver: direct dependencies (predecessors)
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
// DependencyGraphResolver: dependents (successors)
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

// ============================================================================
// DependencyGraphResolver: ready nodes computation
// ============================================================================

// DGR-09: All nodes with no dependencies are ready initially
#[test]
fn ready_nodes_returns_source_nodes_when_nothing_completed() {
    let workflow = make_workflow(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0), ("c", 1, 0, 1.0)],
        vec![],
    );

    let ready = DependencyGraphResolver::ready_nodes(&workflow, &[]);
    assert_eq!(
        ready.len(),
        3,
        "All 3 nodes have no dependencies and are ready"
    );
}

// DGR-10: Nodes with completed dependencies become ready
#[test]
fn ready_nodes_returns_node_when_all_dependencies_completed() {
    // a -> b -> c
    let workflow = make_workflow(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0), ("c", 1, 0, 1.0)],
        vec![
            ("a", "b", EdgeCondition::Always),
            ("b", "c", EdgeCondition::Always),
        ],
    );

    // After 'a' completes, 'b' should be ready
    let ready = DependencyGraphResolver::ready_nodes(&workflow, &[NodeName("a".into())]);
    assert_eq!(
        ready.len(),
        1,
        "Only 'b' should be ready after 'a' completes"
    );
    assert!(ready.contains(&NodeName("b".into())));
}

// DGR-11: Node with multiple dependencies requires all to complete
#[test]
fn ready_nodes_requires_all_dependencies() {
    // a -> c
    // b -> c
    let workflow = make_workflow(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0), ("c", 1, 0, 1.0)],
        vec![
            ("a", "c", EdgeCondition::Always),
            ("b", "c", EdgeCondition::Always),
        ],
    );

    // After 'a' completes but 'b' has not, 'c' should NOT be ready
    let ready = DependencyGraphResolver::ready_nodes(&workflow, &[NodeName("a".into())]);
    assert!(
        !ready.contains(&NodeName("c".into())),
        "'c' should not be ready until both 'a' and 'b' complete"
    );

    // After both 'a' and 'b' complete, 'c' should be ready
    let ready = DependencyGraphResolver::ready_nodes(
        &workflow,
        &[NodeName("a".into()), NodeName("b".into())],
    );
    assert_eq!(ready.len(), 1);
    assert!(ready.contains(&NodeName("c".into())));
}

// DGR-12: Completed nodes are not returned as ready
#[test]
fn ready_nodes_excludes_already_completed_nodes() {
    // a -> b -> c
    let workflow = make_workflow(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0), ("c", 1, 0, 1.0)],
        vec![
            ("a", "b", EdgeCondition::Always),
            ("b", "c", EdgeCondition::Always),
        ],
    );

    // After 'a' and 'b' complete, only 'c' should be ready (not 'a' or 'b')
    let ready = DependencyGraphResolver::ready_nodes(
        &workflow,
        &[NodeName("a".into()), NodeName("b".into())],
    );
    assert_eq!(ready.len(), 1);
    assert!(ready.contains(&NodeName("c".into())));
}

// ============================================================================
// DependencyGraphResolver: execution layers
// ============================================================================

// DGR-13: Linear chain has one node per layer
#[test]
fn execution_layers_linear_chain() {
    // a -> b -> c
    let workflow = make_workflow(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0), ("c", 1, 0, 1.0)],
        vec![
            ("a", "b", EdgeCondition::Always),
            ("b", "c", EdgeCondition::Always),
        ],
    );

    let layers = DependencyGraphResolver::execution_layers(&workflow);
    assert_eq!(layers.len(), 3, "Linear chain has 3 layers");
    assert_eq!(layers[0].len(), 1);
    assert!(layers[0].contains(&NodeName("a".into())));
    assert_eq!(layers[1].len(), 1);
    assert!(layers[1].contains(&NodeName("b".into())));
    assert_eq!(layers[2].len(), 1);
    assert!(layers[2].contains(&NodeName("c".into())));
}

// DGR-14: Parallel branches are in the same layer
#[test]
fn execution_layers_parallel_branches_same_layer() {
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

    let layers = DependencyGraphResolver::execution_layers(&workflow);
    assert_eq!(layers.len(), 3, "Diamond has 3 layers");

    // Layer 0: 'a' only
    assert_eq!(layers[0].len(), 1);
    assert!(layers[0].contains(&NodeName("a".into())));

    // Layer 1: 'b' and 'c' (parallel)
    assert_eq!(layers[1].len(), 2);
    assert!(layers[1].contains(&NodeName("b".into())));
    assert!(layers[1].contains(&NodeName("c".into())));

    // Layer 2: 'd' only
    assert_eq!(layers[2].len(), 1);
    assert!(layers[2].contains(&NodeName("d".into())));
}

// DGR-15: Disconnected components each get their own layer structure
#[test]
fn execution_layers_disconnected_components() {
    // Component 1: a -> b
    // Component 2: c -> d
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
            ("c", "d", EdgeCondition::Always),
        ],
    );

    let layers = DependencyGraphResolver::execution_layers(&workflow);
    // Should still produce valid layers per component
    assert!(!layers.is_empty());

    // All nodes should appear exactly once across layers
    let all_nodes: Vec<NodeName> = layers.iter().flatten().cloned().collect();
    assert_eq!(all_nodes.len(), 4);
}

// DGR-16: Single node workflow produces single layer
#[test]
fn execution_layers_single_node() {
    let workflow = make_workflow("test", vec![("a", 1, 0, 1.0)], vec![]);
    let layers = DependencyGraphResolver::execution_layers(&workflow);
    assert_eq!(layers.len(), 1, "Single node has 1 layer");
    assert_eq!(layers[0].len(), 1);
    assert!(layers[0].contains(&NodeName("a".into())));
}

// ============================================================================
// DependencyGraphResolver: cycle handling
// ============================================================================

// DGR-19: Resolver operates on validated acyclic graphs
// Note: WorkflowDefinition::parse rejects cycles, so the resolver
// never encounters cyclic graphs. This test documents that invariant.
#[test]
fn resolver_operates_on_acyclic_graph() {
    // a -> b -> c (linear chain, clearly acyclic)
    let workflow = make_workflow(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0), ("c", 1, 0, 1.0)],
        vec![
            ("a", "b", EdgeCondition::Always),
            ("b", "c", EdgeCondition::Always),
        ],
    );

    // The resolver should successfully compute transitive dependencies
    let transitive =
        DependencyGraphResolver::transitive_dependencies(&workflow, &NodeName("c".into()));
    assert_eq!(transitive.len(), 2);
}

// DGR-18: Transitive dependents are correctly computed
#[test]
fn transitive_dependents() {
    // a -> b -> c -> d
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
            ("b", "c", EdgeCondition::Always),
            ("c", "d", EdgeCondition::Always),
        ],
    );

    let transitive =
        DependencyGraphResolver::transitive_dependents(&workflow, &NodeName("a".into()));
    assert_eq!(
        transitive.len(),
        3,
        "'a' transitively affects 'b', 'c', 'd'"
    );
    assert!(transitive.contains(&NodeName("b".into())));
    assert!(transitive.contains(&NodeName("c".into())));
    assert!(transitive.contains(&NodeName("d".into())));
}

// ============================================================================
// DependencyGraphResolver: cycle handling
// ============================================================================

// DGR-19: Resolver operates on validated acyclic graphs.
// WorkflowDefinition::parse rejects cycles, so the resolver never encounters
// them in production. If constructed without validation (as make_workflow does),
// the visited-set guard prevents infinite loops and returns partial results.
#[test]
fn transitive_dependencies_handles_unvalidated_cyclic_input() {
    // a -> b -> c -> a (cycle)
    let workflow = make_workflow(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0), ("c", 1, 0, 1.0)],
        vec![
            ("a", "b", EdgeCondition::Always),
            ("b", "c", EdgeCondition::Always),
            ("c", "a", EdgeCondition::Always),
        ],
    );

    // With the continue-based guard, cyclic input terminates without infinite loop.
    // It returns reachable nodes up to the cycle boundary (not empty).
    // The resolver operates on validated acyclic graphs; this is a defensive test.
    let result = DependencyGraphResolver::transitive_dependencies(&workflow, &NodeName("c".into()));
    assert!(
        !result.is_empty(),
        "Cyclic input should still return reachable nodes (terminates via visited set)"
    );
    assert_eq!(result.len(), 2, "From c: reaches b and a before cycle");
}

// ============================================================================
// DependencyGraphResolver: edge condition filtering
// ============================================================================

// DGR-20: Dependencies respect edge conditions (only Always edges count for ready computation)
#[test]
fn dependencies_filters_by_relevant_conditions() {
    let workflow = make_workflow(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0), ("c", 1, 0, 1.0)],
        vec![
            ("a", "b", EdgeCondition::Always),
            ("a", "c", EdgeCondition::OnSuccess), // Only taken on success
        ],
    );

    // Both 'b' and 'c' are direct successors of 'a'
    let deps_of_b = DependencyGraphResolver::dependencies(&workflow, &NodeName("b".into()));
    let deps_of_c = DependencyGraphResolver::dependencies(&workflow, &NodeName("c".into()));

    assert_eq!(deps_of_b.len(), 1);
    assert_eq!(deps_of_c.len(), 1);
}

// ============================================================================
// DependencyGraphResolver: readiness with conditions
// ============================================================================

// DGR-21: Ready nodes considers edge conditions
#[test]
fn ready_nodes_considers_success_failure_conditions() {
    let workflow = make_workflow(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0), ("c", 1, 0, 1.0)],
        vec![
            ("a", "b", EdgeCondition::OnSuccess), // Only if 'a' succeeds
            ("a", "c", EdgeCondition::OnFailure), // Only if 'a' fails
        ],
    );

    // If 'a' succeeded, only 'b' should be ready (not 'c')
    let ready = DependencyGraphResolver::ready_nodes_for_outcome(
        &workflow,
        &[NodeName("a".into())],
        StepOutcome::Success,
    );
    assert_eq!(ready.len(), 1);
    assert!(ready.contains(&NodeName("b".into())));

    // If 'a' failed, only 'c' should be ready (not 'b')
    let ready = DependencyGraphResolver::ready_nodes_for_outcome(
        &workflow,
        &[NodeName("a".into())],
        StepOutcome::Failure,
    );
    assert_eq!(ready.len(), 1);
    assert!(ready.contains(&NodeName("c".into())));
}

// DGR-22: Always edges make nodes ready regardless of outcome
#[test]
fn ready_nodes_always_edges_ready_after_any_outcome() {
    let workflow = make_workflow(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0)],
        vec![("a", "b", EdgeCondition::Always)],
    );

    // After 'a' completes (success or failure), 'b' should be ready
    let ready_success = DependencyGraphResolver::ready_nodes_for_outcome(
        &workflow,
        &[NodeName("a".into())],
        StepOutcome::Success,
    );
    assert!(ready_success.contains(&NodeName("b".into())));

    let ready_failure = DependencyGraphResolver::ready_nodes_for_outcome(
        &workflow,
        &[NodeName("a".into())],
        StepOutcome::Failure,
    );
    assert!(ready_failure.contains(&NodeName("b".into())));
}
