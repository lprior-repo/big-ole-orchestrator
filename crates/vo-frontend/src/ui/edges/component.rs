use super::graph_types::{Connection, ExecutionState, Node, NodeId};
use dioxus::prelude::*;
use std::collections::HashMap;

use super::layout::{
    create_smooth_step_path, find_parallel_branches, resolve_edge_anchors_with_parallel,
};
use super::rendering::{render_parallel_group, render_svg_defs};
use super::types::{sanitize_bend_input_edge, DragState, Position};

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

    let node_by_id = use_memo(move || {
        nodes
            .read()
            .iter()
            .cloned()
            .map(|node| (node.id, node))
            .collect::<HashMap<_, _>>()
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
            {render_svg_defs()}

            for group in parallel_groups.read().iter() {
                {render_parallel_group(group)}
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
                            .map(|node| match node.execution_state {
                                ExecutionState::Running => "running",
                                ExecutionState::Queued => "running",
                                ExecutionState::Completed => "completed",
                                ExecutionState::Failed => "failed",
                                ExecutionState::Idle | ExecutionState::Skipped => "pending",
                            })
                            .unwrap_or("pending");
                        let target_is_running = running_node_ids
                            .read()
                            .contains(&edge.target);
                        let stroke_color = match source_status {
                            "running" => "url(#edge-running-gradient)",
                            "completed" => "rgba(16, 185, 129, 0.85)",
                            "failed" => "rgba(244, 63, 94, 0.85)",
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
