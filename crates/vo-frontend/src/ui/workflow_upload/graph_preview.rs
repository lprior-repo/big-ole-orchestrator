//! Graph preview component for workflow definitions.
//!
//! Renders a simplified node-edge visualization of a parsed workflow
//! using SVG. Nodes are positioned based on the `x`/`y` coordinates
/// in the definition, with edges drawn as directed lines.

use dioxus::prelude::*;

use super::types::{EdgeConditionInput, NodeKindInput, WorkflowDefinition};

/// Render a simplified node-edge graph preview of a workflow definition.
///
/// Uses SVG to draw nodes as rounded rectangles and edges as arrows.
/// Positioned based on the `x`/`y` coordinates in the definition.
#[component]
pub fn GraphPreview(def: WorkflowDefinition) -> Element {
    let node_positions: std::collections::HashMap<String, (f64, f64)> = def
        .nodes
        .iter()
        .filter(|n| n.x.abs() > 0.0 || n.y.abs() > 0.0)
        .map(|n| (n.id.clone(), (n.x, n.y)))
        .collect();

    let _default_nodes: Vec<_> = def
        .nodes
        .iter()
        .filter(|n| node_positions.contains_key(&n.id))
        .collect();

    let has_positions = !node_positions.is_empty();

    // Auto-layout for nodes without positions
    let layout_positions = if !has_positions {
        compute_auto_layout(&def)
    } else {
        node_positions.clone()
    };

    // Calculate viewBox to fit all nodes
    let (view_width, view_height, _view_offset_x, _view_offset_y) =
        compute_view_box(&layout_positions, &def.nodes);

    let node_count = def.nodes.len();
    let edge_count = def.edges.len();
    let node_text = if node_count == 1 { "node" } else { "nodes" };
    let edge_text = if edge_count == 1 { "edge" } else { "edges" };

    rsx! {
        div {
            class: "border border-gray-200 rounded-lg overflow-hidden bg-white",
            // Header
            div {
                class: "flex items-center justify-between px-4 py-2 bg-gray-50 border-b border-gray-200",
                span { class: "text-sm font-medium text-gray-700", "Graph Preview" }
                span { class: "text-xs text-gray-500",
                    "{node_count} {node_text}, {edge_count} {edge_text}"
                }
            }
            // SVG graph
            div { class: "overflow-auto",
                svg {
                    class: "block",
                    width: "{view_width}",
                    height: "{view_height}",
                    view_box: "0 0 {view_width} {view_height}",
                    // Edges (drawn first, behind nodes)
                    for edge in &def.edges {
                        if let Some(preview) = build_edge_preview(
                            &layout_positions,
                            &edge.source,
                            &edge.target,
                            edge.condition.clone(),
                        ) {
                            preview
                        }
                    }
                    // Nodes
                    for node in &def.nodes {
                        if let Some(preview) = build_node_preview(
                            &layout_positions,
                            &node.id,
                            &node.name,
                            &node.kind,
                        ) {
                            preview
                        }
                    }
                }
            }
        }
    }
}

/// Preview of a single workflow node as a rounded rectangle.
#[component]
fn NodePreview(
    x: f64,
    y: f64,
    id: String,
    name: String,
    kind: NodeKindInput,
) -> Element {
    let node_width = 120.0;
    let node_height = 48.0;
    let color = node_color(&kind);
    let label = if name.len() > 18 {
        name[..15].to_string()
    } else {
        name
    };

    rsx! {
        g {
            transform: "translate({x}, {y})",
            // Node rectangle
            rect {
                x: "{-node_width / 2.0}",
                y: "{-node_height / 2.0}",
                width: "{node_width}",
                height: "{node_height}",
                rx: "6",
                fill: "{color}",
                stroke: "#374151",
                "stroke-width": "1.5",
            }
            // Kind badge at top
            rect {
                x: "{-node_width / 2.0 + 4.0}",
                y: "{-node_height / 2.0 + 4.0}",
                width: "60",
                height: "14",
                rx: "3",
                fill: "#1f2937",
            }
            text {
                x: "0",
                y: "-{node_height / 2.0 - 3.0}",
                "text-anchor": "middle",
                "dominant-baseline": "central",
                "font-size": "7",
                "font-weight": "bold",
                fill: "#ffffff",
                "font-family": "ui-monospace, monospace",
                "{kind_label(&kind)}"
            }
            // Node name
            text {
                x: "0",
                y: "{node_height / 2.0 - 10.0}",
                "text-anchor": "middle",
                "dominant-baseline": "central",
                "font-size": "11",
                fill: "#111827",
                "font-weight": "500",
                "{label}"
            }
            // Node ID (tiny)
            text {
                x: "0",
                y: "{node_height / 2.0 - 2.0}",
                "text-anchor": "middle",
                "dominant-baseline": "central",
                "font-size": "7",
                fill: "#6b7280",
                "font-family": "ui-monospace, monospace",
                "{id}"
            }
        }
    }
}

/// Preview of a single edge as a directed line with arrow.
#[component]
fn EdgePreview(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    condition: EdgeConditionInput,
) -> Element {
    let line_color = edge_color(&condition);
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1.0 {
        return rsx! {};
    }

    // Shorten line to stop at node boundary
    let node_radius = 60.0; // half node width
    let shorten = node_radius + 8.0; // node half-width + arrow gap
    let nx = dx / len;
    let ny = dy / len;
    let x1_adj = x1 + nx * shorten;
    let y1_adj = y1 + ny * shorten;
    let x2_adj = x2 - nx * shorten;
    let y2_adj = y2 - ny * shorten;

    // Arrow head
    let arrow_size = 6.0;
    let angle = dy.atan2(dx);
    let ax1 = x2_adj - arrow_size * (angle.cos() * 0.5 + angle.sin() * 0.866);
    let ay1 = y2_adj - arrow_size * (angle.sin() * 0.5 - angle.cos() * 0.866);
    let ax2 = x2_adj - arrow_size * (angle.cos() * 0.5 - angle.sin() * 0.866);
    let ay2 = y2_adj - arrow_size * (angle.sin() * 0.5 + angle.cos() * 0.866);

    let stroke_dasharray = if condition != EdgeConditionInput::Always {
        "4,3"
    } else {
        "none"
    };

    rsx! {
        g {
            // Line
            line {
                x1: "{x1_adj}",
                y1: "{y1_adj}",
                x2: "{x2_adj}",
                y2: "{y2_adj}",
                stroke: "{line_color}",
                "stroke-width": "1.5",
                "stroke-dasharray": "{stroke_dasharray}",
            }
            // Arrow head
            polygon {
                points: "{x2_adj},{y2_adj} {ax1},{ay1} {ax2},{ay2}",
                fill: "{line_color}",
            }
            // Condition label at midpoint
            if condition != EdgeConditionInput::Always {
                let mx_val = (x1_adj + x2_adj) / 2.0;
                let my_val = (y1_adj + y2_adj) / 2.0;
                text {
                    x: "{mx_val}",
                    y: "{my_val - 6.0}",
                    "text-anchor": "middle",
                    "dominant-baseline": "central",
                    "font-size": "7",
                    fill: "{line_color}",
                    "{condition_label(&condition)}"
                }
            }
        }
    }
}

/// Compute an auto-layout when no positions are specified.
///
/// Arranges nodes in a horizontal flow with wrapping.
fn compute_auto_layout(def: &WorkflowDefinition) -> std::collections::HashMap<String, (f64, f64)> {
    let mut positions = std::collections::HashMap::new();
    let node_spacing_x = 160.0;
    let node_spacing_y = 80.0;
    let start_x = 80.0;
    let start_y = 60.0;
    let max_per_row = 4;

    for (i, node) in def.nodes.iter().enumerate() {
        let col = i % max_per_row;
        let row = i / max_per_row;
        let x = start_x + col as f64 * node_spacing_x;
        let y = start_y + row as f64 * node_spacing_y;
        positions.insert(node.id.clone(), (x, y));
    }

    positions
}

/// Compute the SVG viewBox to fit all nodes.
fn compute_view_box(
    positions: &std::collections::HashMap<String, (f64, f64)>,
    _nodes: &[super::types::WorkflowNode],
) -> (f64, f64, f64, f64) {
    if positions.is_empty() {
        return (400.0, 300.0, 0.0, 0.0);
    }

    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;

    for &(x, y) in positions.values() {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }

    let padding = 100.0;
    (
        (max_x - min_x) + 2.0 * padding,
        (max_y - min_y) + 2.0 * padding,
        min_x - padding,
        min_y - padding,
    )
}

/// Color for a node based on its kind.
fn node_color(kind: &NodeKindInput) -> &'static str {
    match kind {
        NodeKindInput::Pure => "#dbeafe",      // blue-100
        NodeKindInput::ManagedEffect => "#d1fae5", // green-100
        NodeKindInput::Wait => "#fef3c7",       // amber-100
        NodeKindInput::Signal => "#fce7f3",     // pink-100
        NodeKindInput::Unsafe => "#fee2e2",     // red-100
        NodeKindInput::Router => "#e0e7ff",     // indigo-100
    }
}

/// Color for an edge based on its condition.
fn edge_color(condition: &EdgeConditionInput) -> &'static str {
    match condition {
        EdgeConditionInput::Always => "#6b7280",   // gray-500
        EdgeConditionInput::OnSuccess => "#10b981", // green-500
        EdgeConditionInput::OnFailure => "#ef4444", // red-500
    }
}

/// Human-readable label for a node kind.
fn kind_label(kind: &NodeKindInput) -> &'static str {
    match kind {
        NodeKindInput::Pure => "PURE",
        NodeKindInput::ManagedEffect => "EFFECT",
        NodeKindInput::Wait => "WAIT",
        NodeKindInput::Signal => "SIGNAL",
        NodeKindInput::Unsafe => "UNSAFE",
        NodeKindInput::Router => "ROUTER",
    }
}

/// Human-readable label for an edge condition.
fn condition_label(condition: &EdgeConditionInput) -> &'static str {
    match condition {
        EdgeConditionInput::Always => "always",
        EdgeConditionInput::OnSuccess => "success",
        EdgeConditionInput::OnFailure => "failure",
    }
}

/// Build an edge preview element from position map.
fn build_edge_preview(
    positions: &std::collections::HashMap<String, (f64, f64)>,
    source: &str,
    target: &str,
    condition: EdgeConditionInput,
) -> Option<Element> {
    let (sx, sy) = positions.get(source)?;
    let (tx, ty) = positions.get(target)?;
    Some(rsx! {
        EdgePreview {
            x1: *sx, y1: *sy,
            x2: *tx, y2: *ty,
            condition,
        }
    })
}

/// Build a node preview element from position map.
fn build_node_preview(
    positions: &std::collections::HashMap<String, (f64, f64)>,
    id: &str,
    name: &str,
    kind: &NodeKindInput,
) -> Option<Element> {
    let &(x, y) = positions.get(id)?;
    Some(rsx! {
        NodePreview {
            x, y,
            id: id.to_string(),
            name: name.to_string(),
            kind: kind.clone(),
        }
    })
}
