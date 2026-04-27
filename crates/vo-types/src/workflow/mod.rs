mod guarantee_class;
mod types;

pub use guarantee_class::GuaranteeClass;
pub use types::{DagNode, Edge, EdgeCondition, RetryPolicy, RetryPolicyError, StepOutcome};

use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::non_empty_vec::NonEmptyVec;
use crate::{NodeName, WorkflowName};

// ---------------------------------------------------------------------------
// WorkflowDefinitionError
// ---------------------------------------------------------------------------

/// Errors returned by `WorkflowDefinition::parse`.
///
/// NOTE: Serde deserialization errors do not implement `Clone` or `PartialEq`, so
/// `DeserializationFailed` stores the error's Display representation as a
/// `String`. `PartialEq` for `DeserializationFailed` compares only the discriminant.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum WorkflowDefinitionError {
    /// JSON could not be deserialized into the intermediate unvalidated struct.
    #[error("workflow definition deserialization failed: {message}")]
    DeserializationFailed { message: String },

    /// The nodes list is empty.
    #[error("workflow definition must contain at least one node")]
    EmptyWorkflow,

    /// The graph contains a cycle.
    #[error("workflow contains a cycle: {cycle_nodes:?}")]
    CycleDetected { cycle_nodes: Vec<NodeName> },

    /// An edge references a node name that does not exist in the nodes list.
    #[error("edge from '{edge_source}' references unknown node '{unknown_node}'")]
    UnknownNode {
        edge_source: NodeName,
        unknown_node: NodeName,
    },

    /// The nodes list contains duplicate node names.
    #[error("duplicate node name: '{node_name}' appears more than once")]
    DuplicateNode { node_name: NodeName },

    /// A `DagNode` contains an invalid `RetryPolicy`.
    #[error("node '{node_name}' has invalid retry policy: {reason}")]
    InvalidRetryPolicy {
        node_name: NodeName,
        reason: RetryPolicyError,
    },

    /// No node has in-degree zero (no entry point exists).
    #[error("workflow has no start node: every node has at least one incoming edge")]
    NoStartNode,

    /// One or more nodes have no edges and are disconnected from the graph.
    #[error("workflow contains orphan nodes with no edges: {orphan_nodes:?}")]
    OrphanNodes { orphan_nodes: Vec<NodeName> },

    /// One or more nodes are not reachable from any start node.
    #[error("workflow contains unreachable nodes: {unreachable_nodes:?}")]
    UnreachableNodes { unreachable_nodes: Vec<NodeName> },
}

// ---------------------------------------------------------------------------
// WorkflowDefinition
// ---------------------------------------------------------------------------

/// The complete, validated workflow DAG.
/// Guaranteed acyclic after construction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub workflow_name: WorkflowName,
    pub nodes: NonEmptyVec<DagNode>,
    pub edges: Vec<Edge>,
}

impl WorkflowDefinition {
    /// Parse from any serde `Deserializer`.
    ///
    /// This is the format-agnostic entry point. Production callers should
    /// supply a concrete deserializer (e.g. `serde_json::Deserializer`).
    ///
    /// Validation order:
    /// 1. Deserialization into intermediate struct
    /// 2. Non-empty nodes check
    /// 3. `RetryPolicy` validation per node
    /// 4. Edge referential integrity (source and target node names must exist)
    /// 5. DFS cycle detection
    /// 6. Connectivity validation (orphan nodes, unreachable nodes, start node)
    ///
    /// # Errors
    ///
    /// Returns `WorkflowDefinitionError` if deserialization fails, the graph is
    /// empty, contains invalid retry policies, unknown edge references, cycles,
    /// orphan nodes, unreachable nodes, or has no start node.
    pub fn from_deserializer<'de, D>(deserializer: D) -> Result<Self, WorkflowDefinitionError>
    where
        D: serde::de::Deserializer<'de>,
    {
        // Step 1: Deserialize into unvalidated intermediate struct
        let unvalidated: UnvalidatedWorkflow = serde::Deserialize::deserialize(deserializer)
            .map_err(|source| WorkflowDefinitionError::DeserializationFailed {
                message: source.to_string(),
            })?;

        Self::validate_unvalidated(unvalidated)
    }

    /// Parse a JSON byte slice into a validated WorkflowDefinition.
    ///
    /// Convenience wrapper that uses `serde_json::from_slice` internally.
    /// Only available in test builds; production code should call
    /// `from_deserializer` directly.
    #[cfg(test)]
    pub fn parse(json_bytes: &[u8]) -> Result<Self, WorkflowDefinitionError> {
        let unvalidated: UnvalidatedWorkflow =
            serde_json::from_slice(json_bytes).map_err(|source| {
                WorkflowDefinitionError::DeserializationFailed {
                    message: source.to_string(),
                }
            })?;

        Self::validate_unvalidated(unvalidated)
    }

    /// Look up a `DagNode` by `NodeName`. Returns `None` if not found.
    #[must_use]
    pub fn get_node(&self, name: &NodeName) -> Option<&DagNode> {
        self.nodes.as_slice().iter().find(|n| &n.node_name == name)
    }

    /// Run the full validation pipeline on an already-deserialized intermediate struct.
    ///
    /// Steps 2–5 from [`Self::from_deserializer`].
    fn validate_unvalidated(
        unvalidated: UnvalidatedWorkflow,
    ) -> Result<Self, WorkflowDefinitionError> {
        // Step 2: Non-empty nodes check
        if unvalidated.nodes.is_empty() {
            return Err(WorkflowDefinitionError::EmptyWorkflow);
        }

        // Step 2b: Check for duplicate node names
        let mut seen: HashMap<&NodeName, usize> = HashMap::new();
        for (i, node) in unvalidated.nodes.iter().enumerate() {
            if let Some(first_idx) = seen.get(&node.node_name) {
                return Err(WorkflowDefinitionError::DuplicateNode {
                    node_name: node.node_name.clone(),
                });
            }
            seen.insert(&node.node_name, i);
        }

        // Step 3: RetryPolicy validation per node
        unvalidated.nodes.iter().try_for_each(|node| {
            RetryPolicy::new(
                node.retry_policy.max_attempts,
                node.retry_policy.backoff_ms,
                node.retry_policy.backoff_multiplier,
            )
            .map_err(|reason| WorkflowDefinitionError::InvalidRetryPolicy {
                node_name: node.node_name.clone(),
                reason,
            })?;
            Ok::<(), WorkflowDefinitionError>(())
        })?;

        // Step 4: Edge referential integrity
        let node_names: HashSet<&NodeName> =
            unvalidated.nodes.iter().map(|n| &n.node_name).collect();
        unvalidated.edges.iter().try_for_each(|edge| {
            if !node_names.contains(&edge.source_node) {
                return Err(WorkflowDefinitionError::UnknownNode {
                    edge_source: edge.source_node.clone(),
                    unknown_node: edge.source_node.clone(),
                });
            }
            if !node_names.contains(&edge.target_node) {
                return Err(WorkflowDefinitionError::UnknownNode {
                    edge_source: edge.source_node.clone(),
                    unknown_node: edge.target_node.clone(),
                });
            }
            Ok(())
        })?;

        // Step 5: DFS cycle detection
        if let Some(cycle_nodes) = detect_cycle(&unvalidated.nodes, &unvalidated.edges) {
            return Err(WorkflowDefinitionError::CycleDetected { cycle_nodes });
        }

        // Step 6: Connectivity validation
        validate_connectivity(&unvalidated.nodes, &unvalidated.edges)?;

        // Construct validated definition
        Ok(WorkflowDefinition {
            workflow_name: unvalidated.workflow_name,
            nodes: NonEmptyVec::new_unchecked(unvalidated.nodes),
            edges: unvalidated.edges,
        })
    }
}

// ---------------------------------------------------------------------------
// next_nodes
// ---------------------------------------------------------------------------

/// Pure function: find the successor nodes for a given current node and outcome.
#[must_use]
pub fn next_nodes<'a>(
    current: &NodeName,
    last_outcome: StepOutcome,
    def: &'a WorkflowDefinition,
) -> Vec<&'a DagNode> {
    def.edges
        .iter()
        .filter(|edge| &edge.source_node == current && edge.condition.matches(last_outcome))
        .filter_map(|edge| def.get_node(&edge.target_node))
        .collect()
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Intermediate struct for JSON deserialization without validation.
/// `RetryPolicy` and edge references are validated after deserialization.
#[derive(Deserialize)]
struct UnvalidatedWorkflow {
    workflow_name: WorkflowName,
    nodes: Vec<DagNode>,
    edges: Vec<Edge>,
}

/// DFS-based cycle detection. Returns `Some(cycle_nodes)` if a cycle is found,
/// where `cycle_nodes` contains the path from the cycle start back to itself
/// (first node repeated at end). Returns None if the graph is acyclic.
fn detect_cycle(nodes: &[DagNode], edges: &[Edge]) -> Option<Vec<NodeName>> {
    // Build adjacency list: node_name -> [target_node_names]
    let mut adj: HashMap<&NodeName, Vec<&NodeName>> = HashMap::new();
    for node in nodes {
        adj.entry(&node.node_name).or_default();
    }
    for edge in edges {
        adj.entry(&edge.source_node)
            .or_default()
            .push(&edge.target_node);
    }

    // DFS state: 0 = unvisited, 1 = in-progress (on current path), 2 = done
    let mut state: HashMap<&NodeName, u8> = HashMap::new();
    let mut path: Vec<NodeName> = Vec::new();

    dfs_cycle(&nodes[0].node_name, &adj, &mut state, &mut path).or_else(|| {
        // Check remaining unvisited nodes (disconnected components)
        nodes[1..].iter().find_map(|node| {
            if state.get(&node.node_name).copied().is_none() {
                dfs_cycle(&node.node_name, &adj, &mut state, &mut path)
            } else {
                None
            }
        })
    })
}

/// Recursive DFS that detects back-edges indicating cycles.
fn dfs_cycle<'a>(
    current: &'a NodeName,
    adj: &HashMap<&'a NodeName, Vec<&'a NodeName>>,
    state: &mut HashMap<&'a NodeName, u8>,
    path: &mut Vec<NodeName>,
) -> Option<Vec<NodeName>> {
    match state.get(current).copied() {
        Some(1) => {
            // Current node is already in the DFS path — cycle found.
            let start_idx = path.iter().position(|n| n == current)?;
            let cycle: Vec<NodeName> = path[start_idx..]
                .iter()
                .chain(std::iter::once(current))
                .cloned()
                .collect();
            Some(cycle)
        }
        Some(2) => None,
        _ => {
            // Unvisited: explore neighbors.
            state.insert(current, 1);
            path.push(current.clone());

            if let Some(neighbors) = adj.get(current) {
                if let Some(cycle) = neighbors
                    .iter()
                    .find_map(|neighbor| dfs_cycle(neighbor, adj, state, path))
                {
                    return Some(cycle);
                }
            }

            path.pop();
            state.insert(current, 2);
            None
        }
    }
}

fn validate_connectivity(
    nodes: &[DagNode],
    edges: &[Edge],
) -> Result<(), WorkflowDefinitionError> {
    if nodes.len() <= 1 || edges.is_empty() {
        return Ok(());
    }

    let node_names: HashSet<&NodeName> = nodes.iter().map(|n| &n.node_name).collect();

    let mut in_degree: HashMap<&NodeName, usize> = HashMap::new();
    let mut out_degree: HashMap<&NodeName, usize> = HashMap::new();
    for name in &node_names {
        in_degree.entry(name).or_insert(0);
        out_degree.entry(name).or_insert(0);
    }
    for edge in edges {
        *out_degree.entry(&edge.source_node).or_insert(0) += 1;
        *in_degree.entry(&edge.target_node).or_insert(0) += 1;
    }

    let orphan_nodes: Vec<NodeName> = nodes
        .iter()
        .filter(|n| in_degree[&n.node_name] == 0 && out_degree[&n.node_name] == 0)
        .map(|n| n.node_name.clone())
        .collect();
    if !orphan_nodes.is_empty() {
        return Err(WorkflowDefinitionError::OrphanNodes { orphan_nodes });
    }

    let start_nodes: Vec<&NodeName> = node_names
        .iter()
        .filter(|name| in_degree[*name] == 0)
        .copied()
        .collect();

    if start_nodes.is_empty() {
        return Err(WorkflowDefinitionError::NoStartNode);
    }

    let mut adj: HashMap<&NodeName, Vec<&NodeName>> = HashMap::new();
    for edge in edges {
        adj.entry(&edge.source_node).or_default().push(&edge.target_node);
    }

    let mut reachable: HashSet<&NodeName> = HashSet::new();
    let mut queue: VecDeque<&NodeName> = start_nodes.iter().copied().collect();
    for name in &start_nodes {
        reachable.insert(name);
    }
    while let Some(current) = queue.pop_front() {
        if let Some(neighbors) = adj.get(current) {
            for neighbor in neighbors {
                if reachable.insert(neighbor) {
                    queue.push_back(neighbor);
                }
            }
        }
    }

    let unreachable_nodes: Vec<NodeName> = node_names
        .iter()
        .filter(|name| !reachable.contains(*name))
        .map(|name| (*name).clone())
        .collect();
    if !unreachable_nodes.is_empty() {
        return Err(WorkflowDefinitionError::UnreachableNodes { unreachable_nodes });
    }

    Ok(())
}
