use std::collections::HashSet;

use crate::{EdgeCondition, NodeName, StepOutcome, WorkflowDefinition};

/// Dependency graph resolver for workflow execution planning.
///
/// This resolver provides functions for:
/// - Finding dependencies (predecessors) and dependents (successors) of nodes
/// - Computing which nodes are ready to execute given completed nodes
/// - Computing execution layers for parallel execution planning
///
/// All functions operate on a validated `WorkflowDefinition` (which is guaranteed acyclic).
pub struct DependencyGraphResolver;

impl DependencyGraphResolver {
    /// Get all direct dependencies (predecessors) of a node.
    ///
    /// A node is a dependency of another if there is an edge from it to the other.
    ///
    /// # Panics
    ///
    /// Panics if `node` is not found in the workflow (caller must validate).
    pub fn dependencies(workflow: &WorkflowDefinition, node: &NodeName) -> Vec<NodeName> {
        workflow
            .edges
            .iter()
            .filter(|edge| &edge.target_node == node)
            .map(|edge| edge.source_node.clone())
            .collect()
    }

    /// Get all direct dependents (successors) of a node.
    ///
    /// A node is a dependent of another if there is an edge from the other to it.
    ///
    /// # Panics
    ///
    /// Panics if `node` is not found in the workflow (caller must validate).
    pub fn dependents(workflow: &WorkflowDefinition, node: &NodeName) -> Vec<NodeName> {
        workflow
            .edges
            .iter()
            .filter(|edge| &edge.source_node == node)
            .map(|edge| edge.target_node.clone())
            .collect()
    }

    /// Get all transitive dependencies of a node.
    ///
    /// Returns all nodes that the given node depends on, directly or indirectly.
    pub fn transitive_dependencies(
        workflow: &WorkflowDefinition,
        node: &NodeName,
    ) -> Vec<NodeName> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = vec![node.clone()];

        while let Some(current) = queue.pop() {
            if visited.insert(current.clone()) {
                let direct_deps = Self::dependencies(workflow, &current);
                for dep in direct_deps {
                    if !result.contains(&dep) {
                        result.push(dep.clone());
                    }
                    queue.push(dep);
                }
            }
        }

        result
    }

    /// Get all transitive dependents of a node.
    ///
    /// Returns all nodes that depend on the given node, directly or indirectly.
    pub fn transitive_dependents(workflow: &WorkflowDefinition, node: &NodeName) -> Vec<NodeName> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = vec![node.clone()];

        while let Some(current) = queue.pop() {
            if visited.insert(current.clone()) {
                let direct_deps = Self::dependents(workflow, &current);
                for dep in direct_deps {
                    if !result.contains(&dep) {
                        result.push(dep.clone());
                    }
                    queue.push(dep);
                }
            }
        }

        result
    }

    /// Get nodes that are ready to execute, given the set of completed nodes.
    ///
    /// A node is ready when all of its dependencies have been completed.
    /// Nodes that are already in `completed` are not returned.
    ///
    /// Note: This treats all edges as `Always` condition. For condition-aware
    /// readiness, use `ready_nodes_for_outcome`.
    pub fn ready_nodes(workflow: &WorkflowDefinition, completed: &[NodeName]) -> Vec<NodeName> {
        let completed_set: HashSet<&NodeName> = completed.iter().collect();

        workflow
            .nodes
            .as_slice()
            .iter()
            .filter_map(|node| {
                let node_name = &node.node_name;
                if completed_set.contains(node_name) {
                    return None;
                }
                let deps = Self::dependencies(workflow, node_name);
                if deps.iter().all(|dep| completed_set.contains(dep)) {
                    Some(node_name.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get nodes that are ready to execute, given the last step's outcome.
    ///
    /// Uses edge conditions to determine which edges are active:
    /// - `Always`: always active
    /// - `OnSuccess`: active only if `last_outcome == StepOutcome::Success`
    /// - `OnFailure`: active only if `last_outcome == StepOutcome::Failure`
    pub fn ready_nodes_for_outcome(
        workflow: &WorkflowDefinition,
        completed: &[NodeName],
        last_outcome: StepOutcome,
    ) -> Vec<NodeName> {
        let completed_set: HashSet<&NodeName> = completed.iter().collect();

        workflow
            .nodes
            .as_slice()
            .iter()
            .filter_map(|node| {
                let node_name = &node.node_name;
                if completed_set.contains(node_name) {
                    return None;
                }
                let deps = Self::dependencies_with_condition(workflow, node_name, last_outcome);
                if deps.iter().all(|dep| completed_set.contains(dep)) {
                    Some(node_name.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Compute execution layers for the workflow.
    ///
    /// Returns nodes grouped by dependency depth. Nodes in the same layer
    /// have no dependencies on each other and can be executed in parallel.
    /// Layers are ordered from root nodes (layer 0) to leaf nodes.
    ///
    /// Returns empty vec for empty workflows.
    pub fn execution_layers(workflow: &WorkflowDefinition) -> Vec<Vec<NodeName>> {
        if workflow.nodes.is_empty() {
            return vec![];
        }

        let mut layers = Vec::new();
        let mut assigned: HashSet<NodeName> = HashSet::new();
        let mut remaining: Vec<NodeName> = workflow
            .nodes
            .as_slice()
            .iter()
            .map(|n| n.node_name.clone())
            .collect();

        while !remaining.is_empty() {
            let mut current_layer = Vec::new();

            for node_name in remaining.iter() {
                let deps = Self::dependencies(workflow, node_name);
                let all_deps_assigned = deps.iter().all(|d| assigned.contains(d));

                if all_deps_assigned {
                    current_layer.push(node_name.clone());
                }
            }

            if current_layer.is_empty() {
                break;
            }

            for node_name in current_layer.iter() {
                remaining.retain(|n| n != node_name);
                assigned.insert(node_name.clone());
            }

            layers.push(current_layer);
        }

        layers
    }

    fn dependencies_with_condition(
        workflow: &WorkflowDefinition,
        node: &NodeName,
        outcome: StepOutcome,
    ) -> Vec<NodeName> {
        workflow
            .edges
            .iter()
            .filter(|edge| &edge.target_node == node && edge.condition.matches(outcome))
            .map(|edge| edge.source_node.clone())
            .collect()
    }
}

impl EdgeCondition {
    fn matches(&self, outcome: StepOutcome) -> bool {
        match self {
            EdgeCondition::Always => true,
            EdgeCondition::OnSuccess => outcome == StepOutcome::Success,
            EdgeCondition::OnFailure => outcome == StepOutcome::Failure,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DagNode, Edge, NonEmptyVec};

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
                        retry_policy: crate::RetryPolicy {
                            max_attempts: a,
                            backoff_ms: b,
                            backoff_multiplier: m,
                            max_backoff_ms: u64::MAX,
                        },
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

    #[test]
    fn dependencies_returns_empty_for_node_with_no_incoming_edges() {
        let workflow = make_workflow(
            "test",
            vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0), ("c", 1, 0, 1.0)],
            vec![("a", "b", EdgeCondition::Always)],
        );

        let deps = DependencyGraphResolver::dependencies(&workflow, &NodeName("c".into()));
        assert!(deps.is_empty());
    }

    #[test]
    fn dependencies_returns_single_predecessor() {
        let workflow = make_workflow(
            "test",
            vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0)],
            vec![("a", "b", EdgeCondition::Always)],
        );

        let deps = DependencyGraphResolver::dependencies(&workflow, &NodeName("b".into()));
        assert_eq!(deps.len(), 1);
        assert!(deps.contains(&NodeName("a".into())));
    }

    #[test]
    fn dependents_returns_single_successor() {
        let workflow = make_workflow(
            "test",
            vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0)],
            vec![("a", "b", EdgeCondition::Always)],
        );

        let succs = DependencyGraphResolver::dependents(&workflow, &NodeName("a".into()));
        assert_eq!(succs.len(), 1);
        assert!(succs.contains(&NodeName("b".into())));
    }

    #[test]
    fn ready_nodes_returns_source_nodes_when_nothing_completed() {
        let workflow = make_workflow(
            "test",
            vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0), ("c", 1, 0, 1.0)],
            vec![],
        );

        let ready = DependencyGraphResolver::ready_nodes(&workflow, &[]);
        assert_eq!(ready.len(), 3);
    }

    #[test]
    fn execution_layers_linear_chain() {
        let workflow = make_workflow(
            "test",
            vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0), ("c", 1, 0, 1.0)],
            vec![
                ("a", "b", EdgeCondition::Always),
                ("b", "c", EdgeCondition::Always),
            ],
        );

        let layers = DependencyGraphResolver::execution_layers(&workflow);
        assert_eq!(layers.len(), 3);
        assert_eq!(layers[0].len(), 1);
        assert!(layers[0].contains(&NodeName("a".into())));
        assert_eq!(layers[1].len(), 1);
        assert!(layers[1].contains(&NodeName("b".into())));
        assert_eq!(layers[2].len(), 1);
        assert!(layers[2].contains(&NodeName("c".into())));
    }
}
