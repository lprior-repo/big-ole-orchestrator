use std::collections::HashMap;

use super::graph_types::{Connection, Node, NodeId, WorkflowNode};
use super::types::{sanitize_bend_input_edge, EdgeAnchor, Position, BEND_CLAMP, NODE_HEIGHT, NODE_WIDTH};
use crate::ui::parallel_group_overlay::{AggregateStatus, BoundingBox, ParallelGroup};

#[cfg(test)]
mod layout_tests {
    use super::*;

    #[test]
    fn create_smooth_step_path_horizontal_line() {
        let from = Position { x: 0.0, y: 50.0 };
        let to = Position { x: 100.0, y: 50.0 };
        let (path, midpoint) = create_smooth_step_path(from, to, 0.0);
        assert!(path.starts_with("M 0 50"));
        assert_eq!(midpoint.x, 50.0);
        assert_eq!(midpoint.y, 50.0);
    }

    #[test]
    fn create_smooth_step_path_vertical_line() {
        let from = Position { x: 50.0, y: 0.0 };
        let to = Position { x: 50.0, y: 100.0 };
        let (path, midpoint) = create_smooth_step_path(from, to, 0.0);
        assert!(path.starts_with("M 50 0"));
        assert_eq!(midpoint.x, 50.0);
        assert_eq!(midpoint.y, 50.0);
    }

    #[test]
    fn create_smooth_step_path_with_bend_offset() {
        let from = Position { x: 0.0, y: 0.0 };
        let to = Position { x: 100.0, y: 100.0 };
        let (path, midpoint) = create_smooth_step_path(from, to, 10.0);
        assert!(!path.is_empty());
        assert!(midpoint.y > 50.0);
    }

    #[test]
    fn create_smooth_step_path_with_negative_bend() {
        let from = Position { x: 0.0, y: 0.0 };
        let to = Position { x: 100.0, y: 100.0 };
        let (path, midpoint) = create_smooth_step_path(from, to, -10.0);
        assert!(!path.is_empty());
        assert!(midpoint.y < 50.0);
    }

    #[test]
    fn create_smooth_step_path_bend_clamped_to_max() {
        let from = Position { x: 0.0, y: 0.0 };
        let to = Position { x: 100.0, y: 100.0 };
        let (_, midpoint) = create_smooth_step_path(from, to, 500.0);
        assert!(midpoint.y <= 100.0 + BEND_CLAMP);
    }

    #[test]
    fn create_smooth_step_path_ignores_nan_coordinates() {
        let from = Position { x: f32::NAN, y: 50.0 };
        let to = Position { x: 100.0, y: 50.0 };
        let (path, midpoint) = create_smooth_step_path(from, to, 0.0);
        assert!(path.contains("L"));
        assert_eq!(midpoint.x, f32::midpoint(f32::NAN, 100.0));
    }

    #[test]
    fn create_smooth_step_path_ignores_infinite_coordinates() {
        let from = Position { x: f32::INFINITY, y: 50.0 };
        let to = Position { x: 100.0, y: 50.0 };
        let (path, _) = create_smooth_step_path(from, to, 0.0);
        assert!(path.contains("L"));
    }

    #[test]
    fn create_smooth_step_path_vertical_very_close_points() {
        let from = Position { x: 50.0, y: 0.0 };
        let to = Position { x: 51.0, y: 0.5 };
        let (path, _) = create_smooth_step_path(from, to, 0.0);
        assert!(path.starts_with("M 50 0 L"));
    }

    #[test]
    fn sanitize_bend_input_edge_clamps_positive() {
        let result = sanitize_bend_input_edge(300.0, 0.0);
        assert_eq!(result, BEND_CLAMP);
    }

    #[test]
    fn sanitize_bend_input_edge_clamps_negative() {
        let result = sanitize_bend_input_edge(-300.0, 0.0);
        assert_eq!(result, -BEND_CLAMP);
    }

    #[test]
    fn sanitize_bend_input_edge_accepts_valid() {
        let result = sanitize_bend_input_edge(50.0, 0.0);
        assert_eq!(result, 50.0);
    }

    #[test]
    fn sanitize_bend_input_edge_returns_start_on_nan() {
        let result = sanitize_bend_input_edge(f32::NAN, 100.0);
        assert_eq!(result, 100.0);
    }

    #[test]
    fn sanitize_bend_input_edge_returns_start_on_infinity() {
        let result = sanitize_bend_input_edge(f32::INFINITY, 100.0);
        assert_eq!(result, 100.0);
    }

    #[test]
    fn find_parallel_branches_returns_empty_for_single_target() {
        let source_id = NodeId::new();
        let target_id = NodeId::new();
        let source = Node::from_workflow_node(
            "Parallel".to_string(),
            WorkflowNode::Parallel(crate::ui::edges::graph_types::ParallelConfig::default()),
            100.0, 100.0,
        );
        let target = Node::from_workflow_node(
            "Target".to_string(),
            WorkflowNode::Run(crate::ui::edges::graph_types::RunConfig::default()),
            300.0, 100.0,
        );
        let nodes = vec![source, target];
        let conn = Connection {
            id: uuid::Uuid::new_v4(),
            source: source_id.clone(),
            target: target_id.clone(),
            source_port: crate::ui::graph::PortName::from("out"),
            target_port: crate::ui::graph::PortName::from("in"),
        };
        let connections = vec![conn];
        let groups = find_parallel_branches(&nodes, &connections);
        assert!(groups.is_empty());
    }
}

pub(crate) fn create_smooth_step_path(
    from: Position,
    to: Position,
    bend_y: f32,
) -> (String, Position) {
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

pub(crate) fn resolve_edge_anchors(
    edges: &[Connection],
    nodes: &[Node],
) -> HashMap<String, EdgeAnchor> {
    let node_by_id: HashMap<_, _> = nodes.iter().map(|node| (node.id, node.clone())).collect();

    edges
        .iter()
        .filter_map(|edge| {
            let source = node_by_id.get(&edge.source)?;
            let target = node_by_id.get(&edge.target)?;
            let from = super::types::get_source_point(source);
            let to = super::types::get_target_point(target);
            Some((edge.id.to_string(), EdgeAnchor { from, to }))
        })
        .collect()
}

pub(crate) fn resolve_edge_anchors_with_parallel(
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
            let from = super::types::get_source_point(source);
            let to = super::types::get_target_point(target);

            let group = parallel_groups.iter().find(|g| {
                g.parallel_node_id == edge.source && g.branch_node_ids.contains(&edge.target)
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
pub(crate) fn calculate_parallel_offset(
    target_id: &NodeId,
    targets: &[Node],
    node_height: f32,
) -> f32 {
    let mut sorted: Vec<_> = targets.iter().enumerate().collect();
    sorted.sort_by_key(|a| a.1.id.0);

    let idx = sorted
        .iter()
        .position(|(_, n)| n.id == *target_id)
        .unwrap_or(0);

    let spacing = node_height / 2.5;
    (idx as f32 - (sorted.len() as f32 - 1.0) / 2.0) * spacing
}

pub(crate) fn find_parallel_branches(
    nodes: &[Node],
    connections: &[Connection],
) -> Vec<ParallelGroup> {
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

            target_nodes.sort_by_key(|a| a.id.0);

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
