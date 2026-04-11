use dioxus::prelude::*;

use crate::ui::editor_interactions::{NODE_HEIGHT, NODE_WIDTH};

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Rect {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

pub(crate) fn get_source_point(node: &oya_frontend::graph::Node) -> Position {
    Position {
        x: node.x + NODE_WIDTH,
        y: node.y + NODE_HEIGHT / 2.0,
    }
}

pub(crate) fn get_target_point(node: &oya_frontend::graph::Node) -> Position {
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

pub(crate) fn normalize_bend_delta(page_delta: f32, zoom: f32) -> f32 {
    if !zoom.is_finite() || zoom <= 0.0 {
        return 0.0;
    }
    page_delta / zoom
}
