#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

use uuid::Uuid;
use vo_frontend::ui::edges::graph_types::{
    Connection, ExecutionState, Node, NodeId, PortName, WorkflowNode,
};
use vo_frontend::ui::edges::layout::{
    calculate_parallel_offset, create_smooth_step_path, find_parallel_branches,
    resolve_edge_anchors, resolve_edge_anchors_with_parallel,
};
use vo_frontend::ui::edges::types::{
    sanitize_bend_input_edge, BEND_CLAMP, NODE_HEIGHT, NODE_WIDTH,
};
use vo_frontend::ui::parallel_group_overlay::{AggregateStatus, BoundingBox, ParallelGroup};

fn build_node(id: NodeId, x: f32, y: f32) -> Node {
    let mut node = Node::from_workflow_node(
        format!("Node {}", id),
        WorkflowNode::Run(crate::ui::edges::graph_types::RunConfig::default()),
        x,
        y,
    );
    node.id = id;
    node
}

fn build_parallel_node(id: NodeId, x: f32, y: f32) -> Node {
    let mut node = Node::from_workflow_node(
        format!("Parallel {}", id),
        WorkflowNode::Parallel(crate::ui::edges::graph_types::ParallelConfig::default()),
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

#[test]
fn create_smooth_step_path_returns_valid_svg_path() {
    let from = vo_frontend::ui::edges::types::Position { x: 0.0, y: 50.0 };
    let to = vo_frontend::ui::edges::types::Position { x: 100.0, y: 50.0 };
    let (path, midpoint) = create_smooth_step_path(from, to, 0.0);
    assert!(path.starts_with("M 0 50"));
    assert!(path.contains("L"));
    assert_eq!(midpoint.x, 50.0);
    assert_eq!(midpoint.y, 50.0);
}

#[test]
fn create_smooth_step_path_horizontal_same_y() {
    let from = vo_frontend::ui::edges::types::Position { x: 0.0, y: 50.0 };
    let to = vo_frontend::ui::edges::types::Position { x: 100.0, y: 50.0 };
    let (path, midpoint) = create_smooth_step_path(from, to, 0.0);
    assert!(path.starts_with("M 0 50"));
    assert!(path.contains("100"));
    assert_eq!(midpoint.x, 50.0);
}

#[test]
fn create_smooth_step_path_vertical_same_x() {
    let from = vo_frontend::ui::edges::types::Position { x: 50.0, y: 0.0 };
    let to = vo_frontend::ui::edges::types::Position { x: 50.0, y: 100.0 };
    let (path, midpoint) = create_smooth_step_path(from, to, 0.0);
    assert!(path.starts_with("M 50 0"));
    assert!(path.contains("L"));
    assert_eq!(midpoint.x, 50.0);
    assert_eq!(midpoint.y, 50.0);
}

#[test]
fn create_smooth_step_path_with_positive_bend() {
    let from = vo_frontend::ui::edges::types::Position { x: 0.0, y: 0.0 };
    let to = vo_frontend::ui::edges::types::Position { x: 100.0, y: 100.0 };
    let (path, midpoint) = create_smooth_step_path(from, to, 10.0);
    assert!(!path.is_empty());
    assert!(midpoint.y > 50.0);
}

#[test]
fn create_smooth_step_path_with_negative_bend() {
    let from = vo_frontend::ui::edges::types::Position { x: 0.0, y: 0.0 };
    let to = vo_frontend::ui::edges::types::Position { x: 100.0, y: 100.0 };
    let (path, midpoint) = create_smooth_step_path(from, to, -10.0);
    assert!(!path.is_empty());
    assert!(midpoint.y < 50.0);
}

#[test]
fn create_smooth_step_path_bend_clamped_to_max() {
    let from = vo_frontend::ui::edges::types::Position { x: 0.0, y: 0.0 };
    let to = vo_frontend::ui::edges::types::Position { x: 100.0, y: 100.0 };
    let (_, midpoint) = create_smooth_step_path(from, to, 500.0);
    assert!(midpoint.y <= 100.0 + BEND_CLAMP);
}

#[test]
fn create_smooth_step_path_bend_clamped_to_min() {
    let from = vo_frontend::ui::edges::types::Position { x: 0.0, y: 0.0 };
    let to = vo_frontend::ui::edges::types::Position { x: 100.0, y: 100.0 };
    let (_, midpoint) = create_smooth_step_path(from, to, -500.0);
    assert!(midpoint.y >= 0.0 - BEND_CLAMP);
}

#[test]
fn create_smooth_step_path_ignores_nan_x() {
    let from = vo_frontend::ui::edges::types::Position {
        x: f32::NAN,
        y: 50.0,
    };
    let to = vo_frontend::ui::edges::types::Position { x: 100.0, y: 50.0 };
    let (path, midpoint) = create_smooth_step_path(from, to, 0.0);
    assert!(path.contains("L"));
    assert!(midpoint.x.is_nan());
}

#[test]
fn create_smooth_step_path_ignores_nan_y() {
    let from = vo_frontend::ui::edges::types::Position {
        x: 0.0,
        y: f32::NAN,
    };
    let to = vo_frontend::ui::edges::types::Position { x: 100.0, y: 50.0 };
    let (path, midpoint) = create_smooth_step_path(from, to, 0.0);
    assert!(path.contains("L"));
    assert!(midpoint.y.is_nan());
}

#[test]
fn create_smooth_step_path_ignores_infinite_x() {
    let from = vo_frontend::ui::edges::types::Position {
        x: f32::INFINITY,
        y: 50.0,
    };
    let to = vo_frontend::ui::edges::types::Position { x: 100.0, y: 50.0 };
    let (path, _) = create_smooth_step_path(from, to, 0.0);
    assert!(path.contains("L"));
}

#[test]
fn create_smooth_step_path_ignores_infinite_y() {
    let from = vo_frontend::ui::edges::types::Position {
        x: 0.0,
        y: f32::INFINITY,
    };
    let to = vo_frontend::ui::edges::types::Position { x: 100.0, y: 50.0 };
    let (path, _) = create_smooth_step_path(from, to, 0.0);
    assert!(path.contains("L"));
}

#[test]
fn create_smooth_step_path_vertical_very_close_points() {
    let from = vo_frontend::ui::edges::types::Position { x: 50.0, y: 0.0 };
    let to = vo_frontend::ui::edges::types::Position { x: 51.0, y: 0.5 };
    let (path, _) = create_smooth_step_path(from, to, 0.0);
    assert!(path.starts_with("M 50 0 L"));
}

#[test]
fn create_smooth_step_path_diagonal_down_right() {
    let from = vo_frontend::ui::edges::types::Position { x: 0.0, y: 0.0 };
    let to = vo_frontend::ui::edges::types::Position { x: 200.0, y: 100.0 };
    let (path, midpoint) = create_smooth_step_path(from, to, 0.0);
    assert!(path.starts_with("M 0 0"));
    assert!(path.contains("200"));
    assert_eq!(midpoint.x, 100.0);
}

#[test]
fn create_smooth_step_path_diagonal_up_right() {
    let from = vo_frontend::ui::edges::types::Position { x: 0.0, y: 100.0 };
    let to = vo_frontend::ui::edges::types::Position { x: 200.0, y: 0.0 };
    let (path, midpoint) = create_smooth_step_path(from, to, 0.0);
    assert!(path.starts_with("M 0 100"));
    assert!(path.contains("200"));
    assert_eq!(midpoint.x, 100.0);
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
fn sanitize_bend_input_edge_returns_start_on_neg_infinity() {
    let result = sanitize_bend_input_edge(f32::NEG_INFINITY, 100.0);
    assert_eq!(result, 100.0);
}

#[test]
fn find_parallel_branches_returns_empty_for_single_target() {
    let source_id = NodeId::new();
    let target_id = NodeId::new();
    let source = build_parallel_node(source_id, 100.0, 100.0);
    let target = build_node(target_id, 300.0, 100.0);
    let nodes = vec![source, target];
    let conn = build_connection(Uuid::new_v4(), source_id, target_id);
    let connections = vec![conn];
    let groups = find_parallel_branches(&nodes, &connections);
    assert!(groups.is_empty());
}

#[test]
fn find_parallel_branches_detects_parallel_node_with_multiple_targets() {
    let source_id = NodeId::new();
    let target_a_id = NodeId::new();
    let target_b_id = NodeId::new();
    let source = build_parallel_node(source_id, 100.0, 100.0);
    let target_a = build_node(target_a_id, 300.0, 100.0);
    let target_b = build_node(target_b_id, 300.0, 200.0);
    let nodes = vec![source, target_a, target_b];
    let conn_a = build_connection(Uuid::new_v4(), source_id, target_a_id);
    let conn_b = build_connection(Uuid::new_v4(), source_id, target_b_id);
    let connections = vec![conn_a, conn_b];
    let groups = find_parallel_branches(&nodes, &connections);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].branch_count, 2);
}

#[test]
fn find_parallel_branches_ignores_non_parallel_source() {
    let source_id = NodeId::new();
    let target_a_id = NodeId::new();
    let target_b_id = NodeId::new();
    let source = build_node(source_id, 100.0, 100.0);
    let target_a = build_node(target_a_id, 300.0, 100.0);
    let target_b = build_node(target_b_id, 300.0, 200.0);
    let nodes = vec![source, target_a, target_b];
    let conn_a = build_connection(Uuid::new_v4(), source_id, target_a_id);
    let conn_b = build_connection(Uuid::new_v4(), source_id, target_b_id);
    let connections = vec![conn_a, conn_b];
    let groups = find_parallel_branches(&nodes, &connections);
    assert!(groups.is_empty());
}

#[test]
fn find_parallel_branches_detects_multiple_parallel_sources() {
    let source_a_id = NodeId::new();
    let source_b_id = NodeId::new();
    let target_a_id = NodeId::new();
    let target_b_id = NodeId::new();
    let source_a = build_parallel_node(source_a_id, 100.0, 100.0);
    let source_b = build_parallel_node(source_b_id, 100.0, 300.0);
    let target_a = build_node(target_a_id, 300.0, 100.0);
    let target_b = build_node(target_b_id, 300.0, 300.0);
    let nodes = vec![source_a, source_b, target_a, target_b];
    let conn_a = build_connection(Uuid::new_v4(), source_a_id, target_a_id);
    let conn_b = build_connection(Uuid::new_v4(), source_b_id, target_b_id);
    let connections = vec![conn_a, conn_b];
    let groups = find_parallel_branches(&nodes, &connections);
    assert_eq!(groups.len(), 2);
}

#[test]
fn resolve_edge_anchors_returns_anchors_for_valid_edges() {
    let source_id = NodeId::new();
    let target_id = NodeId::new();
    let source = build_node(source_id, 100.0, 100.0);
    let target = build_node(target_id, 300.0, 100.0);
    let nodes = vec![source, target];
    let connection = build_connection(Uuid::new_v4(), source_id, target_id);
    let connections = vec![connection.clone()];
    let anchors = resolve_edge_anchors(&connections, &nodes);
    assert!(anchors.contains_key(&connection.id.to_string()));
    let anchor = anchors.get(&connection.id.to_string()).unwrap();
    assert_eq!(anchor.from.x, 320.0);
    assert_eq!(anchor.from.y, 134.0);
    assert_eq!(anchor.to.x, 300.0);
    assert_eq!(anchor.to.y, 134.0);
}

#[test]
fn resolve_edge_anchors_returns_empty_for_missing_source() {
    let source_id = NodeId::new();
    let target_id = NodeId::new();
    let target = build_node(target_id, 300.0, 100.0);
    let nodes = vec![target];
    let connection = build_connection(Uuid::new_v4(), source_id, target_id);
    let connections = vec![connection];
    let anchors = resolve_edge_anchors(&connections, &nodes);
    assert!(anchors.is_empty());
}

#[test]
fn resolve_edge_anchors_returns_empty_for_missing_target() {
    let source_id = NodeId::new();
    let target_id = NodeId::new();
    let source = build_node(source_id, 100.0, 100.0);
    let nodes = vec![source];
    let connection = build_connection(Uuid::new_v4(), source_id, target_id);
    let connections = vec![connection];
    let anchors = resolve_edge_anchors(&connections, &nodes);
    assert!(anchors.is_empty());
}

#[test]
fn resolve_edge_anchors_with_parallel_applies_offsets() {
    let source_id = NodeId::new();
    let target_a_id = NodeId::new();
    let target_b_id = NodeId::new();
    let source = build_parallel_node(source_id, 100.0, 100.0);
    let target_a = build_node(target_a_id, 300.0, 100.0);
    let target_b = build_node(target_b_id, 300.0, 200.0);
    let nodes = vec![source, target_a.clone(), target_b.clone()];
    let conn_a = build_connection(Uuid::new_v4(), source_id, target_a_id);
    let conn_b = build_connection(Uuid::new_v4(), source_id, target_b_id);
    let connections = vec![conn_a.clone(), conn_b.clone()];
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
    assert!(anchors.contains_key(&conn_a.id.to_string()));
    assert!(anchors.contains_key(&conn_b.id.to_string()));
}

#[test]
fn calculate_parallel_offset_returns_zero_for_single_target() {
    let target_id = NodeId::new();
    let target = build_node(target_id, 300.0, 100.0);
    let offset = calculate_parallel_offset(&target_id, &[target], NODE_HEIGHT);
    assert_eq!(offset, 0.0);
}

#[test]
fn calculate_parallel_offset_returns_symmetric_offsets_for_two_targets() {
    let target_a_id = NodeId::new();
    let target_b_id = NodeId::new();
    let target_a = build_node(target_a_id, 300.0, 100.0);
    let target_b = build_node(target_b_id, 300.0, 200.0);
    let targets = vec![target_a.clone(), target_b.clone()];
    let offset_a = calculate_parallel_offset(&target_a_id, &targets, NODE_HEIGHT);
    let offset_b = calculate_parallel_offset(&target_b_id, &targets, NODE_HEIGHT);
    assert_eq!(offset_a, -offset_b);
}

#[test]
fn calculate_parallel_offset_returns_symmetric_offsets_for_three_targets() {
    let target_a_id = NodeId::new();
    let target_b_id = NodeId::new();
    let target_c_id = NodeId::new();
    let target_a = build_node(target_a_id, 300.0, 100.0);
    let target_b = build_node(target_b_id, 300.0, 200.0);
    let target_c = build_node(target_c_id, 300.0, 300.0);
    let targets = vec![target_a.clone(), target_b.clone(), target_c.clone()];
    let offset_a = calculate_parallel_offset(&target_a_id, &targets, NODE_HEIGHT);
    let offset_b = calculate_parallel_offset(&target_b_id, &targets, NODE_HEIGHT);
    let offset_c = calculate_parallel_offset(&target_c_id, &targets, NODE_HEIGHT);
    assert_eq!(offset_b, 0.0);
    assert!(offset_a < 0.0);
    assert!(offset_c > 0.0);
    assert_eq!(offset_a, -offset_c);
}

#[test]
fn parallel_group_calculation_is_deterministic() {
    let source_id = NodeId::new();
    let target_a_id = NodeId::new();
    let target_b_id = NodeId::new();
    let source = build_parallel_node(source_id, 100.0, 100.0);
    let target_a = build_node(target_a_id, 300.0, 100.0);
    let target_b = build_node(target_b_id, 300.0, 200.0);
    let nodes = vec![source, target_a, target_b];
    let conn_a = build_connection(Uuid::new_v4(), source_id, target_a_id);
    let conn_b = build_connection(Uuid::new_v4(), source_id, target_b_id);
    let connections = vec![conn_a, conn_b];
    let groups1 = find_parallel_branches(&nodes, &connections);
    let groups2 = find_parallel_branches(&nodes, &connections);
    assert_eq!(groups1.len(), groups2.len());
    if let Some(g1) = groups1.first() {
        if let Some(g2) = groups2.first() {
            assert_eq!(g1.branch_count, g2.branch_count);
        }
    }
}

#[test]
fn execution_state_all_variants_have_status_badge_class() {
    let states = [
        ExecutionState::Idle,
        ExecutionState::Running,
        ExecutionState::Queued,
        ExecutionState::Completed,
        ExecutionState::Failed,
        ExecutionState::Skipped,
    ];
    for state in states {
        let class = state.status_badge_class();
        assert!(!class.is_empty());
    }
}

#[test]
fn execution_state_all_variants_have_label() {
    let states = [
        ExecutionState::Idle,
        ExecutionState::Running,
        ExecutionState::Queued,
        ExecutionState::Completed,
        ExecutionState::Failed,
        ExecutionState::Skipped,
    ];
    for state in states {
        let label = state.label();
        assert!(!label.is_empty());
    }
}

#[test]
fn execution_state_default_is_idle() {
    let state = ExecutionState::default();
    assert_eq!(state, ExecutionState::Idle);
}

#[test]
fn execution_state_idle_and_queued_have_same_pending_badge() {
    let idle_class = ExecutionState::Idle.status_badge_class();
    let queued_class = ExecutionState::Queued.status_badge_class();
    assert_eq!(idle_class, queued_class);
}

#[test]
fn execution_state_idle_and_queued_have_same_pending_label() {
    let idle_label = ExecutionState::Idle.label();
    let queued_label = ExecutionState::Queued.label();
    assert_eq!(idle_label, queued_label);
}

#[test]
fn execution_state_running_has_different_badge_than_completed() {
    let running_class = ExecutionState::Running.status_badge_class();
    let completed_class = ExecutionState::Completed.status_badge_class();
    assert_ne!(running_class, completed_class);
}

#[test]
fn execution_state_failed_has_different_badge_than_completed() {
    let failed_class = ExecutionState::Failed.status_badge_class();
    let completed_class = ExecutionState::Completed.status_badge_class();
    assert_ne!(failed_class, completed_class);
}

#[test]
fn execution_state_skipped_has_different_badge_than_idle() {
    let skipped_class = ExecutionState::Skipped.status_badge_class();
    let idle_class = ExecutionState::Idle.status_badge_class();
    assert_ne!(skipped_class, idle_class);
}

#[test]
fn node_execution_state_can_be_modified() {
    let mut node = build_node(NodeId::new(), 100.0, 100.0);
    assert_eq!(node.execution_state, ExecutionState::Idle);
    node.execution_state = ExecutionState::Running;
    assert_eq!(node.execution_state, ExecutionState::Running);
    node.execution_state = ExecutionState::Completed;
    assert_eq!(node.execution_state, ExecutionState::Completed);
}

#[test]
fn parallel_group_bounding_box_calculation() {
    let source_id = NodeId::new();
    let target_a_id = NodeId::new();
    let target_b_id = NodeId::new();
    let source = build_parallel_node(source_id, 100.0, 100.0);
    let target_a = build_node(target_a_id, 300.0, 100.0);
    let target_b = build_node(target_b_id, 300.0, 200.0);
    let nodes = vec![source, target_a, target_b];
    let conn_a = build_connection(Uuid::new_v4(), source_id, target_a_id);
    let conn_b = build_connection(Uuid::new_v4(), source_id, target_b_id);
    let connections = vec![conn_a, conn_b];
    let groups = find_parallel_branches(&nodes, &connections);
    assert_eq!(groups.len(), 1);
    let group = &groups[0];
    assert!(group.bounding_box.width > 0.0);
    assert!(group.bounding_box.height > 0.0);
}

#[test]
fn resolve_edge_anchors_with_parallel_handles_empty_groups() {
    let source_id = NodeId::new();
    let target_id = NodeId::new();
    let source = build_node(source_id, 100.0, 100.0);
    let target = build_node(target_id, 300.0, 100.0);
    let nodes = vec![source, target];
    let conn = build_connection(Uuid::new_v4(), source_id, target_id);
    let connections = vec![conn.clone()];
    let groups: Vec<ParallelGroup> = vec![];
    let anchors = resolve_edge_anchors_with_parallel(&connections, &nodes, &groups);
    assert!(anchors.contains_key(&conn.id.to_string()));
}

#[test]
fn resolve_edge_anchors_with_parallel_ignores_unrelated_groups() {
    let source_id = NodeId::new();
    let target_a_id = NodeId::new();
    let target_b_id = NodeId::new();
    let unrelated_source_id = NodeId::new();
    let source = build_parallel_node(source_id, 100.0, 100.0);
    let target_a = build_node(target_a_id, 300.0, 100.0);
    let target_b = build_node(target_b_id, 300.0, 200.0);
    let unrelated_source = build_parallel_node(unrelated_source_id, 100.0, 400.0);
    let nodes = vec![
        source.clone(),
        target_a.clone(),
        target_b.clone(),
        unrelated_source,
    ];
    let conn_a = build_connection(Uuid::new_v4(), source_id, target_a_id);
    let conn_b = build_connection(Uuid::new_v4(), source_id, target_b_id);
    let connections = vec![conn_a.clone(), conn_b.clone()];
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
    assert_eq!(anchors.len(), 2);
}

#[test]
fn edge_connections_with_same_source_and_target_are_distinct() {
    let source_id = NodeId::new();
    let target_id = NodeId::new();
    let conn1 = build_connection(Uuid::new_v4(), source_id, target_id);
    let conn2 = build_connection(Uuid::new_v4(), source_id, target_id);
    assert_ne!(conn1.id, conn2.id);
}

#[test]
fn node_id_uniqueness_in_workflow() {
    let source_id = NodeId::new();
    let target_id = NodeId::new();
    assert_ne!(source_id, target_id);
}

#[test]
fn port_name_from_str_roundtrips() {
    let port = PortName::from("output");
    let port_str: String = port.into();
    assert_eq!(port_str, "output");
}

#[test]
fn port_name_display() {
    let port = PortName::from("input");
    let display = format!("{}", port);
    assert_eq!(display, "input");
}

#[test]
fn parallel_group_aggregate_status_all_variants() {
    let statuses = [
        AggregateStatus::Pending,
        AggregateStatus::Running,
        AggregateStatus::Completed,
        AggregateStatus::Failed,
    ];
    for status in statuses {
        let _ = format!("{:?}", status);
    }
}

#[test]
fn workflow_node_is_parallel_method() {
    let parallel_node =
        WorkflowNode::Parallel(crate::ui::edges::graph_types::ParallelConfig::default());
    let run_node = WorkflowNode::Run(crate::ui::edges::graph_types::RunConfig::default());
    assert!(parallel_node.is_parallel());
    assert!(!run_node.is_parallel());
}

#[test]
fn workflow_node_from_str_parses_correctly() {
    use std::str::FromStr;
    let run = WorkflowNode::from_str("run").unwrap();
    assert!(matches!(run, WorkflowNode::Run(_)));
    let parallel = WorkflowNode::from_str("parallel").unwrap();
    assert!(matches!(parallel, WorkflowNode::Parallel(_)));
    let service_call = WorkflowNode::from_str("service-call").unwrap();
    assert!(matches!(service_call, WorkflowNode::Run(_)));
}

#[test]
fn workflow_node_from_str_rejects_unknown() {
    use std::str::FromStr;
    let result = WorkflowNode::from_str("unknown");
    assert!(result.is_err());
    let result = WorkflowNode::from_str("");
    assert!(result.is_err());
}

#[test]
fn node_constant_dimensions() {
    assert_eq!(NODE_WIDTH, 220.0);
    assert_eq!(NODE_HEIGHT, 68.0);
    assert!(NODE_WIDTH > 0.0);
    assert!(NODE_HEIGHT > 0.0);
}

#[test]
fn bend_clamp_is_reasonable() {
    assert!(BEND_CLAMP > 0.0);
    assert!(BEND_CLAMP < 1000.0);
}
