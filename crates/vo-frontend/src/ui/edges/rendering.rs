use dioxus::prelude::*;

use crate::ui::parallel_group_overlay::ParallelGroup;

/// Render the SVG defs section (gradients and markers).
pub(crate) fn render_svg_defs() -> Element {
    rsx! {
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
    }
}

/// Render a single parallel group overlay with optional branch count badge.
pub(crate) fn render_parallel_group(group: &ParallelGroup) -> Element {
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
    let key = format!(
        "parallel-group-{}-{}",
        group.bounding_box.x, group.bounding_box.y
    );

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
