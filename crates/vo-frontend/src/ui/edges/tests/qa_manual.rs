use super::helpers::*;
use crate::ui::edges::graph_types::{NodeId, WorkflowNode};
use crate::ui::edges::layout::create_smooth_step_path;
use crate::ui::edges::types::{
    get_source_point, get_target_point, sanitize_bend_input_edge, BEND_CLAMP, NODE_HEIGHT,
    NODE_WIDTH, Position,
};
use std::f32::consts::PI;

#[test]
fn given_node_when_get_source_point_then_returns_right_center() {
    let id = NodeId::new();
    let node = build_node(id, 100.0, 200.0);

    let point = get_source_point(&node);

    assert_eq!(point.x, 100.0 + NODE_WIDTH);
    assert_eq!(point.y, 200.0 + NODE_HEIGHT / 2.0);
}

#[test]
fn given_node_when_get_target_point_then_returns_left_center() {
    let id = NodeId::new();
    let node = build_node(id, 100.0, 200.0);

    let point = get_target_point(&node);

    assert_eq!(point.x, 100.0);
    assert_eq!(point.y, 200.0 + NODE_HEIGHT / 2.0);
}

#[test]
fn given_zero_coords_when_get_source_point_then_returns_width_and_half_height() {
    let id = NodeId::new();
    let node = build_node(id, 0.0, 0.0);

    let point = get_source_point(&node);

    assert_eq!(point.x, NODE_WIDTH);
    assert_eq!(point.y, NODE_HEIGHT / 2.0);
}

#[test]
fn given_negative_coords_when_get_target_point_then_returns_negative_x() {
    let id = NodeId::new();
    let node = build_node(id, -50.0, -100.0);

    let point = get_target_point(&node);

    assert_eq!(point.x, -50.0);
    assert_eq!(point.y, -100.0 + NODE_HEIGHT / 2.0);
}

#[test]
fn given_finite_input_within_clamp_when_sanitize_bend_then_returns_input() {
    let result = sanitize_bend_input_edge(50.0, 0.0);
    assert_eq!(result, 50.0);
}

#[test]
fn given_input_at_upper_clamp_when_sanitize_bend_then_returns_clamped() {
    let result = sanitize_bend_input_edge(BEND_CLAMP + 50.0, 0.0);
    assert_eq!(result, BEND_CLAMP);
}

#[test]
fn given_input_at_lower_clamp_when_sanitize_bend_then_returns_clamped() {
    let result = sanitize_bend_input_edge(-BEND_CLAMP - 50.0, 0.0);
    assert_eq!(result, -BEND_CLAMP);
}

#[test]
fn given_nan_input_when_sanitize_bend_then_returns_start_bend() {
    let result = sanitize_bend_input_edge(f32::NAN, 42.0);
    assert_eq!(result, 42.0);
}

#[test]
fn given_infinity_input_when_sanitize_bend_then_returns_start_bend() {
    let result = sanitize_bend_input_edge(f32::INFINITY, 42.0);
    assert_eq!(result, 42.0);
}

#[test]
fn given_neg_infinity_input_when_sanitize_bend_then_returns_start_bend() {
    let result = sanitize_bend_input_edge(f32::NEG_INFINITY, 42.0);
    assert_eq!(result, 42.0);
}

#[test]
fn given_zero_input_when_sanitize_bend_then_returns_zero() {
    let result = sanitize_bend_input_edge(0.0, 100.0);
    assert_eq!(result, 0.0);
}

#[test]
fn given_exactly_clamp_values_when_sanitize_bend_then_returns_exact() {
    assert_eq!(sanitize_bend_input_edge(BEND_CLAMP, 0.0), BEND_CLAMP);
    assert_eq!(sanitize_bend_input_edge(-BEND_CLAMP, 0.0), -BEND_CLAMP);
}

#[test]
fn given_horizontal_rightward_when_create_smooth_step_path_then_produces_path() {
    let from = Position { x: 100.0, y: 50.0 };
    let to = Position { x: 400.0, y: 50.0 };
    let (path, midpoint) = create_smooth_step_path(from, to, 0.0);

    assert!(!path.is_empty());
    assert!(path.starts_with("M "));
    assert_eq!(midpoint.x, 250.0);
    assert_eq!(midpoint.y, 50.0);
}

#[test]
fn given_vertical_connection_when_create_smooth_step_path_then_produces_straight_line() {
    let from = Position { x: 100.0, y: 50.0 };
    let to = Position { x: 101.0, y: 200.0 };
    let (path, midpoint) = create_smooth_step_path(from, to, 0.0);

    assert!(path.starts_with("M "));
    assert!(path.contains("L "));
    assert!(!path.contains("Q"));
    assert_eq!(midpoint.x, 100.5);
}

#[test]
fn given_downward_right_when_create_smooth_step_path_then_produces_step_path() {
    let from = Position { x: 100.0, y: 50.0 };
    let to = Position { x: 400.0, y: 200.0 };
    let (path, midpoint) = create_smooth_step_path(from, to, 0.0);

    assert!(path.contains("Q"));
    assert_eq!(midpoint.x, 250.0);
    assert_eq!(midpoint.y, f32::midpoint(50.0, 200.0));
}

#[test]
fn given_positive_bend_when_create_smooth_step_path_then_midpoint_shifted() {
    let from = Position { x: 100.0, y: 100.0 };
    let to = Position { x: 400.0, y: 100.0 };
    let bend = 50.0;
    let (path, midpoint) = create_smooth_step_path(from, to, bend);

    assert!(!path.is_empty());
    assert_eq!(midpoint.y, 100.0 + bend);
}

#[test]
fn given_negative_bend_when_create_smooth_step_path_then_midpoint_shifted_down() {
    let from = Position { x: 100.0, y: 100.0 };
    let to = Position { x: 400.0, y: 100.0 };
    let bend = -30.0;
    let (path, midpoint) = create_smooth_step_path(from, to, bend);

    assert!(!path.is_empty());
    assert_eq!(midpoint.y, 100.0 + bend);
}

#[test]
fn given_beyond_clamp_bend_when_create_smooth_step_path_then_clamps_to_max() {
    let from = Position { x: 100.0, y: 100.0 };
    let to = Position { x: 400.0, y: 100.0 };
    let (path, midpoint) = create_smooth_step_path(from, to, BEND_CLAMP + 500.0);

    assert!(!path.is_empty());
    assert_eq!(midpoint.y, 100.0 + BEND_CLAMP);
}

#[test]
fn given_zero_dx_when_create_smooth_step_path_then_returns_straight_line() {
    let from = Position { x: 100.0, y: 50.0 };
    let to = Position { x: 100.0, y: 200.0 };
    let (path, _midpoint) = create_smooth_step_path(from, to, 0.0);

    assert!(path.starts_with("M 100 50 L 100 200"));
}

#[test]
fn given_nan_dx_when_create_smooth_step_path_then_returns_fallback_line() {
    let from = Position {
        x: 100.0,
        y: 50.0,
    };
    let to = Position {
        x: f32::NAN,
        y: 200.0,
    };
    let (path, _midpoint) = create_smooth_step_path(from, to, 0.0);

    assert!(path.starts_with("M "));
    assert!(path.contains("L "));
}

#[test]
fn given_nan_dy_when_create_smooth_step_path_then_returns_fallback_line() {
    let from = Position {
        x: 100.0,
        y: 50.0,
    };
    let to = Position {
        x: 400.0,
        y: f32::NAN,
    };
    let (path, _midpoint) = create_smooth_step_path(from, to, 0.0);

    assert!(path.starts_with("M "));
    assert!(path.contains("L "));
}

#[test]
fn given_leftward_connection_when_create_smooth_step_path_then_handles_negative_dx() {
    let from = Position { x: 400.0, y: 50.0 };
    let to = Position { x: 100.0, y: 200.0 };
    let (path, midpoint) = create_smooth_step_path(from, to, 0.0);

    assert!(!path.is_empty());
    assert_eq!(midpoint.x, 250.0);
}

#[test]
fn given_same_points_when_create_smooth_step_path_then_returns_degenerate_path() {
    let from = Position { x: 100.0, y: 50.0 };
    let to = Position { x: 100.0, y: 50.0 };
    let (path, _midpoint) = create_smooth_step_path(from, to, 0.0);

    assert!(path.starts_with("M 100 50 L 100 50"));
}

#[test]
fn given_very_small_dx_when_create_smooth_step_path_then_returns_straight_line() {
    let from = Position { x: 100.0, y: 50.0 };
    let to = Position { x: 101.5, y: 200.0 };
    let (path, _midpoint) = create_smooth_step_path(from, to, 0.0);

    assert!(path.contains("L "));
}

#[test]
fn given_node_from_workflow_node_when_created_then_has_idle_execution_state() {
    let node =
        crate::ui::edges::graph_types::Node::from_workflow_node("test".to_string(), WorkflowNode::Run(crate::ui::edges::graph_types::RunConfig::default()), 10.0, 20.0);

    assert_eq!(
        node.execution_state,
        crate::ui::edges::graph_types::ExecutionState::Idle
    );
    assert_eq!(node.name, "test");
    assert_eq!(node.x, 10.0);
    assert_eq!(node.y, 20.0);
}

#[test]
fn given_workflow_node_from_str_when_run_then_returns_run_variant() {
    let result = "run".parse::<WorkflowNode>();
    assert!(matches!(result, Ok(WorkflowNode::Run(_))));
}

#[test]
fn given_workflow_node_from_str_when_parallel_then_returns_parallel_variant() {
    let result = "parallel".parse::<WorkflowNode>();
    assert!(matches!(result, Ok(WorkflowNode::Parallel(_))));
}

#[test]
fn given_workflow_node_from_str_when_service_call_then_returns_run_variant() {
    let result = "service-call".parse::<WorkflowNode>();
    assert!(matches!(result, Ok(WorkflowNode::Run(_))));
}

#[test]
fn given_workflow_node_from_str_when_unknown_then_returns_error() {
    let result = "unknown".parse::<WorkflowNode>();
    assert!(result.is_err());
}

#[test]
fn given_workflow_node_is_parallel_when_parallel_then_returns_true() {
    let node = WorkflowNode::Parallel(crate::ui::edges::graph_types::ParallelConfig::default());
    assert!(node.is_parallel());
}

#[test]
fn given_workflow_node_is_parallel_when_run_then_returns_false() {
    let node = WorkflowNode::Run(crate::ui::edges::graph_types::RunConfig::default());
    assert!(!node.is_parallel());
}

#[test]
fn given_node_id_default_then_generates_unique_ids() {
    let id1 = NodeId::default();
    let id2 = NodeId::default();
    assert_ne!(id1, id2);
}

#[test]
fn given_node_id_display_then_shows_uuid() {
    let id = NodeId::default();
    let display = format!("{id}");
    assert!(!display.is_empty());
    assert_eq!(display.len(), 36);
}

#[test]
fn given_execution_state_default_then_is_idle() {
    assert_eq!(
        crate::ui::edges::graph_types::ExecutionState::default(),
        crate::ui::edges::graph_types::ExecutionState::Idle
    );
}
