use dioxus::prelude::*;
use oya_frontend::graph::workflow_node::WorkflowNode;
use oya_frontend::graph::{Connection, ExecutionState, Node, NodeId};
use std::collections::HashMap;

use crate::ui::editor_interactions::{NODE_HEIGHT, NODE_WIDTH};
use crate::ui::parallel_group_overlay::{AggregateStatus, BoundingBox, ParallelGroup};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

const BEND_CLAMP: f32 = 200.0;

#[derive(Clone, Copy, PartialEq)]
struct EdgeAnchor {
    from: Position,
    to: Position,
}

#[derive(Clone)]
pub struct DragState {
    pub edge_id: String,
    pub start_page_y: f32,
    pub start_bend: f32,
}

fn get_source_point(node: &Node) -> Position {
    Position {
        x: node.x + NODE_WIDTH,
        y: node.y + NODE_HEIGHT / 2.0,
    }
}

fn get_target_point(node: &Node) -> Position {
    Position {
        x: node.x,
        y: node.y + NODE_HEIGHT / 2.0,
    }
}

fn create_smooth_step_path(from: Position, to: Position, bend_y: f32) -> (String, Position) {
    let mid_y = f32::midpoint(from.y, to.y) + bend_y.clamp(-BEND_CLAMP, BEND_CLAMP);
    let radius: f32 = 8.0;

    let dx = to.x - from.x;
    let dy = to.y - from.y;

    if dx.abs() < 2.0 || !dx.is_finite() || !dy.is_finite() {
        return (
            format!("M {} {} L {} {}", from.x, from.y, to.x, to.y),
            Position {
                x: f32::midpoint(from.x, to.x),
                y: mid_y,
            },
        );
    }

    let sign_x = if dx > 0.0 { 1.0 } else { -1.0 };
    let r = radius.min(dx.abs() / 2.0).min(dy.abs() / 4.0);

    (
        format!(
            "M {fx} {fy} L {fx} {my_r} Q {fx} {my} {fx_r} {my} L {tx_r} {my} Q {tx} {my} {tx} {my_r2} L {tx} {ty}",
            fx = from.x,
            fy = from.y,
            my = mid_y,
            my_r = mid_y - r,
            my_r2 = mid_y + r,
            fx_r = from.x + sign_x * r,
            tx_r = to.x - sign_x * r,
            tx = to.x,
            ty = to.y
        ),
        Position {
            x: f32::midpoint(from.x, to.x),
            y: mid_y,
        },
    )
}

fn resolve_edge_anchors(edges: &[Connection], nodes: &[Node]) -> HashMap<String, EdgeAnchor> {
    let node_by_id: HashMap<_, _> = nodes.iter().map(|node| (node.id, node.clone())).collect();

    edges
        .iter()
        .filter_map(|edge| {
            let source = node_by_id.get(&edge.source)?;
            let target = node_by_id.get(&edge.target)?;
            let from = get_source_point(source);
            let to = get_target_point(target);
            Some((edge.id.to_string(), EdgeAnchor { from, to }))
        })
        .collect()
}

fn resolve_edge_anchors_with_parallel(
    edges: &[Connection],
    nodes: &[Node],
    parallel_groups: &[ParallelGroup],
) -> HashMap<String, EdgeAnchor> {
    let node_by_id: HashMap<_, _> = nodes.iter().map(|node| (node.id, node.clone())).collect();

    edges
        .iter()
        .filter_map(|edge| {
            let source = node_by_id.get(&edge.source)?;
            let target = node_by_id.get(&edge.target)?;
            let from = get_source_point(source);
            let to = get_target_point(target);

            let group = parallel_groups.iter().find(|g| {
                g.parallel_node_id == edge.source
                    && g.branch_node_ids.iter().any(|id| *id == edge.target)
            });

            let adjusted_to = group.map_or(to, |g| {
                let branch_nodes: Vec<Node> = g
                    .branch_node_ids
                    .iter()
                    .filter_map(|id| node_by_id.get(id).cloned())
                    .collect();
                let offset = calculate_parallel_offset(&edge.target, &branch_nodes, NODE_HEIGHT);
                Position {
                    x: to.x,
                    y: to.y + offset,
                }
            });

            Some((
                edge.id.to_string(),
                EdgeAnchor {
                    from,
                    to: adjusted_to,
                },
            ))
        })
        .collect()
}

#[allow(clippy::cast_precision_loss)]
fn calculate_parallel_offset(target_id: &NodeId, targets: &[Node], node_height: f32) -> f32 {
    let mut sorted: Vec<_> = targets.iter().enumerate().collect();
    sorted.sort_by(|a, b| a.1.id.0.cmp(&b.1.id.0));

    let idx = sorted
        .iter()
        .position(|(_, n)| n.id == *target_id)
        .unwrap_or(0);

    let spacing = node_height / 2.5;
    (idx as f32 - (sorted.len() as f32 - 1.0) / 2.0) * spacing
}

fn find_parallel_branches(nodes: &[Node], connections: &[Connection]) -> Vec<ParallelGroup> {
    // Only consider explicit WorkflowNode::Parallel nodes as sources for parallel groups
    let parallel_node_ids: Vec<NodeId> = nodes
        .iter()
        .filter(|node| matches!(node.node, WorkflowNode::Parallel(_)))
        .map(|node| node.id)
        .collect();

    let mut source_targets: HashMap<NodeId, std::collections::HashSet<NodeId>> = HashMap::new();

    for connection in connections {
        // Only include connections from explicit Parallel nodes
        if parallel_node_ids.contains(&connection.source) {
            source_targets
                .entry(connection.source)
                .or_default()
                .insert(connection.target);
        }
    }

    let node_by_id: HashMap<_, _> = nodes.iter().map(|node| (node.id, node.clone())).collect();

    source_targets
        .into_iter()
        .filter_map(|(source_id, target_ids)| {
            if target_ids.len() < 2 {
                return None;
            }

            let mut target_nodes: Vec<Node> = target_ids
                .iter()
                .copied()
                .filter_map(|id| node_by_id.get(&id).cloned())
                .collect();

            target_nodes.sort_by(|a, b| a.id.0.cmp(&b.id.0));

            let min_y = target_nodes
                .iter()
                .map(|n| n.y)
                .fold(f32::INFINITY, f32::min);
            let max_y = target_nodes
                .iter()
                .map(|n| n.y + NODE_HEIGHT)
                .fold(f32::NEG_INFINITY, f32::max);
            let min_x = target_nodes
                .iter()
                .map(|n| n.x)
                .fold(f32::INFINITY, f32::min);
            let max_x = target_nodes
                .iter()
                .map(|n| n.x + NODE_WIDTH)
                .fold(f32::NEG_INFINITY, f32::max);

            let bounds = BoundingBox {
                x: min_x - 8.0,
                y: min_y - 8.0,
                width: (max_x - min_x) + 16.0,
                height: (max_y - min_y) + 16.0,
            };

            Some(ParallelGroup {
                parallel_node_id: source_id,
                branch_node_ids: target_nodes.iter().map(|n| n.id).collect(),
                bounding_box: bounds,
                branch_count: target_nodes.len(),
                aggregate_status: AggregateStatus::Pending,
            })
        })
        .collect()
}

fn sanitize_bend_input_edge(input: f32, start_bend: f32) -> f32 {
    if !input.is_finite() {
        return start_bend;
    }
    input.clamp(-BEND_CLAMP, BEND_CLAMP)
}

fn normalize_bend_delta(page_delta: f32, zoom: f32) -> f32 {
    if !zoom.is_finite() || zoom <= 0.0 {
        return 0.0;
    }
    page_delta / zoom
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Rect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[cfg(test)]
mod tests {
    use super::{
        calculate_parallel_offset, find_parallel_branches, normalize_bend_delta,
        resolve_edge_anchors_with_parallel, AggregateStatus, BoundingBox, ParallelGroup, Rect,
    };
    use oya_frontend::graph::{Connection, ExecutionState, Node, NodeId, PortName, WorkflowNode};
    use uuid::Uuid;

    // Constants for test data builders
    const NODE_HEIGHT: f32 = 68.0;

    // ==================== Test Data Builders ====================

    fn build_node(id: NodeId, x: f32, y: f32) -> Node {
        let mut node = Node::from_workflow_node(
            format!("Node {}", id),
            WorkflowNode::Run(oya_frontend::graph::workflow_node::RunConfig::default()),
            x,
            y,
        );
        node.id = id;
        node
    }

    /// Build a Parallel node for testing
    fn build_parallel_node(id: NodeId, x: f32, y: f32) -> Node {
        let mut node = Node::from_workflow_node(
            format!("Parallel {}", id),
            WorkflowNode::Parallel(oya_frontend::graph::workflow_node::ParallelConfig::default()),
            x,
            y,
        );
        node.id = id;
        node
    }

    fn build_connection(id: Uuid, source: NodeId, target: NodeId) -> Connection {
        Connection {
            id,
            source,
            target,
            source_port: PortName::from("out"),
            target_port: PortName::from("in"),
        }
    }

    fn build_node_with_id(id: NodeId, x: f32, y: f32) -> Node {
        let mut node = build_node(id, x, y);
        node.id = id;
        node
    }

    // ==================== find_parallel_branches Tests ====================

    #[test]
    fn given_source_with_two_targets_when_find_parallel_then_returns_one_group() {
        let source_id = NodeId::new();
        let target_a_id = NodeId::new();
        let target_b_id = NodeId::new();

        let source = build_parallel_node(source_id, 100.0, 100.0);
        let target_a = build_node(target_a_id, 300.0, 100.0);
        let target_b = build_node(target_b_id, 300.0, 200.0);

        let nodes = vec![source.clone(), target_a.clone(), target_b.clone()];

        let conn_a = build_connection(Uuid::new_v4(), source_id, target_a_id);
        let conn_b = build_connection(Uuid::new_v4(), source_id, target_b_id);
        let connections = vec![conn_a, conn_b];

        let groups = find_parallel_branches(&nodes, &connections);

        assert_eq!(groups.len(), 1);
        let group = &groups[0];

        assert_eq!(group.parallel_node_id, source_id);
        assert_eq!(group.branch_node_ids.len(), 2);
        // Target nodes are sorted by ID lexicographically
        let mut sorted_ids = [target_a_id, target_b_id];
        sorted_ids.sort_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(group.branch_node_ids[0], sorted_ids[0]);
        assert_eq!(group.branch_node_ids[1], sorted_ids[1]);
        assert_eq!(group.bounding_box.x, 292.0);
        assert_eq!(group.bounding_box.y, 92.0);
        assert_eq!(group.bounding_box.width, 236.0);
        assert_eq!(group.bounding_box.height, 184.0);
    }

    #[test]
    fn given_source_with_three_targets_when_find_parallel_then_returns_one_group() {
        let source_id = NodeId::new();
        let target_a_id = NodeId::new();
        let target_b_id = NodeId::new();
        let target_c_id = NodeId::new();

        let source = build_parallel_node(source_id, 100.0, 100.0);
        let target_a = build_node(target_a_id, 300.0, 100.0);
        let target_b = build_node(target_b_id, 300.0, 200.0);
        let target_c = build_node(target_c_id, 300.0, 300.0);

        let nodes = vec![source, target_a, target_b, target_c];

        let conn_a = build_connection(Uuid::new_v4(), source_id, target_a_id);
        let conn_b = build_connection(Uuid::new_v4(), source_id, target_b_id);
        let conn_c = build_connection(Uuid::new_v4(), source_id, target_c_id);
        let connections = vec![conn_a, conn_b, conn_c];

        let groups = find_parallel_branches(&nodes, &connections);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].branch_node_ids.len(), 3);
    }

    #[test]
    fn given_source_with_many_targets_when_find_parallel_then_returns_one_group() {
        let source_id = NodeId::new();
        let mut target_ids = vec![];
        let mut nodes = vec![];
        let mut connections = vec![];

        for i in 0..5 {
            let target_id = NodeId::new();
            target_ids.push(target_id);
            nodes.push(build_node(target_id, 300.0, 100.0 + (i as f32) * 100.0));
            connections.push(build_connection(Uuid::new_v4(), source_id, target_id));
        }

        let source = build_parallel_node(source_id, 100.0, 100.0);
        nodes.insert(0, source);

        let groups = find_parallel_branches(&nodes, &connections);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].branch_node_ids.len(), 5);
    }

    #[test]
    fn given_single_connection_when_find_parallel_then_returns_empty_vec() {
        let source_id = NodeId::new();
        let target_id = NodeId::new();

        let source = build_node(source_id, 100.0, 100.0);
        let target = build_node(target_id, 300.0, 100.0);

        let nodes = vec![source, target];

        let connection = build_connection(Uuid::new_v4(), source_id, target_id);
        let connections = vec![connection];

        let groups = find_parallel_branches(&nodes, &connections);

        assert!(groups.is_empty());
    }

    #[test]
    fn given_empty_connections_when_find_parallel_then_returns_empty_vec() {
        let nodes = vec![];
        let connections = vec![];

        let groups = find_parallel_branches(&nodes, &connections);

        assert!(groups.is_empty());
    }

    #[test]
    fn given_empty_nodes_when_find_parallel_then_returns_empty_vec() {
        let nodes = vec![];
        let source_id = NodeId::new();
        let target_id = NodeId::new();

        let connection = build_connection(Uuid::new_v4(), source_id, target_id);
        let connections = vec![connection];

        let groups = find_parallel_branches(&nodes, &connections);

        assert!(groups.is_empty());
    }

    #[test]
    fn given_many_non_parallel_sources_when_find_parallel_then_returns_empty_vec() {
        let mut nodes = vec![];
        let mut connections = vec![];

        for i in 0..10 {
            let source_id = NodeId::new();
            let target_id = NodeId::new();

            let source = build_node(source_id, 100.0, (i as f32) * 200.0);
            let target = build_node(target_id, 300.0, (i as f32) * 200.0);

            nodes.push(source);
            nodes.push(target);

            connections.push(build_connection(Uuid::new_v4(), source_id, target_id));
        }

        let groups = find_parallel_branches(&nodes, &connections);

        assert!(groups.is_empty());
    }

    #[test]
    fn given_duplicate_connections_when_find_parallel_then_treats_as_single_connection() {
        let source_id = NodeId::new();
        let target_id = NodeId::new();

        let source = build_node(source_id, 100.0, 100.0);
        let target = build_node(target_id, 300.0, 100.0);

        let nodes = vec![source, target];

        // Two connections from same source to same target
        let conn_a = build_connection(Uuid::new_v4(), source_id, target_id);
        let conn_b = build_connection(Uuid::new_v4(), source_id, target_id);
        let connections = vec![conn_a, conn_b];

        let groups = find_parallel_branches(&nodes, &connections);

        assert!(groups.is_empty());
    }

    #[test]
    fn given_mixed_parallel_and_non_parallel_when_find_parallel_then_only_parallel_returned() {
        let source_a_id = NodeId::new();
        let source_b_id = NodeId::new();
        let target_a1_id = NodeId::new();
        let target_a2_id = NodeId::new();
        let target_b1_id = NodeId::new();

        let source_a = build_parallel_node(source_a_id, 100.0, 100.0);
        let source_b = build_node(source_b_id, 100.0, 300.0);
        let target_a1 = build_node(target_a1_id, 300.0, 100.0);
        let target_a2 = build_node(target_a2_id, 300.0, 200.0);
        let target_b1 = build_node(target_b1_id, 300.0, 300.0);

        let nodes = vec![source_a, source_b, target_a1, target_a2, target_b1];

        let conn_a1 = build_connection(Uuid::new_v4(), source_a_id, target_a1_id);
        let conn_a2 = build_connection(Uuid::new_v4(), source_a_id, target_a2_id);
        let conn_b1 = build_connection(Uuid::new_v4(), source_b_id, target_b1_id);
        let connections = vec![conn_a1, conn_a2, conn_b1];

        let groups = find_parallel_branches(&nodes, &connections);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].branch_node_ids.len(), 2);
    }

    // ==================== calculate_parallel_offset Tests ====================

    #[test]
    fn given_two_targets_when_calculate_offset_then_returns_symmetric_values() {
        let target_a_id = NodeId::new();
        let target_b_id = NodeId::new();

        let target_a = build_node(target_a_id, 300.0, 100.0);
        let target_b = build_node(target_b_id, 300.0, 200.0);

        let targets = vec![target_a.clone(), target_b.clone()];

        let offset_a = calculate_parallel_offset(&target_a_id, &targets, NODE_HEIGHT);
        let offset_b = calculate_parallel_offset(&target_b_id, &targets, NODE_HEIGHT);

        let spacing = NODE_HEIGHT / 2.5;

        let mut sorted_ids = [target_a_id, target_b_id];
        sorted_ids.sort_by(|left, right| left.0.cmp(&right.0));

        let expected_a = if target_a_id == sorted_ids[0] {
            -spacing / 2.0
        } else {
            spacing / 2.0
        };
        let expected_b = -expected_a;

        assert_eq!(offset_a, expected_a);
        assert_eq!(offset_b, expected_b);
    }

    #[test]
    fn given_three_targets_when_calculate_offset_then_returns_centered_values() {
        let target_a_id = NodeId::new();
        let target_b_id = NodeId::new();
        let target_c_id = NodeId::new();

        let target_a = build_node(target_a_id, 300.0, 100.0);
        let target_b = build_node(target_b_id, 300.0, 200.0);
        let target_c = build_node(target_c_id, 300.0, 300.0);

        let targets = vec![target_a, target_b, target_c];

        let offset_a = calculate_parallel_offset(&target_a_id, &targets, NODE_HEIGHT);
        let offset_b = calculate_parallel_offset(&target_b_id, &targets, NODE_HEIGHT);
        let offset_c = calculate_parallel_offset(&target_c_id, &targets, NODE_HEIGHT);

        let spacing = NODE_HEIGHT / 2.5;

        let mut sorted_ids = [target_a_id, target_b_id, target_c_id];
        sorted_ids.sort_by(|left, right| left.0.cmp(&right.0));

        let expected_for = |id: NodeId| {
            if id == sorted_ids[0] {
                -spacing
            } else if id == sorted_ids[1] {
                0.0
            } else {
                spacing
            }
        };

        assert_eq!(offset_a, expected_for(target_a_id));
        assert_eq!(offset_b, expected_for(target_b_id));
        assert_eq!(offset_c, expected_for(target_c_id));
    }

    #[test]
    fn given_four_targets_when_calculate_offset_then_returns_symmetric_values() {
        let target_a_id = NodeId::new();
        let target_b_id = NodeId::new();
        let target_c_id = NodeId::new();
        let target_d_id = NodeId::new();

        let target_a = build_node(target_a_id, 300.0, 100.0);
        let target_b = build_node(target_b_id, 300.0, 200.0);
        let target_c = build_node(target_c_id, 300.0, 300.0);
        let target_d = build_node(target_d_id, 300.0, 400.0);

        let targets = vec![target_a, target_b, target_c, target_d];

        let offset_a = calculate_parallel_offset(&target_a_id, &targets, NODE_HEIGHT);
        let offset_b = calculate_parallel_offset(&target_b_id, &targets, NODE_HEIGHT);
        let offset_c = calculate_parallel_offset(&target_c_id, &targets, NODE_HEIGHT);
        let offset_d = calculate_parallel_offset(&target_d_id, &targets, NODE_HEIGHT);

        let spacing = NODE_HEIGHT / 2.5;

        let mut sorted_ids = [target_a_id, target_b_id, target_c_id, target_d_id];
        sorted_ids.sort_by(|left, right| left.0.cmp(&right.0));

        let expected_for = |id: NodeId| {
            if id == sorted_ids[0] {
                -spacing * 1.5
            } else if id == sorted_ids[1] {
                -spacing / 2.0
            } else if id == sorted_ids[2] {
                spacing / 2.0
            } else {
                spacing * 1.5
            }
        };

        assert_eq!(offset_a, expected_for(target_a_id));
        assert_eq!(offset_b, expected_for(target_b_id));
        assert_eq!(offset_c, expected_for(target_c_id));
        assert_eq!(offset_d, expected_for(target_d_id));
    }

    #[test]
    fn given_single_target_when_calculate_offset_then_returns_zero() {
        let target_id = NodeId::new();
        let target = build_node(target_id, 300.0, 100.0);

        let targets = vec![target];

        let offset = calculate_parallel_offset(&target_id, &targets, NODE_HEIGHT);

        assert_eq!(offset, 0.0);
    }

    #[test]
    fn given_target_id_not_in_targets_when_calculate_offset_then_returns_zero() {
        let target_id = NodeId::new();
        let other_id = NodeId::new();
        let target = build_node(other_id, 300.0, 100.0);

        let targets = vec![target];

        let offset = calculate_parallel_offset(&target_id, &targets, NODE_HEIGHT);

        assert_eq!(offset, 0.0);
    }

    #[test]
    fn given_targets_at_varying_y_positions_when_calculate_offset_then_respects_sorted_order() {
        let target_a_id = NodeId::new();
        let target_b_id = NodeId::new();
        let target_c_id = NodeId::new();

        // Create nodes with y-positions that don't match ID order
        let target_a = build_node(target_a_id, 300.0, 300.0); // y=300, but ID sorts first
        let target_b = build_node(target_b_id, 300.0, 100.0); // y=100, but ID sorts middle
        let target_c = build_node(target_c_id, 300.0, 200.0); // y=200, but ID sorts last

        let targets = vec![target_a, target_b, target_c];

        let offset_a = calculate_parallel_offset(&target_a_id, &targets, NODE_HEIGHT);
        let offset_b = calculate_parallel_offset(&target_b_id, &targets, NODE_HEIGHT);
        let offset_c = calculate_parallel_offset(&target_c_id, &targets, NODE_HEIGHT);

        // Offsets are determined by sorted ID order, not y-position
        let spacing = NODE_HEIGHT / 2.5;
        let mut sorted_ids = [target_a_id, target_b_id, target_c_id];
        sorted_ids.sort_by(|left, right| left.0.cmp(&right.0));

        let expected_for = |id: NodeId| {
            if id == sorted_ids[0] {
                -spacing
            } else if id == sorted_ids[1] {
                0.0
            } else {
                spacing
            }
        };

        assert_eq!(offset_a, expected_for(target_a_id));
        assert_eq!(offset_b, expected_for(target_b_id));
        assert_eq!(offset_c, expected_for(target_c_id));
    }

    // ==================== resolve_edge_anchors_with_parallel Tests ====================

    #[test]
    fn given_parallel_groups_when_resolve_anchors_then_offsets_applied_to_targets() {
        let source_id = NodeId::new();
        let target_a_id = NodeId::new();
        let target_b_id = NodeId::new();

        let source = build_node(source_id, 100.0, 100.0);
        let target_a = build_node(target_a_id, 300.0, 100.0);
        let target_b = build_node(target_b_id, 300.0, 200.0);

        let nodes = vec![source, target_a.clone(), target_b.clone()];

        let conn_a = build_connection(Uuid::new_v4(), source_id, target_a_id);
        let conn_b = build_connection(Uuid::new_v4(), source_id, target_b_id);
        let connections = vec![conn_a, conn_b];

        // Create parallel group
        let group = ParallelGroup {
            parallel_node_id: source_id,
            branch_node_ids: vec![target_a_id, target_b_id],
            bounding_box: BoundingBox {
                x: 292.0,
                y: 92.0,
                width: 16.0,
                height: 116.0,
            },
            branch_count: 2,
            aggregate_status: AggregateStatus::Pending,
        };
        let groups = vec![group];

        let anchors = resolve_edge_anchors_with_parallel(&connections, &nodes, &groups);

        let anchor_a = anchors.get(&connections[0].id.to_string()).copied();
        let anchor_b = anchors.get(&connections[1].id.to_string()).copied();

        assert!(anchor_a.is_some());
        assert!(anchor_b.is_some());

        let anchor_a = anchor_a.unwrap();
        let anchor_b = anchor_b.unwrap();

        let spacing = NODE_HEIGHT / 2.5;
        let mut sorted_ids = [target_a_id, target_b_id];
        sorted_ids.sort_by(|left, right| left.0.cmp(&right.0));

        let expected_offset_a = if target_a_id == sorted_ids[0] {
            -spacing / 2.0
        } else {
            spacing / 2.0
        };
        let expected_offset_b = -expected_offset_a;

        assert_eq!(anchor_a.from.x, 320.0); // source.x + NODE_WIDTH
        assert_eq!(anchor_a.from.y, 134.0); // source.y + NODE_HEIGHT / 2
        assert_eq!(anchor_a.to.y, 134.0 + expected_offset_a);

        assert_eq!(anchor_b.from.x, 320.0);
        assert_eq!(anchor_b.from.y, 134.0);
        assert_eq!(anchor_b.to.y, 234.0 + expected_offset_b);
    }

    #[test]
    fn given_non_parallel_edges_when_resolve_anchors_then_no_offsets_applied() {
        let source_id = NodeId::new();
        let target_id = NodeId::new();

        let source = build_node(source_id, 100.0, 100.0);
        let target = build_node(target_id, 300.0, 100.0);

        let nodes = vec![source, target];

        let connection = build_connection(Uuid::new_v4(), source_id, target_id);
        let connections = vec![connection.clone()];

        let groups: Vec<ParallelGroup> = vec![];

        let anchors = resolve_edge_anchors_with_parallel(&connections, &nodes, &groups);

        let anchor = anchors.get(&connection.id.to_string()).copied();

        assert!(anchor.is_some());
        let anchor = anchor.unwrap();

        // No offset applied since no parallel group
        assert_eq!(anchor.to.y, 134.0); // target.y + NODE_HEIGHT / 2
    }

    #[test]
    fn given_mixed_parallel_and_non_parallel_edges_when_resolve_anchors() {
        let source_id = NodeId::new();
        let target_a_id = NodeId::new();
        let target_b_id = NodeId::new();
        let target_c_id = NodeId::new();

        let source = build_node(source_id, 100.0, 100.0);
        let target_a = build_node(target_a_id, 300.0, 100.0);
        let target_b = build_node(target_b_id, 300.0, 200.0);
        let target_c = build_node(target_c_id, 300.0, 300.0);

        let nodes = vec![source, target_a.clone(), target_b.clone(), target_c.clone()];

        let conn_a = build_connection(Uuid::new_v4(), source_id, target_a_id);
        let conn_b = build_connection(Uuid::new_v4(), source_id, target_b_id);
        let conn_c = build_connection(Uuid::new_v4(), source_id, target_c_id);
        let connections = vec![conn_a.clone(), conn_b.clone(), conn_c.clone()];

        // Only target_a and target_b are in parallel group
        let group = ParallelGroup {
            parallel_node_id: source_id,
            branch_node_ids: vec![target_a_id, target_b_id],
            bounding_box: BoundingBox {
                x: 292.0,
                y: 92.0,
                width: 16.0,
                height: 116.0,
            },
            branch_count: 2,
            aggregate_status: AggregateStatus::Pending,
        };
        let groups = vec![group];

        let anchors = resolve_edge_anchors_with_parallel(&connections, &nodes, &groups);

        let anchor_a = anchors.get(&conn_a.id.to_string()).copied();
        let anchor_b = anchors.get(&conn_b.id.to_string()).copied();
        let anchor_c = anchors.get(&conn_c.id.to_string()).copied();

        let spacing = NODE_HEIGHT / 2.5;
        let mut sorted_ids = [target_a_id, target_b_id];
        sorted_ids.sort_by(|left, right| left.0.cmp(&right.0));

        let expected_offset_a = if target_a_id == sorted_ids[0] {
            -spacing / 2.0
        } else {
            spacing / 2.0
        };
        let expected_offset_b = -expected_offset_a;

        // Parallel edges have offsets
        assert_eq!(anchor_a.unwrap().to.y, 134.0 + expected_offset_a);
        assert_eq!(anchor_b.unwrap().to.y, 234.0 + expected_offset_b);

        // Non-parallel edge has no offset
        assert_eq!(anchor_c.unwrap().to.y, 334.0);
    }

    // ==================== Rect Tests ====================

    #[test]
    fn given_rect_when_created_then_has_correct_values() {
        let rect = Rect {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 50.0,
        };

        assert_eq!(rect.x, 10.0);
        assert_eq!(rect.y, 20.0);
        assert_eq!(rect.width, 100.0);
        assert_eq!(rect.height, 50.0);
    }

    // ==================== Integration Tests ====================

    #[test]
    fn given_workflow_with_parallel_branches_when_full_pipeline_then_correct_output() {
        // Complete workflow: nodes + connections -> parallel groups -> edge anchors

        let source_id = NodeId::new();
        let target_a_id = NodeId::new();
        let target_b_id = NodeId::new();

        let source = build_parallel_node(source_id, 100.0, 100.0);
        let target_a = build_node(target_a_id, 300.0, 100.0);
        let target_b = build_node(target_b_id, 300.0, 200.0);

        let nodes = vec![source, target_a, target_b];

        let conn_a = build_connection(Uuid::new_v4(), source_id, target_a_id);
        let conn_b = build_connection(Uuid::new_v4(), source_id, target_b_id);
        let connections = vec![conn_a.clone(), conn_b.clone()];

        // Step 1: Find parallel groups
        let groups = find_parallel_branches(&nodes, &connections);
        assert_eq!(groups.len(), 1);

        // Step 2: Resolve edge anchors with parallel groups
        let anchors = resolve_edge_anchors_with_parallel(&connections, &nodes, &groups);

        // Step 3: Verify anchors exist and have correct structure
        assert_eq!(anchors.len(), 2);

        let anchor_a = anchors.get(&conn_a.id.to_string()).copied().unwrap();
        let anchor_b = anchors.get(&conn_b.id.to_string()).copied().unwrap();

        // Both anchors start from same source point
        assert_eq!(anchor_a.from.x, anchor_b.from.x);
        assert_eq!(anchor_a.from.y, anchor_b.from.y);

        // Anchor to is at target position
        assert_eq!(anchor_a.to.x, 300.0);
        assert_eq!(anchor_b.to.x, 300.0);
    }

    // ==================== Explicit Parallel Source Gating Tests ====================

    #[test]
    fn given_non_parallel_node_with_two_targets_when_find_parallel_then_returns_empty() {
        // Even with >=2 outgoing edges, non-Parallel nodes should NOT create parallel groups
        let source_id = NodeId::new();
        let target_a_id = NodeId::new();
        let target_b_id = NodeId::new();

        let source = build_node(source_id, 100.0, 100.0); // Not a Parallel node
        let target_a = build_node(target_a_id, 300.0, 100.0);
        let target_b = build_node(target_b_id, 300.0, 200.0);

        let nodes = vec![source.clone(), target_a.clone(), target_b.clone()];

        let conn_a = build_connection(Uuid::new_v4(), source_id, target_a_id);
        let conn_b = build_connection(Uuid::new_v4(), source_id, target_b_id);
        let connections = vec![conn_a, conn_b];

        let groups = find_parallel_branches(&nodes, &connections);

        // Should be empty because source is not WorkflowNode::Parallel
        assert!(groups.is_empty());
    }

    #[test]
    fn given_parallel_node_with_two_targets_when_find_parallel_then_returns_one_group() {
        let source_id = NodeId::new();
        let target_a_id = NodeId::new();
        let target_b_id = NodeId::new();

        let source = build_parallel_node(source_id, 100.0, 100.0); // Explicit Parallel node
        let target_a = build_node(target_a_id, 300.0, 100.0);
        let target_b = build_node(target_b_id, 300.0, 200.0);

        let nodes = vec![source.clone(), target_a.clone(), target_b.clone()];

        let conn_a = build_connection(Uuid::new_v4(), source_id, target_a_id);
        let conn_b = build_connection(Uuid::new_v4(), source_id, target_b_id);
        let connections = vec![conn_a, conn_b];

        let groups = find_parallel_branches(&nodes, &connections);

        assert_eq!(groups.len(), 1);
        let group = &groups[0];

        assert_eq!(group.parallel_node_id, source_id);
        assert_eq!(group.branch_node_ids.len(), 2);
    }

    #[test]
    fn given_multiple_parallel_nodes_when_find_parallel_then_returns_groups_for_each() {
        let source_a_id = NodeId::new();
        let source_b_id = NodeId::new();
        let target_a1_id = NodeId::new();
        let target_a2_id = NodeId::new();
        let target_b1_id = NodeId::new();
        let target_b2_id = NodeId::new();

        let source_a = build_parallel_node(source_a_id, 100.0, 100.0);
        let source_b = build_parallel_node(source_b_id, 100.0, 300.0);
        let target_a1 = build_node(target_a1_id, 300.0, 100.0);
        let target_a2 = build_node(target_a2_id, 300.0, 200.0);
        let target_b1 = build_node(target_b1_id, 300.0, 300.0);
        let target_b2 = build_node(target_b2_id, 300.0, 400.0);

        let nodes = vec![
            source_a, source_b, target_a1, target_a2, target_b1, target_b2,
        ];

        let conn_a1 = build_connection(Uuid::new_v4(), source_a_id, target_a1_id);
        let conn_a2 = build_connection(Uuid::new_v4(), source_a_id, target_a2_id);
        let conn_b1 = build_connection(Uuid::new_v4(), source_b_id, target_b1_id);
        let conn_b2 = build_connection(Uuid::new_v4(), source_b_id, target_b2_id);
        let connections = vec![conn_a1, conn_a2, conn_b1, conn_b2];

        let groups = find_parallel_branches(&nodes, &connections);

        assert_eq!(groups.len(), 2);
    }

    // ==================== Zoom-Normalized Bend Tests ====================

    #[test]
    fn given_valid_zoom_when_normalize_bend_then_returns_scaled_delta() {
        let page_delta = 100.0;
        let zoom = 2.0; // 200% zoom

        let result = normalize_bend_delta(page_delta, zoom);

        assert_eq!(result, 50.0);
    }

    #[test]
    fn given_zoom_of_one_when_normalize_bend_then_returns_same_delta() {
        let page_delta = 75.0;
        let zoom = 1.0;

        let result = normalize_bend_delta(page_delta, zoom);

        assert_eq!(result, 75.0);
    }

    #[test]
    fn given_invalid_zoom_zero_when_normalize_bend_then_returns_zero() {
        let page_delta = 100.0;
        let zoom = 0.0;

        let result = normalize_bend_delta(page_delta, zoom);

        assert_eq!(result, 0.0);
    }

    #[test]
    fn given_invalid_zoom_negative_when_normalize_bend_then_returns_zero() {
        let page_delta = 100.0;
        let zoom = -1.0;

        let result = normalize_bend_delta(page_delta, zoom);

        assert_eq!(result, 0.0);
    }

    #[test]
    fn given_invalid_zoom_nan_when_normalize_bend_then_returns_zero() {
        let page_delta = 100.0;
        let zoom = f32::NAN;

        let result = normalize_bend_delta(page_delta, zoom);

        assert_eq!(result, 0.0);
    }

    #[test]
    fn given_invalid_zoom_infinity_when_normalize_bend_then_returns_zero() {
        let page_delta = 100.0;
        let zoom = f32::INFINITY;

        let result = normalize_bend_delta(page_delta, zoom);

        assert_eq!(result, 0.0);
    }

    // ==================== Shared Target Disambiguation Test ====================

    #[test]
    fn given_shared_target_across_sources_when_resolve_anchors_then_uses_source_target_match() {
        // Scenario: Two different Parallel sources both point to the SAME target
        // The anchor resolution should correctly associate each edge with its source
        let source_a_id = NodeId::new();
        let source_b_id = NodeId::new();
        let shared_target_id = NodeId::new();

        // Both sources must be Parallel nodes for parallel group detection
        let source_a = build_parallel_node(source_a_id, 100.0, 100.0);
        let source_b = build_parallel_node(source_b_id, 100.0, 300.0);
        let shared_target = build_node(shared_target_id, 300.0, 200.0);

        let nodes = vec![source_a.clone(), source_b.clone(), shared_target.clone()];

        let conn_a = build_connection(Uuid::new_v4(), source_a_id, shared_target_id);
        let conn_b = build_connection(Uuid::new_v4(), source_b_id, shared_target_id);
        let connections = vec![conn_a.clone(), conn_b.clone()];

        // Create parallel groups for each source (each has single target)
        let group_a = ParallelGroup {
            parallel_node_id: source_a_id,
            branch_node_ids: vec![shared_target_id],
            bounding_box: BoundingBox {
                x: 292.0,
                y: 192.0,
                width: 236.0,
                height: 84.0,
            },
            branch_count: 1,
            aggregate_status: AggregateStatus::Pending,
        };
        let group_b = ParallelGroup {
            parallel_node_id: source_b_id,
            branch_node_ids: vec![shared_target_id],
            bounding_box: BoundingBox {
                x: 292.0,
                y: 392.0,
                width: 236.0,
                height: 84.0,
            },
            branch_count: 1,
            aggregate_status: AggregateStatus::Pending,
        };
        let groups = vec![group_a, group_b];

        let anchors = resolve_edge_anchors_with_parallel(&connections, &nodes, &groups);

        // Both edges should resolve to the same target position (no offset since single target)
        let anchor_a = anchors.get(&conn_a.id.to_string()).copied();
        let anchor_b = anchors.get(&conn_b.id.to_string()).copied();

        assert!(anchor_a.is_some());
        assert!(anchor_b.is_some());

        let anchor_a = anchor_a.unwrap();
        let anchor_b = anchor_b.unwrap();

        // Both should have the same target y since there's only one target in each group
        assert_eq!(anchor_a.to.y, anchor_b.to.y);
    }
}

#[component]
pub fn FlowEdges(
    edges: ReadSignal<Vec<Connection>>,
    nodes: ReadSignal<Vec<Node>>,
    temp_edge: ReadSignal<Option<(Position, Position)>>,
    running_node_ids: ReadSignal<Vec<NodeId>>,
    zoom: ReadSignal<f32>,
) -> Element {
    let mut hovered_edge = use_signal(|| None::<String>);
    let mut bend_offsets = use_signal(HashMap::<String, f32>::new);
    let mut drag_state = use_signal(|| None::<DragState>);

    let _edge_anchors = use_memo(move || {
        let node_list = nodes.read();
        let edge_list = edges.read();
        resolve_edge_anchors(&edge_list, &node_list)
    });

    let node_by_id = use_memo(move || {
        nodes
            .read()
            .iter()
            .cloned()
            .map(|node| (node.id, node))
            .collect::<HashMap<_, _>>()
    });

    let parallel_groups = use_memo(move || {
        let node_list = nodes.read();
        let edge_list = edges.read();
        find_parallel_branches(&node_list, &edge_list)
    });

    let temp_path = use_memo(move || {
        (*temp_edge.read()).map(|(from, to)| create_smooth_step_path(from, to, 0.0).0)
    });

    let edge_anchors_with_parallel = use_memo(move || {
        let node_list = nodes.read();
        let edge_list = edges.read();
        resolve_edge_anchors_with_parallel(&edge_list, &node_list, &parallel_groups.read())
    });

    let svg_pointer_class = if drag_state.read().is_some() {
        "pointer-events-auto"
    } else {
        "pointer-events-none"
    };

    rsx! {
        svg {
            class: "absolute inset-0 overflow-visible {svg_pointer_class}",
            style: "width: 100%; height: 100%; z-index: 0;",
            onmousemove: move |evt| {
                if let Some(state) = drag_state.read().clone() {
                    let coordinates = evt.page_coordinates();
                    #[allow(clippy::cast_possible_truncation)]
                    let page_y = coordinates.y as f32;
                    if !page_y.is_finite() {
                        return;
                    }
                    let current_zoom = *zoom.read();
                    // Validate zoom before applying delta
                    if !current_zoom.is_finite() || current_zoom <= 0.0 {
                        return;
                    }
                    // Normalize page-space delta to canvas-space using zoom
                    let page_delta = page_y - state.start_page_y;
                    let canvas_delta = page_delta / current_zoom;
                    let next_bend = sanitize_bend_input_edge(state.start_bend + canvas_delta, state.start_bend);
                    bend_offsets.write().insert(state.edge_id, next_bend);
                }
            },
            onmouseup: move |_| {
                drag_state.set(None);
            },
            onmouseleave: move |_| {
                drag_state.set(None);
            },
            defs {
                linearGradient {
                    id: "edge-running-gradient",
                    x1: "0%",
                    y1: "0%",
                    x2: "100%",
                    y2: "0%",
                    stop { offset: "0%", stop_color: "rgba(14,165,233,0.95)" }
                    stop { offset: "100%", stop_color: "rgba(45,212,191,0.95)" }
                }
                marker {
                    id: "arrowhead",
                    marker_width: "10",
                    marker_height: "8",
                    ref_x: "9",
                    ref_y: "4",
                    orient: "auto",
                    path {
                        d: "M 0 0 L 10 4 L 0 8 z",
                        class: "fill-slate-600"
                    }
                }
                marker {
                    id: "arrowhead-active",
                    marker_width: "10",
                    marker_height: "8",
                    ref_x: "9",
                    ref_y: "4",
                    orient: "auto",
                    path {
                        d: "M 0 0 L 10 4 L 0 8 z",
                        class: "fill-cyan-500"
                    }
                }
            }

          for group in parallel_groups.read().iter() {
                {
                    let (color, border_color) = if group.branch_node_ids.len() > 2 {
                        ("rgba(251, 146, 60, 0.14)", "rgba(245, 158, 11, 0.4)")
                    } else {
                        ("rgba(20, 184, 166, 0.10)", "rgba(13, 148, 136, 0.35)")
                    };
                    let badge_count = if group.branch_node_ids.len() > 1 {
                        Some(group.branch_node_ids.len())
                    } else {
                        None
                    };
                    let key = format!("parallel-group-{}-{}", group.bounding_box.x, group.bounding_box.y);

                    let badge_left = group.bounding_box.x + group.bounding_box.width + 8.0;
                    let badge_top = group.bounding_box.y - 24.0;
                    rsx! {
                        rect {
                            key: "{key}",
                            x: "{group.bounding_box.x}",
                            y: "{group.bounding_box.y}",
                            width: "{group.bounding_box.width}",
                            height: "{group.bounding_box.height}",
                            rx: "8",
                            fill: "{color}",
                            stroke: "{border_color}",
                            stroke_width: "1.5"
                        }
                        {badge_count.map(|count| rsx! {
                            g {
                                rect {
                                    x: "{badge_left}",
                                    y: "{badge_top}",
                                    width: "86",
                                    height: "18",
                                    rx: "6",
                                    fill: "rgba(15,23,42,0.92)",
                                    stroke: "rgba(71,85,105,0.8)",
                                    stroke_width: "1"
                                }
                                text {
                                    x: "{badge_left + 8.0}",
                                    y: "{badge_top + 12.5}",
                                    fill: "rgba(226,232,240,0.95)",
                                    font_size: "10",
                                    font_weight: "600",
                                    "{count} branches"
                                }
                            }
                        })}
                    }
                }
            }

            for edge in edges.read().iter() {
                {
                    let edge_id = edge.id.to_string();
                    let anchor = edge_anchors_with_parallel.read().get(&edge_id).copied();

                    if let Some(anchor) = anchor {
                        let bend = bend_offsets
                            .read()
                            .get(&edge_id)
                            .copied()
                            .map_or(0.0, |value| value);
                        let (path, midpoint) = create_smooth_step_path(anchor.from, anchor.to, bend);
                        let dragging_this = drag_state
                            .read()
                            .as_ref()
                            .is_some_and(|state| state.edge_id == edge_id);
                        let hovered_this = hovered_edge
                            .read()
                            .as_ref()
                            .is_some_and(|id| *id == edge_id);
                        let handle_opacity = if hovered_this || dragging_this { "1" } else { "0" };
                        let source_status = node_by_id
                            .read()
                            .get(&edge.source)
                            .and_then(|node| node.config.get("status"))
                            .and_then(serde_json::Value::as_str)
                            .map_or_else(|| "pending".to_string(), std::string::ToString::to_string);
                        let target_is_running = running_node_ids
                            .read()
                            .contains(&edge.target);
                        let stroke_color = match source_status {
                            ref status if status == "running" => "url(#edge-running-gradient)",
                            ref status if status == "completed" => "rgba(16, 185, 129, 0.85)",
                            ref status if status == "failed" => "rgba(244, 63, 94, 0.85)",
                            _ => "rgba(148, 163, 184, 0.9)",
                        };
                        let marker = if source_status == "running" || target_is_running {
                            "url(#arrowhead-active)"
                        } else {
                            "url(#arrowhead)"
                        };
                        let dash = if source_status == "running" || target_is_running { "6 4" } else { "0" };
                        let animation_class = if target_is_running { "edge-animated" } else { "" };

                        rsx! {
                            g { key: "{edge_id}",
                                path {
                                    d: "{path}",
                                    fill: "none",
                                    stroke: "transparent",
                                    stroke_width: "16",
                                    pointer_events: "stroke",
                                    class: "pointer-events-auto",
                                    onmouseenter: {
                                        let edge_id = edge_id.clone();
                                        move |_| hovered_edge.set(Some(edge_id.clone()))
                                    },
                                    onmouseleave: {
                                        let edge_id = edge_id.clone();
                                        move |_| {
                                            let is_dragging = drag_state
                                                .read()
                                                .as_ref()
                                                .is_some_and(|state| state.edge_id == edge_id);
                                            if !is_dragging {
                                                hovered_edge.set(None);
                                            }
                                        }
                                    }
                                }
                                path {
                                    d: "{path}",
                                    fill: "none",
                                    stroke: "rgba(14,116,144,0.18)",
                                    stroke_width: "6",
                                    opacity: if target_is_running { "1" } else { "0" },
                                    class: "transition-opacity duration-150",
                                }
                                path {
                                    d: "{path}",
                                    fill: "none",
                                    stroke: "{stroke_color}",
                                    stroke_width: "2",
                                    marker_end: "{marker}",
                                    stroke_dasharray: "{dash}",
                                    class: "transition-all duration-150 {animation_class}",
                                    style: if target_is_running { Some("animation: flow 0.5s linear infinite") } else { None }
                                }
                                circle {
                                    cx: "{midpoint.x}",
                                    cy: "{midpoint.y}",
                                    r: "5",
                                    fill: "rgba(99, 102, 241, 0.95)",
                                    stroke: "rgba(226, 232, 240, 0.95)",
                                    stroke_width: "1.5",
                                    opacity: "{handle_opacity}",
                                    class: "pointer-events-auto cursor-ns-resize transition-opacity duration-100",
                                    onmousedown: {
                                        let edge_id = edge_id.clone();
                                        move |evt| {
                                            evt.stop_propagation();
                                            let coordinates = evt.page_coordinates();
                                            #[allow(clippy::cast_possible_truncation)]
                                            let page_y = coordinates.y as f32;
                                            if !page_y.is_finite() {
                                                return;
                                            }
                                            let current_bend = bend_offsets
                                                .read()
                                                .get(&edge_id)
                                                .copied()
                                                .map_or(0.0, |value| value);
                                            let next_bend = sanitize_bend_input_edge(current_bend, current_bend);
                                            drag_state.set(Some(DragState {
                                                edge_id: edge_id.clone(),
                                                start_page_y: page_y,
                                                start_bend: next_bend,
                                            }));
                                            hovered_edge.set(Some(edge_id.clone()));
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        rsx! {}
                    }
                }
            }

            if let Some(path) = temp_path.read().as_ref() {
                path {
                    d: "{path}",
                    fill: "none",
                    stroke: "rgba(99, 102, 241, 0.6)",
                    stroke_width: "2",
                    stroke_dasharray: "6 4"
                }
            }
        }
    }
}
