//! Red Queen tests: next_nodes-edge-cases dimension.
//!
//! Tests non-existent current node, duplicates, condition filtration, and complex graphs.

use super::helpers;
use crate::*;
use std::collections::HashSet;

// RQ-27: next_nodes with non-existent current node returns empty
#[test]
fn rq_next_nodes_nonexistent_current_returns_empty() {
    let def = helpers::make_def("test", vec![("a", 1, 0, 1.0)], vec![]);
    let result = next_nodes(&NodeName("nonexistent".into()), StepOutcome::Success, &def);
    assert!(result.is_empty());
}

// RQ-28: next_nodes with non-existent current on Failure returns empty
#[test]
fn rq_next_nodes_nonexistent_current_failure_returns_empty() {
    let def = helpers::make_def("test", vec![("a", 1, 0, 1.0)], vec![]);
    let result = next_nodes(&NodeName("nonexistent".into()), StepOutcome::Failure, &def);
    assert!(result.is_empty());
}

// RQ-29: next_nodes with duplicate edges returns duplicate results (per NG-14)
#[test]
fn rq_next_nodes_duplicate_edges_returns_duplicates() {
    let def = WorkflowDefinition {
        workflow_name: WorkflowName("test".into()),
        nodes: NonEmptyVec::new_unchecked(vec![
            DagNode {
                node_name: NodeName("a".into()),
                retry_policy: RetryPolicy::new(1, 0, 1.0).unwrap(),
                compensation_policy: None,
            },
            DagNode {
                node_name: NodeName("b".into()),
                retry_policy: RetryPolicy::new(1, 0, 1.0).unwrap(),
                compensation_policy: None,
            },
        ]),
        edges: vec![
            Edge {
                source_node: NodeName("a".into()),
                target_node: NodeName("b".into()),
                condition: EdgeCondition::Always,
            },
            Edge {
                source_node: NodeName("a".into()),
                target_node: NodeName("b".into()),
                condition: EdgeCondition::Always,
            },
        ],
    };
    let result = next_nodes(&NodeName("a".into()), StepOutcome::Success, &def);
    // Two identical edges -> two results (NG-14: no edge deduplication)
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].node_name, NodeName("b".into()));
    assert_eq!(result[1].node_name, NodeName("b".into()));
}

// RQ-30: next_nodes with same target via different conditions
#[test]
fn rq_next_nodes_same_target_different_conditions_success() {
    let def = WorkflowDefinition {
        workflow_name: WorkflowName("test".into()),
        nodes: NonEmptyVec::new_unchecked(vec![
            DagNode {
                node_name: NodeName("a".into()),
                retry_policy: RetryPolicy::new(1, 0, 1.0).unwrap(),
                compensation_policy: None,
            },
            DagNode {
                node_name: NodeName("b".into()),
                retry_policy: RetryPolicy::new(1, 0, 1.0).unwrap(),
                compensation_policy: None,
            },
        ]),
        edges: vec![
            Edge {
                source_node: NodeName("a".into()),
                target_node: NodeName("b".into()),
                condition: EdgeCondition::Always,
            },
            Edge {
                source_node: NodeName("a".into()),
                target_node: NodeName("b".into()),
                condition: EdgeCondition::OnSuccess,
            },
        ],
    };
    // Success: Always + OnSuccess both fire -> 2 results
    let result_success = next_nodes(&NodeName("a".into()), StepOutcome::Success, &def);
    assert_eq!(result_success.len(), 2);

    // Failure: only Always fires -> 1 result
    let result_failure = next_nodes(&NodeName("a".into()), StepOutcome::Failure, &def);
    assert_eq!(result_failure.len(), 1);
}

// RQ-31: next_nodes OnFailure-only edge returns nothing on Success
#[test]
fn rq_next_nodes_on_failure_only_returns_nothing_on_success() {
    let def = helpers::make_def(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0)],
        vec![("a", "b", EdgeCondition::OnFailure)],
    );
    let result = next_nodes(&NodeName("a".into()), StepOutcome::Success, &def);
    assert!(result.is_empty());
}

// RQ-32: next_nodes OnSuccess-only edge returns nothing on Failure
#[test]
fn rq_next_nodes_on_success_only_returns_nothing_on_failure() {
    let def = helpers::make_def(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0)],
        vec![("a", "b", EdgeCondition::OnSuccess)],
    );
    let result = next_nodes(&NodeName("a".into()), StepOutcome::Failure, &def);
    assert!(result.is_empty());
}

// RQ-33: next_nodes terminal node (no outgoing edges) returns empty for both outcomes
#[test]
fn rq_next_nodes_terminal_node_empty_for_both_outcomes() {
    let def = helpers::make_def(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0)],
        vec![("a", "b", EdgeCondition::Always)],
    );
    assert!(next_nodes(&NodeName("b".into()), StepOutcome::Success, &def).is_empty());
    assert!(next_nodes(&NodeName("b".into()), StepOutcome::Failure, &def).is_empty());
}

// RQ-34: next_nodes always edge matches both outcomes
#[test]
fn rq_next_nodes_always_edge_matches_both_outcomes() {
    let def = helpers::make_def(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0)],
        vec![("a", "b", EdgeCondition::Always)],
    );
    assert_eq!(
        next_nodes(&NodeName("a".into()), StepOutcome::Success, &def).len(),
        1
    );
    assert_eq!(
        next_nodes(&NodeName("a".into()), StepOutcome::Failure, &def).len(),
        1
    );
}

// RQ-35: next_nodes with mixed conditions from same source
#[test]
fn rq_next_nodes_mixed_conditions_three_targets() {
    let def = helpers::make_def(
        "test",
        vec![
            ("a", 1, 0, 1.0),
            ("b", 1, 0, 1.0),
            ("c", 1, 0, 1.0),
            ("d", 1, 0, 1.0),
        ],
        vec![
            ("a", "b", EdgeCondition::Always),
            ("a", "c", EdgeCondition::OnSuccess),
            ("a", "d", EdgeCondition::OnFailure),
        ],
    );
    // Success: Always(b) + OnSuccess(c) = [b, c]
    let success = next_nodes(&NodeName("a".into()), StepOutcome::Success, &def);
    let names: HashSet<&str> = success.iter().map(|n| n.node_name.0.as_str()).collect();
    assert_eq!(names.len(), 2);
    assert!(names.contains("b") && names.contains("c"));

    // Failure: Always(b) + OnFailure(d) = [b, d]
    let failure = next_nodes(&NodeName("a".into()), StepOutcome::Failure, &def);
    let names: HashSet<&str> = failure.iter().map(|n| n.node_name.0.as_str()).collect();
    assert_eq!(names.len(), 2);
    assert!(names.contains("b") && names.contains("d"));
}
