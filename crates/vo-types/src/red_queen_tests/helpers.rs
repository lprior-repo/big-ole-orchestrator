//! Red Queen adversarial test helpers for vo-types workflow module.
//!
//! bead_id: vo-ald
//! phase: state-5-red-queen
//!
//! Shared test infrastructure used by all Red Queen test dimensions.
//! These helpers bypass parse validation for unit testing purposes.

use crate::*;
#[cfg(feature = "proptest")]
use proptest::prelude::*;

/// Build a WorkflowDefinition directly, bypassing parse validation.
pub fn make_def(
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

/// Proptest strategy for StepOutcome.
#[cfg(feature = "proptest")]
pub fn step_outcome_strategy() -> impl Strategy<Value = StepOutcome> {
    proptest::prop_oneof![Just(StepOutcome::Success), Just(StepOutcome::Failure),]
}

/// Proptest strategy for EdgeCondition.
#[cfg(feature = "proptest")]
pub fn edge_condition_strategy() -> impl Strategy<Value = EdgeCondition> {
    proptest::prop_oneof![
        Just(EdgeCondition::Always),
        Just(EdgeCondition::OnSuccess),
        Just(EdgeCondition::OnFailure),
    ]
}
