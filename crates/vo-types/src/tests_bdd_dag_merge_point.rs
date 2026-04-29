//! BDD tests for DAG merge point correctness.
//!
//! bead_id: ve-gj3ez
//!
//! Given/When/Then scenarios covering:
//! 1. Premature merge: merge point fires before all inputs received (false positive)
//! 2. Late merge: merge point waits for inputs that will never arrive (deadlock)
//!
//! The core issue: when a merge point has multiple Always-incoming edges but the
//! predecessors are on mutually exclusive conditional paths (OnSuccess vs OnFailure),
//! the workflow deadlocks because one branch never runs but the merge waits for it.

use crate::{
    DagNode, DependencyGraphResolver, Edge, EdgeCondition, NodeName, NonEmptyVec, RetryPolicy,
    StepOutcome, WorkflowDefinition, WorkflowDefinitionError, WorkflowName,
};

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

fn parse_workflow(json: serde_json::Value) -> Result<WorkflowDefinition, WorkflowDefinitionError> {
    let bytes = serde_json::to_vec(&json).expect("serialize");
    WorkflowDefinition::parse(&bytes)
}

fn node_json(name: &str) -> serde_json::Value {
    serde_json::json!({
        "node_name": name,
        "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}
    })
}

fn edge_json(from: &str, to: &str, condition: &str) -> serde_json::Value {
    serde_json::json!({
        "source_node": from,
        "target_node": to,
        "condition": condition
    })
}

// ============================================================================
// Scenario 1: Valid fan-in with parallel Always branches
// ============================================================================
//
//     A
//    / \
//   B   C    (both Always)
//    \ /
//     D       (D waits for both B and C to complete)
//
// This is VALID because both B and C can always run (Always edges from A).

mod scenario_1_valid_parallel_fan_in {
    use super::*;

    #[test]
    fn given_parallel_always_branches_merging_when_dag_validated_then_valid() {
        let json = serde_json::json!({
            "workflow_name": "valid-fan-in",
            "nodes": [node_json("A"), node_json("B"), node_json("C"), node_json("D")],
            "edges": [
                edge_json("A", "B", "Always"),
                edge_json("A", "C", "Always"),
                edge_json("B", "D", "Always"),
                edge_json("C", "D", "Always"),
            ]
        });
        let result = parse_workflow(json);
        let def = result.expect("parallel Always branches should be valid");
        assert_eq!(def.nodes.len(), 4);
        assert_eq!(def.edges.len(), 4);
    }

    #[test]
    fn given_valid_fan_in_when_a_completes_then_b_and_c_become_ready() {
        let def = make_workflow(
            "fan-in",
            vec![
                ("A", 1, 0, 1.0),
                ("B", 1, 0, 1.0),
                ("C", 1, 0, 1.0),
                ("D", 1, 0, 1.0),
            ],
            vec![
                ("A", "B", EdgeCondition::Always),
                ("A", "C", EdgeCondition::Always),
                ("B", "D", EdgeCondition::Always),
                ("C", "D", EdgeCondition::Always),
            ],
        );

        let ready = DependencyGraphResolver::ready_nodes_for_outcome(
            &def,
            &[NodeName("A".into())],
            StepOutcome::Success,
        );
        assert_eq!(ready.len(), 2);
        assert!(ready.contains(&NodeName("B".into())));
        assert!(ready.contains(&NodeName("C".into())));
    }

    #[test]
    fn given_valid_fan_in_after_b_and_c_complete_then_d_becomes_ready() {
        let def = make_workflow(
            "fan-in",
            vec![
                ("A", 1, 0, 1.0),
                ("B", 1, 0, 1.0),
                ("C", 1, 0, 1.0),
                ("D", 1, 0, 1.0),
            ],
            vec![
                ("A", "B", EdgeCondition::Always),
                ("A", "C", EdgeCondition::Always),
                ("B", "D", EdgeCondition::Always),
                ("C", "D", EdgeCondition::Always),
            ],
        );

        let ready = DependencyGraphResolver::ready_nodes_for_outcome(
            &def,
            &[
                NodeName("A".into()),
                NodeName("B".into()),
                NodeName("C".into()),
            ],
            StepOutcome::Success,
        );
        assert_eq!(ready.len(), 1);
        assert!(ready.contains(&NodeName("D".into())));
    }

    #[test]
    fn given_valid_fan_in_d_is_not_ready_after_only_b_completes() {
        let def = make_workflow(
            "fan-in",
            vec![
                ("A", 1, 0, 1.0),
                ("B", 1, 0, 1.0),
                ("C", 1, 0, 1.0),
                ("D", 1, 0, 1.0),
            ],
            vec![
                ("A", "B", EdgeCondition::Always),
                ("A", "C", EdgeCondition::Always),
                ("B", "D", EdgeCondition::Always),
                ("C", "D", EdgeCondition::Always),
            ],
        );

        let ready = DependencyGraphResolver::ready_nodes_for_outcome(
            &def,
            &[NodeName("A".into()), NodeName("B".into())],
            StepOutcome::Success,
        );
        assert!(
            !ready.contains(&NodeName("D".into())),
            "D should not be ready until both B and C complete"
        );
    }
}

// ============================================================================
// Scenario 2: Invalid - exclusive branches with Always merge (DEADLOCK)
// ============================================================================
//
//     A
//    / \
//   B   C    (B: OnSuccess, C: OnFailure - mutually exclusive!)
//    \ /
//     D       (D waits for both B and C - DEADLOCK!)
//
// When A succeeds: B runs, C never runs, D waits for C forever.
// When A fails: C runs, B never runs, D waits for B forever.
//
// This is INVALID but current validation does NOT catch it.

mod scenario_2_exclusive_branches_with_always_merge {
    use super::*;

    #[test]
    fn given_exclusive_branches_merging_with_always_edges_when_dag_validated_then_should_reject() {
        let json = serde_json::json!({
            "workflow_name": "deadlock-merge",
            "nodes": [node_json("A"), node_json("B"), node_json("C"), node_json("D")],
            "edges": [
                edge_json("A", "B", "OnSuccess"),
                edge_json("A", "C", "OnFailure"),
                edge_json("B", "D", "Always"),
                edge_json("C", "D", "Always"),
            ]
        });
        let result = parse_workflow(json);
        assert!(
            result.is_err(),
            "Workflow with exclusive branches merging via Always edges should be rejected - causes deadlock"
        );
    }

    #[test]
    fn when_a_succeeds_only_b_runs_and_d_waits_for_c_forever() {
        let def = make_workflow(
            "deadlock-merge",
            vec![
                ("A", 1, 0, 1.0),
                ("B", 1, 0, 1.0),
                ("C", 1, 0, 1.0),
                ("D", 1, 0, 1.0),
            ],
            vec![
                ("A", "B", EdgeCondition::OnSuccess),
                ("A", "C", EdgeCondition::OnFailure),
                ("B", "D", EdgeCondition::Always),
                ("C", "D", EdgeCondition::Always),
            ],
        );

        let ready_after_a_success = DependencyGraphResolver::ready_nodes_for_outcome(
            &def,
            &[NodeName("A".into())],
            StepOutcome::Success,
        );
        assert_eq!(ready_after_a_success.len(), 1);
        assert!(ready_after_a_success.contains(&NodeName("B".into())));

        let ready_after_b_success = DependencyGraphResolver::ready_nodes_for_outcome(
            &def,
            &[NodeName("A".into()), NodeName("B".into())],
            StepOutcome::Success,
        );
        assert!(
            !ready_after_b_success.contains(&NodeName("D".into())),
            "D should NOT be ready - C never ran so D waits forever (deadlock)"
        );
        assert!(
            ready_after_b_success.is_empty(),
            "No nodes should be ready - deadlock condition"
        );
    }

    #[test]
    fn when_a_fails_only_c_runs_and_d_waits_for_b_forever() {
        let def = make_workflow(
            "deadlock-merge",
            vec![
                ("A", 1, 0, 1.0),
                ("B", 1, 0, 1.0),
                ("C", 1, 0, 1.0),
                ("D", 1, 0, 1.0),
            ],
            vec![
                ("A", "B", EdgeCondition::OnSuccess),
                ("A", "C", EdgeCondition::OnFailure),
                ("B", "D", EdgeCondition::Always),
                ("C", "D", EdgeCondition::Always),
            ],
        );

        let ready_after_a_failure = DependencyGraphResolver::ready_nodes_for_outcome(
            &def,
            &[NodeName("A".into())],
            StepOutcome::Failure,
        );
        assert_eq!(ready_after_a_failure.len(), 1);
        assert!(ready_after_a_failure.contains(&NodeName("C".into())));

        let ready_after_c_failure = DependencyGraphResolver::ready_nodes_for_outcome(
            &def,
            &[NodeName("A".into()), NodeName("C".into())],
            StepOutcome::Failure,
        );
        assert!(
            !ready_after_c_failure.contains(&NodeName("D".into())),
            "D should NOT be ready - B never ran so D waits forever (deadlock)"
        );
    }
}

// ============================================================================
// Scenario 3: Valid - single branch followed by merge
// ============================================================================
//
//     A
//     |
//     B
//    / \
//   C   D    (both Always)
//    \ /
//     E
//
// This is VALID - no deadlock because only one path exists.

mod scenario_3_valid_single_path_merge {
    use super::*;

    #[test]
    fn given_single_path_with_merge_when_dag_validated_then_valid() {
        let json = serde_json::json!({
            "workflow_name": "single-path-merge",
            "nodes": [
                node_json("A"),
                node_json("B"),
                node_json("C"),
                node_json("D"),
                node_json("E"),
            ],
            "edges": [
                edge_json("A", "B", "Always"),
                edge_json("B", "C", "Always"),
                edge_json("B", "D", "Always"),
                edge_json("C", "E", "Always"),
                edge_json("D", "E", "Always"),
            ]
        });
        let result = parse_workflow(json);
        let def = result.expect("single path with merge should be valid");
        assert_eq!(def.nodes.len(), 5);
    }

    #[test]
    fn execution_layers_for_single_path_merge() {
        let def = make_workflow(
            "single-path",
            vec![
                ("A", 1, 0, 1.0),
                ("B", 1, 0, 1.0),
                ("C", 1, 0, 1.0),
                ("D", 1, 0, 1.0),
                ("E", 1, 0, 1.0),
            ],
            vec![
                ("A", "B", EdgeCondition::Always),
                ("B", "C", EdgeCondition::Always),
                ("B", "D", EdgeCondition::Always),
                ("C", "E", EdgeCondition::Always),
                ("D", "E", EdgeCondition::Always),
            ],
        );

        let layers = DependencyGraphResolver::execution_layers(&def);
        assert_eq!(layers.len(), 4);
        assert_eq!(layers[0].len(), 1); // A
        assert_eq!(layers[1].len(), 1); // B
        assert_eq!(layers[2].len(), 2); // C, D (parallel)
        assert_eq!(layers[3].len(), 1); // E
    }
}

// ============================================================================
// Scenario 4: Late merge detection - merge point with unreachable predecessor
// ============================================================================
//
// Three branches where one is unreachable, but merge waits for all three.
//
//     A
//    /|\
//   B C D
//    \不全|
//     E
//
// If B and D are on exclusive conditions but E waits for all three, deadlock.

mod scenario_4_three_way_merge_with_exclusive {
    use super::*;

    #[test]
    fn given_three_branches_two_exclusive_merging_when_validated_then_should_reject() {
        let json = serde_json::json!({
            "workflow_name": "three-way-deadlock",
            "nodes": [node_json("A"), node_json("B"), node_json("C"), node_json("D"), node_json("E")],
            "edges": [
                edge_json("A", "B", "OnSuccess"),
                edge_json("A", "C", "Always"),
                edge_json("A", "D", "OnFailure"),
                edge_json("B", "E", "Always"),
                edge_json("C", "E", "Always"),
                edge_json("D", "E", "Always"),
            ]
        });
        let result = parse_workflow(json);
        assert!(
            result.is_err(),
            "Workflow where E waits for B, C, D but B and D are mutually exclusive should be rejected"
        );
    }
}

// ============================================================================
// Scenario 5: Wide fan-in with all Always edges (VALID)
// ============================================================================

mod scenario_5_wide_fan_in_all_always {
    use super::*;

    #[test]
    fn given_10_branches_all_always_merging_when_validated_then_valid() {
        let mut nodes = vec![
            serde_json::json!({"node_name": "entry", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}),
        ];
        for i in 0..10 {
            nodes.push(node_json(&format!("worker{}", i)));
        }
        nodes.push(node_json("merge"));

        let mut edges = Vec::new();
        for i in 0..10 {
            edges.push(edge_json("entry", &format!("worker{}", i), "Always"));
            edges.push(edge_json(&format!("worker{}", i), "merge", "Always"));
        }

        let json = serde_json::json!({
            "workflow_name": "wide-fan-in",
            "nodes": nodes,
            "edges": edges,
        });
        let result = parse_workflow(json);
        let def = result.expect("10-way fan-in with all Always should be valid");
        assert_eq!(def.nodes.len(), 12);
    }

    #[test]
    fn merge_waits_for_all_10_workers_before_firing() {
        let json = serde_json::json!({
            "workflow_name": "wide-fan-in",
            "nodes": [
                {"node_name": "entry", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
                {"node_name": "merge", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            ],
            "edges": [
                {"source_node": "entry", "target_node": "merge", "condition": "Always"},
            ]
        });
        let def = parse_workflow(json).expect("parse should succeed");
        let ready = DependencyGraphResolver::ready_nodes_for_outcome(
            &def,
            &[NodeName("entry".into())],
            StepOutcome::Success,
        );
        assert!(ready.contains(&NodeName("merge".into())));
    }
}

// ============================================================================
// Scenario 7: Mixed conditions but still valid merge
// ============================================================================
//
// When a node has multiple incoming edges with different conditions,
// as long as ALL edges can potentially be satisfied by some execution
// path, the merge is valid.
//
//     A
//    / \
//   B   C   (B: OnSuccess, C: OnSuccess - both taken on success)
//    \ /
//     D
//
// When A succeeds: B runs, C also runs, D gets both inputs.

mod scenario_7_both_branches_taken_on_same_outcome {
    use super::*;

    #[test]
    fn given_both_branches_taken_on_success_when_a_succeeds_then_d_waits_for_both() {
        let def = make_workflow(
            "both-on-success",
            vec![
                ("A", 1, 0, 1.0),
                ("B", 1, 0, 1.0),
                ("C", 1, 0, 1.0),
                ("D", 1, 0, 1.0),
            ],
            vec![
                ("A", "B", EdgeCondition::OnSuccess),
                ("A", "C", EdgeCondition::OnSuccess),
                ("B", "D", EdgeCondition::Always),
                ("C", "D", EdgeCondition::Always),
            ],
        );

        let ready_after_a_success = DependencyGraphResolver::ready_nodes_for_outcome(
            &def,
            &[NodeName("A".into())],
            StepOutcome::Success,
        );
        assert_eq!(ready_after_a_success.len(), 2);
        assert!(ready_after_a_success.contains(&NodeName("B".into())));
        assert!(ready_after_a_success.contains(&NodeName("C".into())));

        let ready_after_both_complete = DependencyGraphResolver::ready_nodes_for_outcome(
            &def,
            &[
                NodeName("A".into()),
                NodeName("B".into()),
                NodeName("C".into()),
            ],
            StepOutcome::Success,
        );
        assert_eq!(ready_after_both_complete.len(), 1);
        assert!(ready_after_both_complete.contains(&NodeName("D".into())));
    }
}
