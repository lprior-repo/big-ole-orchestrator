//! Cycle detection using DFS (white/gray/black algorithm).

use std::collections::HashMap;

use crate::error::WorkflowDefinitionError;
use crate::types::{DagNode, Edge, NodeName};

/// Detect a cycle in the given graph.
///
/// Returns `Some(cycle_nodes)` if a cycle is found (list of node names in path order),
/// or `None` if the graph is acyclic.
///
/// The detection uses DFS (white/gray/black algorithm) and returns the first cycle discovered.
/// The `cycle_nodes` ordering is deterministic based on DFS discovery order.
#[must_use]
pub fn detect_cycle(nodes: &[DagNode], edges: &[Edge]) -> Option<Vec<NodeName>> {
    if nodes.is_empty() {
        return None;
    }
    let adjacency = build_adjacency_list(nodes, edges);
    let mut color: HashMap<NodeName, usize> = nodes.iter().map(|n| (n.name.clone(), 0)).collect();
    let mut parent: HashMap<NodeName, NodeName> = HashMap::new();
    for node in nodes {
        if color.get(&node.name) == Some(&0) {
            if let Some(cycle) = dfs_visit(&node.name, &adjacency, &mut color, &mut parent) {
                return Some(cycle);
            }
        }
    }
    None
}

fn build_adjacency_list(nodes: &[DagNode], edges: &[Edge]) -> HashMap<NodeName, Vec<NodeName>> {
    let mut adjacency: HashMap<NodeName, Vec<NodeName>> =
        nodes.iter().map(|n| (n.name.clone(), Vec::new())).collect();
    for edge in edges {
        if let Some(targets) = adjacency.get_mut(&edge.source_node) {
            targets.push(edge.target_node.clone());
        }
    }
    adjacency
}

/// DFS visit helper using white/gray/black algorithm.
/// Returns `Some(cycle_nodes)` if cycle found, `None` otherwise.
fn dfs_visit(
    node: &NodeName,
    adjacency: &HashMap<NodeName, Vec<NodeName>>,
    color: &mut HashMap<NodeName, usize>,
    parent: &mut HashMap<NodeName, NodeName>,
) -> Option<Vec<NodeName>> {
    color.insert(node.clone(), 1);
    let neighbors = adjacency.get(node)?;
    for neighbor in neighbors {
        match color.get(neighbor) {
            Some(&1) => return Some(build_cycle_path(node, neighbor, parent)),
            Some(&0) => {
                parent.insert(neighbor.clone(), node.clone());
                if let Some(c) = dfs_visit(neighbor, adjacency, color, parent) {
                    return Some(c);
                }
            }
            _ => {}
        }
    }
    color.insert(node.clone(), 2);
    None
}

/// Build the cycle path by backtracking from node to neighbor through parent links.
fn build_cycle_path(
    node: &NodeName,
    neighbor: &NodeName,
    parent: &HashMap<NodeName, NodeName>,
) -> Vec<NodeName> {
    if node == neighbor {
        return vec![neighbor.clone()];
    }
    let mut cycle = vec![neighbor.clone()];
    let mut current = node.clone();
    while current.as_str() != neighbor.as_str() {
        cycle.push(current.clone());
        match parent.get(&current) {
            Some(p) => current = p.clone(),
            None => break,
        }
    }
    cycle.push(neighbor.clone());
    cycle.reverse();
    cycle
}

/// Validate that the graph has no cycles.
pub(crate) fn validate_no_cycles(
    nodes: &[DagNode],
    edges: &[Edge],
) -> Result<(), WorkflowDefinitionError> {
    detect_cycle(nodes, edges)
        .map(|cycle_nodes| WorkflowDefinitionError::CycleDetected { cycle_nodes })
        .map(Err)
        .unwrap_or(Ok(()))
}
