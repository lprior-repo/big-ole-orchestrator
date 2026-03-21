#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

use dioxus::prelude::*;
use oya_frontend::graph::{Connection, Node, NodeId, Viewport};

// ── Constants ─────────────────────────────────────────────────────────────────

const NODE_W: f32 = 220.0;
const NODE_H: f32 = 68.0;
/// Padding added around node bounds so nodes never sit right on the SVG edge.
const SCENE_PAD: f32 = 60.0;

// ── Data ──────────────────────────────────────────────────────────────────────

/// Axis-aligned bounding box of all nodes in scene-space, with padding.
#[derive(Clone, Copy, Debug, PartialEq)]
struct SceneBounds {
    min_x: f32,
    min_y: f32,
    width: f32,
    height: f32,
}

/// The camera rectangle projected into scene-space coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ViewportRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

/// Fill / stroke pair for a minimap node rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
struct NodeColors {
    fill: &'static str,
    stroke: &'static str,
}

// ── Calculations ──────────────────────────────────────────────────────────────
// Every function here is pure: same inputs → same output, no side effects.

/// Compute scene bounds from a slice of nodes.
/// Falls back to a wide default when the canvas is empty so the SVG stays
/// well-formed.
fn scene_bounds(nodes: &[Node]) -> SceneBounds {
    nodes
        .iter()
        .fold(None::<(f32, f32, f32, f32)>, |acc, n| {
            let r = n.x + NODE_W;
            let b = n.y + NODE_H;
            Some(match acc {
                Some((x0, y0, x1, y1)) => (x0.min(n.x), y0.min(n.y), x1.max(r), y1.max(b)),
                None => (n.x, n.y, r, b),
            })
        })
        .map_or(
            SceneBounds {
                min_x: -200.0,
                min_y: -200.0,
                width: 1200.0,
                height: 1000.0,
            },
            |(x0, y0, x1, y1)| SceneBounds {
                min_x: x0 - SCENE_PAD,
                min_y: y0 - SCENE_PAD,
                width: SCENE_PAD.mul_add(2.0, x1 - x0).max(1.0),
                height: SCENE_PAD.mul_add(2.0, y1 - y0).max(1.0),
            },
        )
}

/// Project the camera viewport into scene-space so it can be drawn as a rect
/// inside the minimap SVG.
///
/// Scene coordinates:  `x_scene = (screen_x − vp.x) / vp.zoom`
/// At `screen_x = 0`:  `x_scene = −vp.x / zoom`
fn viewport_rect(vp: &Viewport, canvas_w: f32, canvas_h: f32) -> ViewportRect {
    let zoom = if vp.zoom.is_finite() && vp.zoom > 0.0 {
        vp.zoom
    } else {
        1.0
    };
    ViewportRect {
        x: -vp.x / zoom,
        y: -vp.y / zoom,
        width: canvas_w / zoom,
        height: canvas_h / zoom,
    }
}

/// Category-keyed fill/stroke colours for minimap node rects.
fn node_colors(category: &str, selected: bool) -> NodeColors {
    if selected {
        return NodeColors {
            fill: "rgba(99,102,241,0.40)",
            stroke: "rgba(129,140,248,0.90)",
        };
    }
    match category {
        "entry" => NodeColors {
            fill: "rgba(96,165,250,0.25)",
            stroke: "rgba(96,165,250,0.70)",
        },
        "durable" => NodeColors {
            fill: "rgba(74,222,128,0.25)",
            stroke: "rgba(74,222,128,0.70)",
        },
        "state" => NodeColors {
            fill: "rgba(34,211,238,0.25)",
            stroke: "rgba(34,211,238,0.70)",
        },
        "flow" => NodeColors {
            fill: "rgba(244,114,182,0.25)",
            stroke: "rgba(244,114,182,0.70)",
        },
        "timing" => NodeColors {
            fill: "rgba(192,132,252,0.25)",
            stroke: "rgba(192,132,252,0.70)",
        },
        "signal" => NodeColors {
            fill: "rgba(251,191,36,0.25)",
            stroke: "rgba(251,191,36,0.70)",
        },
        _ => NodeColors {
            fill: "rgba(100,116,139,0.25)",
            stroke: "rgba(100,116,139,0.70)",
        },
    }
}

// ── Component (Action layer) ──────────────────────────────────────────────────
// Reads signals, calls pure calc functions, renders.  No mutation.

#[component]
pub fn FlowMinimap(
    nodes: ReadSignal<Vec<Node>>,
    edges: ReadSignal<Vec<Connection>>,
    selected_node_id: ReadSignal<Option<NodeId>>,
    viewport: ReadSignal<Viewport>,
    canvas_width: f32,
    canvas_height: f32,
    on_zoom_in: EventHandler<MouseEvent>,
    on_zoom_out: EventHandler<MouseEvent>,
    on_fit_view: EventHandler<MouseEvent>,
) -> Element {
    let node_list = nodes.read().clone();
    let edge_list = edges.read().clone();
    let vp = viewport.read().clone();
    let sel_id = *selected_node_id.read();

    // ── pure calculations ─────────────────────────────────────────────────
    let bounds = scene_bounds(&node_list);
    let vp_rect = viewport_rect(&vp, canvas_width, canvas_height);

    // Fast O(n) lookup: NodeId → &Node (lives for this render frame)
    let node_map: std::collections::HashMap<NodeId, &Node> =
        node_list.iter().map(|n| (n.id, n)).collect();

    let viewbox = format!(
        "{} {} {} {}",
        bounds.min_x, bounds.min_y, bounds.width, bounds.height
    );
    let node_total = node_list.len();
    let edge_total = edge_list.len();

    // ── render ────────────────────────────────────────────────────────────
    rsx! {
        div {
            class: "pointer-events-none absolute bottom-4 right-4 h-[138px] w-[220px] \
                    overflow-hidden rounded-xl border border-slate-700/80 bg-gradient-to-br \
                    from-slate-950/95 via-slate-900/95 to-cyan-950/60 \
                    shadow-2xl shadow-slate-950/70 backdrop-blur-sm",

            div {
                class: "absolute left-2 top-2 z-10 flex items-center gap-2 rounded-md border border-slate-700 bg-slate-900/85 px-2 py-1 text-[10px]",
                span { class: "font-semibold uppercase tracking-wide text-slate-300", "Map" }
                span { class: "rounded border border-cyan-800/70 bg-cyan-900/40 px-1.5 py-px text-cyan-200", "{node_total} nodes" }
                span { class: "rounded border border-slate-700 px-1.5 py-px text-slate-300", "{edge_total} links" }
            }

            div { class: "pointer-events-auto absolute right-2 top-2 z-10 flex items-center gap-1",
                button {
                    class: "flex h-5 min-w-5 items-center justify-center rounded border border-slate-700 bg-slate-900/90 px-1 text-[10px] font-semibold text-slate-200 transition-colors hover:border-cyan-500/60 hover:text-cyan-200",
                    title: "Zoom out",
                    onclick: move |evt| on_zoom_out.call(evt),
                    "-"
                }
                button {
                    class: "flex h-5 min-w-5 items-center justify-center rounded border border-slate-700 bg-slate-900/90 px-1 text-[10px] font-semibold text-slate-200 transition-colors hover:border-cyan-500/60 hover:text-cyan-200",
                    title: "Zoom in",
                    onclick: move |evt| on_zoom_in.call(evt),
                    "+"
                }
                button {
                    class: "flex h-5 items-center justify-center rounded border border-slate-700 bg-slate-900/90 px-1.5 text-[9px] font-semibold uppercase tracking-wide text-slate-200 transition-colors hover:border-cyan-500/60 hover:text-cyan-200",
                    title: "Fit view",
                    onclick: move |evt| on_fit_view.call(evt),
                    "Fit"
                }
            }

            svg {
                view_box: "{viewbox}",
                class: "h-full w-full pt-7",
                xmlns: "http://www.w3.org/2000/svg",

                // ── Edges ───────────────────────────────────────────────
                for edge in edge_list {
                    {
                        match (node_map.get(&edge.source), node_map.get(&edge.target)) {
                            (Some(src), Some(tgt)) => rsx! {
                                line {
                                    key: "e-{edge.id}",
                                    x1: "{src.x + NODE_W / 2.0}",
                                    y1: "{src.y + NODE_H}",
                                    x2: "{tgt.x + NODE_W / 2.0}",
                                    y2: "{tgt.y}",
                                    stroke: "rgba(148,163,184,0.42)",
                                    stroke_width: "4",
                                }
                            },
                            _ => rsx! {},
                        }
                    }
                }

                // ── Nodes ───────────────────────────────────────────────
                for node in node_list {
                    {
                        let colors = node_colors(
                            &node.category.to_string(),
                            sel_id.is_some_and(|id| id == node.id),
                        );
                        rsx! {
                            rect {
                                key: "n-{node.id}",
                                x: "{node.x}",
                                y: "{node.y}",
                                width: "{NODE_W}",
                                height: "{NODE_H}",
                                rx: "6",
                                fill: "{colors.fill}",
                                stroke: "{colors.stroke}",
                                stroke_width: "3",
                            }
                        }
                    }
                }

                // ── Viewport indicator ──────────────────────────────────
                rect {
                    x: "{vp_rect.x}",
                    y: "{vp_rect.y}",
                    width: "{vp_rect.width}",
                    height: "{vp_rect.height}",
                    fill: "rgba(34,211,238,0.05)",
                    stroke: "rgba(125,211,252,0.58)",
                    stroke_width: "6",
                    rx: "4",
                }
            }
        }
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use oya_frontend::graph::workflow_node::WorkflowNode;

    fn make_node(x: f32, y: f32) -> Node {
        Node::from_workflow_node(
            String::new(),
            WorkflowNode::Run(oya_frontend::graph::workflow_node::RunConfig::default()),
            x,
            y,
        )
    }

    #[test]
    fn given_empty_nodes_when_computing_bounds_then_fallback_is_used() {
        let bounds = scene_bounds(&[]);

        assert_eq!(bounds.min_x, -200.0);
        assert!(bounds.width > 0.0);
    }

    #[test]
    fn given_single_node_when_computing_bounds_then_node_is_padded() {
        let node = make_node(100.0, 50.0);
        let bounds = scene_bounds(&[node]);

        assert_eq!(bounds.min_x, 100.0 - SCENE_PAD);
        assert_eq!(bounds.min_y, 50.0 - SCENE_PAD);
    }

    #[test]
    fn given_valid_viewport_when_projecting_rect_then_math_is_correct() {
        let vp = Viewport {
            x: 200.0,
            y: 100.0,
            zoom: 2.0,
        };
        let rect = viewport_rect(&vp, 800.0, 600.0);

        assert_eq!(rect.x, -100.0); // -200 / 2
        assert_eq!(rect.y, -50.0); // -100 / 2
        assert_eq!(rect.width, 400.0); // 800 / 2
        assert_eq!(rect.height, 300.0); // 600 / 2
    }

    #[test]
    fn given_zero_zoom_when_projecting_rect_then_fallback_zoom_of_one_is_used() {
        let vp = Viewport {
            x: 0.0,
            y: 0.0,
            zoom: 0.0,
        };
        let rect = viewport_rect(&vp, 800.0, 600.0);

        assert_eq!(rect.width, 800.0);
    }

    #[test]
    fn given_selected_node_when_getting_colors_then_indigo_is_returned() {
        let colors = node_colors("entry", true);

        assert_eq!(colors.fill, "rgba(99,102,241,0.40)");
    }

    #[test]
    fn given_durable_category_when_getting_colors_then_green_is_returned() {
        let colors = node_colors("durable", false);

        assert_eq!(colors.stroke, "rgba(74,222,128,0.70)");
    }
}
