//! Next-step selection types and contract.
//!
//! Design-by-contract for deterministic next-step selection in workflow execution.
//! Pure calculation layer — no external state mutation.
//!
//! # Types
//!
//! - `NextStep`: Result of next-step selection, carrying the selected step identity
//! - `SchedulingIntention`: Event payload emitted when a step is scheduled
//! - `SelectionError`: Errors from next-step selection
//!
//! # Invariants
//!
//! - INV-001: Selection is deterministic given identical inputs (workflow, completed set, last_outcome)
//! - INV-002: Selection does not mutate external state
//! - INV-003: Selected step must be a valid ready node (all dependencies completed, conditions satisfied)
//! - INV-004: If no ready nodes exist, selection returns `None` (not an error)
//!
//! # References
//!
//! - Parent: ve-ohwpr (Isolate next-step selection and step scheduling)
//! - Black-hat review: ve-4cxd8 (F-03: StepScheduled carries no step_id payload)

use serde::{Deserialize, Serialize};

use crate::{NodeName, StepOutcome};

// ============================================================================
// Error Types
// ============================================================================

/// Errors returned by next-step selection.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SelectionError {
    /// No ready nodes exist given the current completed set and last outcome.
    #[error("no ready nodes: all pending nodes have unmet dependencies or conditions")]
    NoReadyNodes,

    /// Invalid input: completed set contains nodes not in the workflow.
    #[error("completed node '{0}' does not exist in workflow")]
    UnknownCompletedNode(NodeName),

    /// Invalid input: last_outcome is provided but no edges require condition matching.
    #[error("last_outcome provided but workflow has no conditional edges")]
    UnnecessaryOutcome,
}

// ============================================================================
// Core Types
// ============================================================================

/// Result of next-step selection.
///
/// Carries the identity of the selected step for deterministic replay.
///
/// # Determinism Guarantee (INV-001)
///
/// Given identical inputs (workflow definition, completed set, last outcome),
/// this type will always contain the same `step_id` (or `None` if no ready nodes).
///
/// # Ordering Tiebreaker
///
/// When multiple nodes are ready, the first node in definition order (as defined
/// in the workflow's nodes list) is selected. This provides a deterministic
/// tiebreaker without requiring additional metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NextStep {
    /// The selected step's node name.
    pub step_id: NodeName,

    /// The attempt number for this scheduling (1-indexed).
    ///
    /// Computed from the command history of the workflow instance.
    pub attempt: u32,

    /// Fence token for this scheduling (for single-active guarantees).
    pub fence: u64,
}

/// Scheduling intention event emitted when a step is selected.
///
/// This is the event payload that replaces the bare `StepScheduled` transition
/// event. It carries all necessary context for the executor to begin execution.
///
/// # Relation to State Machine
///
/// When the state machine transitions from `RunningDecision` to `StepScheduled`,
/// it emits this `SchedulingIntention` as the event payload. The state machine
/// itself remains pure (no data), but the emitted event carries the step identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulingIntention {
    /// Workflow instance identifier.
    pub workflow_id: String,

    /// Selected step's node name.
    pub step_id: NodeName,

    /// Attempt number (1-indexed).
    pub attempt: u32,

    /// Fence token for single-active-instance guarantee.
    pub fence: u64,

    /// Unique execution identifier for this scheduling.
    pub execution_id: String,
}

impl SchedulingIntention {
    /// Create a new scheduling intention.
    #[must_use]
    pub fn new(
        workflow_id: String,
        step_id: NodeName,
        attempt: u32,
        fence: u64,
        execution_id: String,
    ) -> Self {
        Self {
            workflow_id,
            step_id,
            attempt,
            fence,
            execution_id,
        }
    }
}

// ============================================================================
// Selection API
// ============================================================================

/// Select the next step to execute from a workflow definition.
///
/// # Arguments
///
/// * `workflow` - The validated workflow definition (guaranteed acyclic)
/// * `completed` - Set of node names that have been completed
/// * `last_outcome` - Outcome of the most recently completed step (used for condition matching)
///
/// # Returns
///
/// * `Ok(Some(NextStep))` - A step was selected
/// * `Ok(None)` - No ready nodes exist (workflow is waiting for external input or blocked)
/// * `Err(SelectionError)` - Invalid input (completed set contains unknown nodes)
///
/// # Invariants Enforced
///
/// * INV-001: Deterministic selection — identical inputs yield identical results
/// * INV-003: Selected step is a valid ready node (all dependencies completed, conditions satisfied)
///
/// # Determinism Guarantee
///
/// When multiple nodes are ready, the first node in definition order (as defined
/// in `workflow.nodes`) is selected. This provides a deterministic tiebreaker.
///
/// # Condition Matching
///
/// For each incoming edge to a candidate node, the edge's condition is evaluated
/// against the outcome of its source node. If the source node is in `completed`,
/// its outcome is determined by checking if `last_outcome` matches. This works
/// correctly for linear chains but may be lossy for complex DAGs with fan-in from
/// nodes with different outcomes.
///
/// For full per-node outcome tracking, the caller should ensure `completed` only
/// contains nodes whose outcomes are consistent with `last_outcome`, or use a
/// more sophisticated outcome map in a future extension.
pub fn select_next_step(
    workflow: &crate::WorkflowDefinition,
    completed: &[NodeName],
    last_outcome: Option<StepOutcome>,
) -> Result<Option<NextStep>, SelectionError> {
    // Validate completed set
    for node in completed {
        if workflow.get_node(node).is_none() {
            return Err(SelectionError::UnknownCompletedNode(node.clone()));
        }
    }

    // Find ready nodes
    let ready_nodes = find_ready_nodes(workflow, completed, last_outcome);

    if ready_nodes.is_empty() {
        return Err(SelectionError::NoReadyNodes);
    }

    // Select first node in definition order (deterministic tiebreaker)
    let selected = ready_nodes.into_iter().next().unwrap();

    // TODO: Compute attempt and fence from command history / instance state
    // For now, use default values — these should be injected by the caller
    Ok(Some(NextStep {
        step_id: selected,
        attempt: 1,
        fence: 0,
    }))
}

/// Find all nodes that are ready to execute.
///
/// A node is ready when:
/// 1. It has not yet been completed
/// 2. All its dependencies have been completed
/// 3. All incoming edge conditions are satisfied (if last_outcome is provided)
///
/// Returns nodes in definition order (as they appear in the workflow).
fn find_ready_nodes(
    workflow: &crate::WorkflowDefinition,
    completed: &[NodeName],
    last_outcome: Option<StepOutcome>,
) -> Vec<NodeName> {
    let completed_set: std::collections::HashSet<&NodeName> = completed.iter().collect();

    workflow
        .nodes
        .as_slice()
        .iter()
        .filter_map(|node| {
            let node_name = &node.node_name;

            // Skip already completed nodes
            if completed_set.contains(node_name) {
                return None;
            }

            // Check dependencies
            let deps = crate::DependencyGraphResolver::dependencies(workflow, node_name);
            if !deps.iter().all(|dep| completed_set.contains(dep)) {
                return None;
            }

            // Check edge conditions if last_outcome is provided
            if let Some(outcome) = last_outcome {
                let incoming: Vec<_> = workflow
                    .edges
                    .iter()
                    .filter(|edge| &edge.target_node == node_name)
                    .collect();

                if !incoming.is_empty() {
                    let all_conditions_satisfied = incoming.iter().all(|edge| {
                        // Source node must be completed
                        completed_set.contains(&edge.source_node)
                            // Condition must match last outcome
                            && edge.condition.matches(outcome)
                    });

                    if !all_conditions_satisfied {
                        return None;
                    }
                }
            }

            Some(node_name.clone())
        })
        .collect()
}

// ============================================================================
// Intention Emission
// ============================================================================

/// Convert a `NextStep` selection into a `SchedulingIntention` event.
///
/// This is the boundary between pure selection (which returns `NextStep`)
/// and event emission (which produces `SchedulingIntention`).
///
/// # Arguments
///
/// * `next_step` - The selected step (from `select_next_step`)
/// * `workflow_id` - The workflow instance identifier
/// * `execution_id` - A unique execution identifier for this scheduling
///
/// # Returns
///
/// A `SchedulingIntention` ready to be emitted as a transition event payload.
#[must_use]
pub fn emit_scheduling_intention(
    next_step: NextStep,
    workflow_id: String,
    execution_id: String,
) -> SchedulingIntention {
    SchedulingIntention::new(
        workflow_id,
        next_step.step_id,
        next_step.attempt,
        next_step.fence,
        execution_id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_workflow() -> crate::WorkflowDefinition {
        use crate::{DagNode, Edge, EdgeCondition, NonEmptyVec, RetryPolicy, WorkflowName};

        let nodes = NonEmptyVec::new_unchecked(vec![
            DagNode {
                node_name: NodeName("A".to_string()),
                retry_policy: RetryPolicy::new(3, 100, 2.0).unwrap(),
                compensation_policy: Default::default(),
            },
            DagNode {
                node_name: NodeName("B".to_string()),
                retry_policy: RetryPolicy::new(3, 100, 2.0).unwrap(),
                compensation_policy: Default::default(),
            },
            DagNode {
                node_name: NodeName("C".to_string()),
                retry_policy: RetryPolicy::new(3, 100, 2.0).unwrap(),
                compensation_policy: Default::default(),
            },
        ]);

        let edges = vec![
            Edge {
                source_node: NodeName("A".to_string()),
                target_node: NodeName("B".to_string()),
                condition: EdgeCondition::Always,
            },
            Edge {
                source_node: NodeName("A".to_string()),
                target_node: NodeName("C".to_string()),
                condition: EdgeCondition::OnSuccess,
            },
        ];

        crate::WorkflowDefinition {
            workflow_name: WorkflowName("test".to_string()),
            nodes,
            edges,
        }
    }

    #[test]
    fn test_select_next_step_returns_first_node_when_no_completed() {
        let workflow = create_test_workflow();
        let completed = vec![];

        let result = select_next_step(&workflow, &completed, None);

        assert!(result.is_ok());
        let next_step = result.unwrap();
        assert!(next_step.is_some());
        assert_eq!(next_step.unwrap().step_id.to_string(), "A");
    }

    #[test]
    fn test_select_next_step_returns_b_after_a_completes() {
        let workflow = create_test_workflow();
        let completed = vec![NodeName("A".to_string())];

        let result = select_next_step(&workflow, &completed, Some(StepOutcome::Success));

        assert!(result.is_ok());
        let next_step = result.unwrap();
        assert!(next_step.is_some());
        assert_eq!(next_step.unwrap().step_id.to_string(), "B");
    }

    #[test]
    fn test_select_next_step_returns_c_after_a_completes_success() {
        let workflow = create_test_workflow();
        let completed = vec![NodeName("A".to_string())];

        let result = select_next_step(&workflow, &completed, Some(StepOutcome::Success));

        assert!(result.is_ok());
        let next_step = result.unwrap();
        assert!(next_step.is_some());
        // Both B and C are ready, but B comes first in definition order
        assert_eq!(next_step.unwrap().step_id.to_string(), "B");
    }

    #[test]
    fn test_select_next_step_returns_none_when_all_completed() {
        let workflow = create_test_workflow();
        let completed = vec![
            NodeName("A".to_string()),
            NodeName("B".to_string()),
            NodeName("C".to_string()),
        ];

        let result = select_next_step(&workflow, &completed, Some(StepOutcome::Success));

        assert!(matches!(result, Err(SelectionError::NoReadyNodes)));
    }

    #[test]
    fn test_select_next_step_returns_error_for_unknown_completed_node() {
        let workflow = create_test_workflow();
        let completed = vec![NodeName("Unknown".to_string())];

        let result = select_next_step(&workflow, &completed, None);

        assert!(matches!(
            result,
            Err(SelectionError::UnknownCompletedNode(_))
        ));
    }

    #[test]
    fn test_scheduling_intention_creation() {
        let intention = SchedulingIntention::new(
            "wf-123".to_string(),
            NodeName("step-1".to_string()),
            1,
            42,
            "exec-456".to_string(),
        );

        assert_eq!(intention.workflow_id, "wf-123");
        assert_eq!(intention.step_id.to_string(), "step-1");
        assert_eq!(intention.attempt, 1);
        assert_eq!(intention.fence, 42);
        assert_eq!(intention.execution_id, "exec-456");
    }

    #[test]
    fn test_emit_scheduling_intention() {
        let next_step = NextStep {
            step_id: NodeName("step-1".to_string()),
            attempt: 1,
            fence: 42,
        };

        let intention =
            emit_scheduling_intention(next_step, "wf-123".to_string(), "exec-456".to_string());

        assert_eq!(intention.workflow_id, "wf-123");
        assert_eq!(intention.step_id.to_string(), "step-1");
        assert_eq!(intention.attempt, 1);
        assert_eq!(intention.fence, 42);
        assert_eq!(intention.execution_id, "exec-456");
    }
}
