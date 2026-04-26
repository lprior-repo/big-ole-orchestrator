//! BDD tests for DAG Cycle Validation & Edge Integrity.
//!
//! bead_id: ve-ttqfg
//!
//! Given/When/Then scenarios covering:
//! 1. Cycle detection with full path reporting
//! 2. Diamond DAG validity
//! 3. Self-loop detection
//! 4. Disconnected subgraph handling
//! 5. Mutually exclusive conditional edges
//! 6. Exhaustive conditional coverage
//! 7. Empty condition defaults to unconditional
//! 8. Large DAG performance (<100ms for 100 nodes)
//! 9. Parallel fan-in validity
//! 10. Terminal-to-active edge rejection

pub(crate) mod cycle_detection;
pub(crate) mod dag_validity;
pub(crate) mod edge_conditions;
pub(crate) mod edge_integrity;
pub(crate) mod performance;

use crate::{
    DagNode, DependencyGraphResolver, Edge, EdgeCondition, NodeName, NonEmptyVec, RetryPolicy,
    StepOutcome, WorkflowDefinition, WorkflowDefinitionError, WorkflowName,
};

/// Helper: construct a valid WorkflowDefinition directly (bypasses parse).
pub(crate) fn make_workflow(
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

/// Helper: parse a JSON workflow definition.
pub(crate) fn parse_workflow(json: serde_json::Value) -> Result<WorkflowDefinition, WorkflowDefinitionError> {
    let bytes = serde_json::to_vec(&json).expect("serialize");
    WorkflowDefinition::parse(&bytes)
}

/// Helper: make a single-node JSON fragment.
pub(crate) fn node_json(name: &str) -> serde_json::Value {
    serde_json::json!({
        "node_name": name,
        "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}
    })
}

/// Helper: make an edge JSON fragment.
pub(crate) fn edge_json(from: &str, to: &str, condition: &str) -> serde_json::Value {
    serde_json::json!({
        "source_node": from,
        "target_node": to,
        "condition": condition
    })
}
