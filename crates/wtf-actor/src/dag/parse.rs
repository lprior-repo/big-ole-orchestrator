//! Parse `graph_raw` JSON into a `HashMap<NodeId, DagNode>`.
//!
//! All functions are pure calculations (no I/O, no async).
//! Cycle detection uses Kahn's algorithm — the one bounded mutation
//! is isolated in [`kahn_run`] and is strictly necessary for
//! topological ordering computation.

use std::collections::{HashMap, HashSet, VecDeque};

use super::state::{DagNode, NodeId};

/// Error returned when parsing a DAG graph definition.
#[derive(Debug, thiserror::Error)]
pub enum DagParseError {
    #[error("invalid JSON in graph_raw: {0}")]
    InvalidJson(String),

    #[error("graph_raw must be a JSON object with a 'nodes' field")]
    MissingNodesField,

    #[error("'nodes' must be an array")]
    NodesNotArray,

    #[error("duplicate node id: {0}")]
    DuplicateNodeId(String),

    #[error("node '{node_id}' has unknown predecessor '{predecessor_id}'")]
    UnknownPredecessor {
        node_id: String,
        predecessor_id: String,
    },

    #[error("cycle detected involving nodes: {0}")]
    CycleDetected(String),

    #[error("node at index {index} is missing required field '{field}'")]
    MissingNodeField { index: usize, field: &'static str },
}

/// Parse a `graph_raw` JSON string into a `HashMap<NodeId, DagNode>`.
///
/// Expected JSON format:
/// ```json
/// {
///   "nodes": [
///     { "id": "A", "activity_type": "fetch", "predecessors": [] },
///     { "id": "B", "activity_type": "transform", "predecessors": ["A"] }
///   ]
/// }
/// ```
pub fn parse_dag_graph(graph_raw: &str) -> Result<HashMap<NodeId, DagNode>, DagParseError> {
    let root = serde_json::from_str::<serde_json::Value>(graph_raw)
        .map_err(|e| DagParseError::InvalidJson(e.to_string()))?;

    let nodes_array = root
        .get("nodes")
        .ok_or(DagParseError::MissingNodesField)?
        .as_array()
        .ok_or(DagParseError::NodesNotArray)?;

    let node_map = extract_nodes(nodes_array)?;
    validate_predecessors(&node_map)?;
    detect_cycles(&node_map)?;

    Ok(node_map)
}

/// Extract nodes from the JSON array into a `HashMap`.
///
/// Each node must have `id`, `activity_type`, and `predecessors` fields.
fn extract_nodes(nodes: &[serde_json::Value]) -> Result<HashMap<NodeId, DagNode>, DagParseError> {
    nodes
        .iter()
        .enumerate()
        .try_fold(HashMap::new(), |mut acc, (index, value)| {
            let obj = value.as_object().ok_or(DagParseError::MissingNodeField {
                index,
                field: "(object)",
            })?;

            let id = required_str(obj, "id", index)?;
            let activity_type = required_str(obj, "activity_type", index)?;
            let pred_array = required_array(obj, "predecessors", index)?;

            let pred_ids: Vec<NodeId> = pred_array
                .iter()
                .filter_map(|v| v.as_str().map(NodeId::new))
                .collect();

            let node_id = NodeId::new(id);

            if acc.contains_key(&node_id) {
                return Err(DagParseError::DuplicateNodeId(id.to_owned()));
            }

            acc.insert(
                node_id,
                DagNode {
                    activity_type: activity_type.to_owned(),
                    predecessors: pred_ids,
                },
            );
            Ok(acc)
        })
}

/// Extract a required string field from a JSON object.
fn required_str<'a>(
    obj: &'a serde_json::Map<String, serde_json::Value>,
    field: &'static str,
    index: usize,
) -> Result<&'a str, DagParseError> {
    obj.get(field)
        .and_then(|v| v.as_str())
        .ok_or(DagParseError::MissingNodeField { index, field })
}

/// Extract a required array field from a JSON object.
fn required_array<'a>(
    obj: &'a serde_json::Map<String, serde_json::Value>,
    field: &'static str,
    index: usize,
) -> Result<&'a [serde_json::Value], DagParseError> {
    obj.get(field)
        .and_then(|v| v.as_array())
        .map(std::vec::Vec::as_slice)
        .ok_or(DagParseError::MissingNodeField { index, field })
}

/// Validate that every predecessor reference points to an existing node.
fn validate_predecessors(node_map: &HashMap<NodeId, DagNode>) -> Result<(), DagParseError> {
    let ids: HashSet<&NodeId> = node_map.keys().collect();

    let first_bad = node_map.iter().find_map(|(node_id, node)| {
        node.predecessors.iter().find_map(|pred| {
            (!ids.contains(pred)).then_some(DagParseError::UnknownPredecessor {
                node_id: node_id.as_str().to_owned(),
                predecessor_id: pred.as_str().to_owned(),
            })
        })
    });

    first_bad.map_or(Ok(()), Err)
}

/// Detect cycles using Kahn's algorithm.
///
/// If the topological sort cannot process all nodes, a cycle exists.
fn detect_cycles(node_map: &HashMap<NodeId, DagNode>) -> Result<(), DagParseError> {
    if node_map.is_empty() {
        return Ok(());
    }

    let successors = build_successor_index(node_map);
    let in_degree = compute_in_degree(node_map);
    let total = node_map.len();

    let (processed, remaining) = kahn_run(&in_degree, &successors, total);

    if processed < total {
        let msg = remaining.join(", ");
        return Err(DagParseError::CycleDetected(msg));
    }

    Ok(())
}

/// Build a successor index (reverse adjacency list) from predecessor refs.
fn build_successor_index(node_map: &HashMap<NodeId, DagNode>) -> HashMap<&NodeId, Vec<&NodeId>> {
    node_map
        .iter()
        .fold(HashMap::new(), |mut acc, (node_id, node)| {
            for pred in &node.predecessors {
                if let Some((k, _)) = node_map.get_key_value(pred) {
                    acc.entry(k).or_default().push(node_id);
                }
            }
            acc
        })
}

/// Compute in-degree for each node from predecessor references.
///
/// For each node, its in-degree equals the number of predecessors it has
/// (each predecessor represents an incoming edge).
fn compute_in_degree(node_map: &HashMap<NodeId, DagNode>) -> HashMap<&NodeId, usize> {
    node_map
        .iter()
        .map(|(id, node)| (id, node.predecessors.len()))
        .collect()
}

/// Run Kahn's algorithm.
///
/// Returns `(processed_count, unprocessed_node_ids)`.
/// If `processed_count < total`, the unprocessed nodes form one or more cycles.
///
/// This is the single bounded-mutation point: Kahn's algorithm fundamentally
/// requires updating in-degrees and draining a queue. The mutation is
/// confined to local variables within this function (~20 lines).
fn kahn_run(
    in_degree: &HashMap<&NodeId, usize>,
    successors: &HashMap<&NodeId, Vec<&NodeId>>,
    total: usize,
) -> (usize, Vec<String>) {
    let mut degrees: HashMap<&NodeId, usize> = in_degree.iter().map(|(&k, &v)| (k, v)).collect();

    let mut queue: VecDeque<&NodeId> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(&id, _)| id)
        .collect();

    let mut processed = 0_usize;
    while processed < total {
        let Some(node) = queue.pop_front() else {
            break;
        };
        processed += 1;
        for &succ in successors
            .get(node)
            .map_or(&[] as &[&NodeId], |s| s.as_slice())
        {
            if let Some(d) = degrees.get_mut(succ) {
                *d = d.saturating_sub(1);
                if *d == 0 {
                    queue.push_back(succ);
                }
            }
        }
    }

    let remaining: Vec<String> = degrees
        .iter()
        .filter(|&(_, &deg)| deg > 0)
        .map(|(&id, _)| id.as_str().to_owned())
        .collect();

    (processed, remaining)
}
