//! DAG correctness tests batch 2: transitive closures.
//!
//! bead_id: ve-nzmc1

#![allow(clippy::unwrap_used)]

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

/// Verify transitive_dependencies works correctly for diamond DAGs (fan-in).
/// Previously returned empty due to visited-set guard treating shared
/// predecessors as cycle signals. Fixed: ve-4m8jp.
#[test]
fn transitive_dependencies_correct_for_diamond_fan_in() {
    //    a
    //   / \
    //  b   c
    //   \ /
    //    d
    let wf = make_workflow(
        "diamond",
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

    let result = DependencyGraphResolver::transitive_dependencies(&wf, &NodeName("d".into()));
    let result_set: std::collections::HashSet<_> = result.into_iter().collect();

    let expected: std::collections::HashSet<_> = [
        NodeName("a".into()),
        NodeName("b".into()),
        NodeName("c".into()),
    ]
    .into_iter()
    .collect();

    assert_eq!(result_set, expected);
}
