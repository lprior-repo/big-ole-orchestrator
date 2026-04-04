//! Dag workflow builder with cycle detection.

use std::collections::HashMap;

use crate::cycle::validate_no_cycles;
use crate::error::{RetryPolicyError, WorkflowDefinitionError};
use crate::types::{DagNode, Edge, NodeName, WorkflowDefinition};

// ============================================================================
// Validation
// ============================================================================

/// Validate that all edges reference existing nodes.
pub(crate) fn validate_edges_exist(
    _nodes: &[DagNode],
    edges: &[Edge],
    node_map: &HashMap<NodeName, DagNode>,
) -> Result<(), WorkflowDefinitionError> {
    edges
        .iter()
        .find(|edge| {
            !node_map.contains_key(&edge.source_node) || !node_map.contains_key(&edge.target_node)
        })
        .map(|edge| {
            let unknown = if !node_map.contains_key(&edge.source_node) {
                edge.source_node.clone()
            } else {
                edge.target_node.clone()
            };
            let known = if node_map.contains_key(&edge.source_node) {
                edge.source_node.clone()
            } else {
                edge.target_node.clone()
            };
            WorkflowDefinitionError::UnknownNode {
                edge_source: known,
                unknown_target: unknown,
            }
        })
        .map(Err)
        .unwrap_or(Ok(()))
}

pub(crate) fn validate_retry_policies(nodes: &[DagNode]) -> Result<(), WorkflowDefinitionError> {
    nodes
        .iter()
        .find_map(|node| {
            node.retry_policy.as_ref().and_then(|policy| {
                if policy.backoff_ms == 0 {
                    Some(WorkflowDefinitionError::InvalidRetryPolicy {
                        node_name: node.name.clone(),
                        reason: RetryPolicyError::NegativeBackoff(0),
                    })
                } else if policy.max_retries == u32::MAX {
                    Some(WorkflowDefinitionError::InvalidRetryPolicy {
                        node_name: node.name.clone(),
                        reason: RetryPolicyError::MaxRetriesExceeded(policy.max_retries),
                    })
                } else {
                    None
                }
            })
        })
        .map(Err)
        .unwrap_or(Ok(()))
}

// ============================================================================
// Dag Builder
// ============================================================================

/// Workflow builder that tracks nodes and edges.
#[derive(Debug, Clone, Default)]
pub struct Dag {
    name: String,
    nodes: HashMap<NodeName, DagNode>,
    edges: Vec<Edge>,
}

impl Dag {
    /// Create a new Dag with the given workflow name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            nodes: HashMap::new(),
            edges: Vec::new(),
        }
    }

    /// Add a node to the DAG.
    pub fn add_node(&mut self, node: DagNode) -> &mut Self {
        self.nodes.insert(node.name.clone(), node);
        self
    }

    /// Add an edge to the DAG.
    pub fn connect(&mut self, source: NodeName, target: NodeName) -> &mut Self {
        self.edges.push(Edge {
            source_node: source,
            target_node: target,
            condition: None,
        });
        self
    }

    /// Build and validate the DAG, running cycle detection.
    ///
    /// Returns `Ok(WorkflowDefinition)` if acyclic, `Err(WorkflowDefinitionError)` otherwise.
    ///
    /// # Errors
    /// Returns `WorkflowDefinitionError::EmptyWorkflow` if no nodes added.
    /// Returns `WorkflowDefinitionError::CycleDetected { cycle_nodes }` if cycle found.
    /// Returns `WorkflowDefinitionError::UnknownNode` if an edge references a non-existent node.
    /// Returns `WorkflowDefinitionError::InvalidRetryPolicy` if a node has an invalid retry policy.
    pub fn build(self) -> Result<WorkflowDefinition, WorkflowDefinitionError> {
        if self.nodes.is_empty() {
            return Err(WorkflowDefinitionError::EmptyWorkflow);
        }
        let nodes_vec: Vec<DagNode> = self.nodes.values().cloned().collect();
        validate_edges_exist(&nodes_vec, &self.edges, &self.nodes)?;
        validate_retry_policies(&nodes_vec)?;
        validate_no_cycles(&nodes_vec, &self.edges)?;
        Ok(WorkflowDefinition {
            workflow_name: self.name,
            nodes: nodes_vec,
            edges: self.edges,
        })
    }
}
