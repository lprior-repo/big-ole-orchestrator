use std::collections::HashSet;

use crate::{NodeName, StepOutcome, WorkflowDefinition};

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
    /// Returns empty if `node` is not found in the workflow.
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
    /// Returns empty if `node` is not found in the workflow.
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
        visited.insert(node.clone());

        let mut queue = vec![node.clone()];
        while let Some(current) = queue.pop() {
            let direct_deps = Self::dependencies(workflow, &current);
            for dep in direct_deps {
                if visited.contains(&dep) {
                    // Back-edge detected (cycle) — return empty as signal.
                    return vec![];
                }
                visited.insert(dep.clone());
                result.push(dep.clone());
                queue.push(dep);
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
        visited.insert(node.clone());

        let mut queue = vec![node.clone()];
        while let Some(current) = queue.pop() {
            let direct_deps = Self::dependents(workflow, &current);
            for dep in direct_deps {
                if visited.contains(&dep) {
                    // Back-edge detected (cycle) — return empty as signal.
                    return vec![];
                }
                visited.insert(dep.clone());
                result.push(dep.clone());
                queue.push(dep);
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
                let incoming: Vec<_> = workflow
                    .edges
                    .iter()
                    .filter(|edge| &edge.target_node == node_name)
                    .collect();

                if incoming.is_empty() {
                    return Some(node_name.clone());
                }

                let all_satisfied = incoming.iter().all(|edge| {
                    completed_set.contains(&edge.source_node)
                        && edge.condition.matches(last_outcome)
                });

                if all_satisfied {
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
}
