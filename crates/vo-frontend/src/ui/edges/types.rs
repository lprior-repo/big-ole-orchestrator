use super::graph_types::Node;

pub(crate) const NODE_WIDTH: f32 = 220.0;
pub(crate) const NODE_HEIGHT: f32 = 68.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

pub(crate) const BEND_CLAMP: f32 = 200.0;

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct EdgeAnchor {
    pub(crate) from: Position,
    pub(crate) to: Position,
}

#[derive(Clone)]
pub struct DragState {
    pub edge_id: String,
    pub start_page_y: f32,
    pub start_bend: f32,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Rect {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

pub(crate) fn get_source_point(node: &Node) -> Position {
    Position {
        x: node.x + NODE_WIDTH,
        y: node.y + NODE_HEIGHT / 2.0,
    }
}

pub(crate) fn get_target_point(node: &Node) -> Position {
    Position {
        x: node.x,
        y: node.y + NODE_HEIGHT / 2.0,
    }
}

pub(crate) fn sanitize_bend_input_edge(input: f32, start_bend: f32) -> f32 {
    if !input.is_finite() {
        return start_bend;
    }
    input.clamp(-BEND_CLAMP, BEND_CLAMP)
}

#[allow(dead_code)]
pub(crate) fn normalize_bend_delta(page_delta: f32, zoom: f32) -> f32 {
    if !zoom.is_finite() || zoom <= 0.0 {
        return 0.0;
    }
    page_delta / zoom
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_equality() {
        let p1 = Position { x: 10.0, y: 20.0 };
        let p2 = Position { x: 10.0, y: 20.0 };
        let p3 = Position { x: 30.0, y: 40.0 };
        assert_eq!(p1, p2);
        assert_ne!(p1, p3);
    }

    #[test]
    fn position_copy() {
        let p1 = Position { x: 10.0, y: 20.0 };
        let p2 = p1;
        assert_eq!(p1, p2);
    }

    #[test]
    fn edge_anchor_clone() {
        let anchor = EdgeAnchor {
            from: Position { x: 0.0, y: 0.0 },
            to: Position { x: 100.0, y: 50.0 },
        };
        let cloned = anchor.clone();
        assert_eq!(anchor.from, cloned.from);
        assert_eq!(anchor.to, cloned.to);
    }

    #[test]
    fn drag_state_creation() {
        let state = DragState {
            edge_id: "edge-1".to_string(),
            start_page_y: 100.0,
            start_bend: 0.0,
        };
        assert_eq!(state.edge_id, "edge-1");
        assert_eq!(state.start_page_y, 100.0);
        assert_eq!(state.start_bend, 0.0);
    }

    #[test]
    fn rect_creation() {
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

    #[test]
    fn sanitize_bend_input_edge_within_bounds() {
        let result = sanitize_bend_input_edge(50.0, 0.0);
        assert_eq!(result, 50.0);
    }

    #[test]
    fn sanitize_bend_input_edge_at_upper_bound() {
        let result = sanitize_bend_input_edge(250.0, 0.0);
        assert_eq!(result, BEND_CLAMP);
    }

    #[test]
    fn sanitize_bend_input_edge_at_lower_bound() {
        let result = sanitize_bend_input_edge(-250.0, 0.0);
        assert_eq!(result, -BEND_CLAMP);
    }

    #[test]
    fn sanitize_bend_input_edge_with_nan_returns_start_bend() {
        let result = sanitize_bend_input_edge(f32::NAN, 42.0);
        assert_eq!(result, 42.0);
    }

    #[test]
    fn sanitize_bend_input_edge_with_infinity_returns_start_bend() {
        let result = sanitize_bend_input_edge(f32::INFINITY, 42.0);
        assert_eq!(result, 42.0);
        let result_neg = sanitize_bend_input_edge(f32::NEG_INFINITY, 42.0);
        assert_eq!(result_neg, 42.0);
    }

    #[test]
    fn normalize_bend_delta_normal_case() {
        let result = normalize_bend_delta(100.0, 2.0);
        assert_eq!(result, 50.0);
    }

    #[test]
    fn normalize_bend_delta_with_zoom_one() {
        let result = normalize_bend_delta(75.0, 1.0);
        assert_eq!(result, 75.0);
    }

    #[test]
    fn normalize_bend_delta_with_zoom_point_five() {
        let result = normalize_bend_delta(50.0, 0.5);
        assert_eq!(result, 100.0);
    }

    #[test]
    fn normalize_bend_delta_with_zero_zoom_returns_zero() {
        let result = normalize_bend_delta(100.0, 0.0);
        assert_eq!(result, 0.0);
    }

    #[test]
    fn normalize_bend_delta_with_negative_zoom_returns_zero() {
        let result = normalize_bend_delta(100.0, -1.0);
        assert_eq!(result, 0.0);
    }

    #[test]
    fn normalize_bend_delta_with_nan_zoom_returns_zero() {
        let result = normalize_bend_delta(100.0, f32::NAN);
        assert_eq!(result, 0.0);
    }

    #[test]
    fn normalize_bend_delta_with_infinity_zoom_returns_zero() {
        let result = normalize_bend_delta(100.0, f32::INFINITY);
        assert_eq!(result, 0.0);
    }

    #[test]
    fn get_source_point_calculates_correctly() {
        use crate::ui::edges::graph_types::{Node, NodeId, WorkflowNode};
        let node = Node::from_workflow_node(
            "test".to_string(),
            WorkflowNode::Run(Default::default()),
            100.0,
            50.0,
        );
        let point = get_source_point(&node);
        assert_eq!(point.x, 100.0 + crate::ui::edges::types::NODE_WIDTH);
        assert_eq!(point.y, 50.0 + crate::ui::edges::types::NODE_HEIGHT / 2.0);
    }

    #[test]
    fn get_target_point_calculates_correctly() {
        use crate::ui::edges::graph_types::{Node, NodeId, WorkflowNode};
        let node = Node::from_workflow_node(
            "test".to_string(),
            WorkflowNode::Run(Default::default()),
            100.0,
            50.0,
        );
        let point = get_target_point(&node);
        assert_eq!(point.x, 100.0);
        assert_eq!(point.y, 50.0 + crate::ui::edges::types::NODE_HEIGHT / 2.0);
    }
}
