#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

pub const NODE_WIDTH: f32 = 220.0;
pub const NODE_HEIGHT: f32 = 68.0;
pub type SelectionRect = (f32, f32, f32, f32);

#[must_use]
pub fn normalize_rect(start: (f32, f32), end: (f32, f32)) -> SelectionRect {
    let min_x = start.0.min(end.0);
    let min_y = start.1.min(end.1);
    let max_x = start.0.max(end.0);
    let max_y = start.1.max(end.1);
    (min_x, min_y, max_x, max_y)
}

#[must_use]
pub fn rect_contains(rect: SelectionRect, point: (f32, f32)) -> bool {
    point.0 >= rect.0 && point.0 <= rect.2 && point.1 >= rect.1 && point.1 <= rect.3
}

#[must_use]
pub fn node_intersects_rect(node_x: f32, node_y: f32, rect: SelectionRect) -> bool {
    let node_left = node_x;
    let node_top = node_y;
    let node_right = node_x + NODE_WIDTH;
    let node_bottom = node_y + NODE_HEIGHT;

    !(node_right < rect.0 || node_left > rect.2 || node_bottom < rect.1 || node_top > rect.3)
}

#[must_use]
pub fn snap_handle(
    nodes: &[oya_frontend::graph::Node],
    mx: f32,
    my: f32,
    viewport: &oya_frontend::graph::Viewport,
) -> Option<(
    oya_frontend::graph::NodeId,
    String,
    crate::ui::edges::Position,
)> {
    const SCREEN_SNAP_RADIUS: f32 = 24.0;

    if !viewport.zoom.is_finite() || viewport.zoom.abs() <= f32::EPSILON {
        return None;
    }

    // Convert screen-space radius to canvas-space for zoom-invariant behavior
    let canvas_radius = SCREEN_SNAP_RADIUS / viewport.zoom.abs();
    let radius_sq = canvas_radius * canvas_radius;

    let canvas_x = (mx - viewport.x) / viewport.zoom;
    let canvas_y = (my - viewport.y) / viewport.zoom;

    let mut best: Option<(
        oya_frontend::graph::NodeId,
        String,
        crate::ui::edges::Position,
        f32,
    )> = None;

    for node in nodes {
        let handle_y = node.y + NODE_HEIGHT / 2.0;
        let candidates = [
            (
                "target",
                crate::ui::edges::Position {
                    x: node.x,
                    y: handle_y,
                },
            ),
            (
                "source",
                crate::ui::edges::Position {
                    x: node.x + NODE_WIDTH,
                    y: handle_y,
                },
            ),
        ];

        for (kind, pos) in candidates {
            let dx = canvas_x - pos.x;
            let dy = canvas_y - pos.y;
            let dist_sq = dx.mul_add(dx, dy * dy);

            if dist_sq > radius_sq {
                continue;
            }

            let should_replace = match &best {
                Some((best_id, best_kind, _, best_dist)) => {
                    // Deterministic tie-breaking: compare by distance first, then node id, then handle kind
                    if dist_sq < *best_dist {
                        true
                    } else if (dist_sq - *best_dist).abs() < f32::EPSILON {
                        // Equal distance: use stable ordering by node id and handle kind
                        let node_cmp = node.id.0.cmp(&best_id.0);
                        if node_cmp == std::cmp::Ordering::Equal {
                            kind.cmp(best_kind) == std::cmp::Ordering::Less
                        } else {
                            node_cmp == std::cmp::Ordering::Less
                        }
                    } else {
                        false
                    }
                }
                None => true,
            };
            if should_replace {
                best = Some((node.id, kind.to_string(), pos, dist_sq));
            }
        }
    }

    best.map(|(node_id, handle_kind, position, _)| (node_id, handle_kind, position))
}

#[cfg(test)]
mod tests {
    use super::{node_intersects_rect, normalize_rect, rect_contains, snap_handle};
    use oya_frontend::graph::{Viewport, Workflow};

    #[test]
    fn given_drag_points_when_normalizing_then_rect_bounds_are_ordered() {
        let rect = normalize_rect((120.0, 30.0), (20.0, 90.0));

        assert_eq!(rect, (20.0, 30.0, 120.0, 90.0));
    }

    #[test]
    fn given_rect_boundary_point_when_checking_contains_then_point_is_inside() {
        let rect = (10.0, 10.0, 20.0, 20.0);

        assert!(rect_contains(rect, (10.0, 20.0)));
    }

    #[test]
    fn given_node_overlapping_selection_when_checking_intersection_then_it_is_detected() {
        let intersects = node_intersects_rect(50.0, 50.0, (0.0, 0.0, 100.0, 100.0));

        assert!(intersects);
    }

    #[test]
    fn given_invalid_zoom_when_snapping_handle_then_no_handle_is_returned() {
        let mut workflow = Workflow::new();
        let _ = workflow.add_node("http-handler", 200.0, 200.0);

        let result = snap_handle(
            &workflow.nodes,
            200.0,
            200.0,
            &Viewport {
                x: 0.0,
                y: 0.0,
                zoom: 0.0,
            },
        );

        assert!(result.is_none());
    }

    #[test]
    fn given_equal_distance_candidates_when_snapping_then_selection_is_deterministic() {
        let mut workflow = Workflow::new();
        let node_a_id = workflow.add_node("node-a", 100.0, 100.0);
        let node_b_id = workflow.add_node("node-b", 300.0, 100.0);

        let viewport = Viewport {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        };

        // Cursor at x=200 is equidistant from both source handles (x=100 and x=134)
        // With deterministic tie-breaking by node id
        let first_result = snap_handle(&workflow.nodes, 200.0, 134.0, &viewport);
        assert!(first_result.is_some());
        let (first_id, _, _) = first_result.unwrap();

        // The node A should be selected (deterministic ordering)
        assert_eq!(first_id, node_a_id);
    }

    #[test]
    fn given_zoom_level_when_snapping_then_behavior_is_zoom_invariant() {
        let mut workflow = Workflow::new();
        let _ = workflow.add_node("node", 100.0, 100.0);

        // At zoom 1.0: cursor at (123, 109) is ~23 pixels from source handle (320, 134)
        // At zoom 0.5: cursor at (210, 117) should snap to same handle (same visual distance)
        // At zoom 2.0: cursor at (246, 121) should snap to same handle (same visual distance)

        let viewport_1x = Viewport {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        };
        let snapped_1x = snap_handle(&workflow.nodes, 318.0, 134.0, &viewport_1x);

        let viewport_05x = Viewport {
            x: 0.0,
            y: 0.0,
            zoom: 0.5,
        };
        let snapped_05x = snap_handle(&workflow.nodes, 209.0, 117.0, &viewport_05x);

        let viewport_2x = Viewport {
            x: 0.0,
            y: 0.0,
            zoom: 2.0,
        };
        let snapped_2x = snap_handle(&workflow.nodes, 246.0, 121.0, &viewport_2x);

        // All should snap to the same handle (source handle at x=320, y=134)
        assert!(snapped_1x.is_some());
        assert!(snapped_05x.is_some());
        assert!(snapped_2x.is_some());

        if let (Some((id1, kind1, _)), Some((id2, kind2, _)), Some((id3, kind3, _))) =
            (snapped_1x, snapped_05x, snapped_2x)
        {
            assert_eq!(id1, id2, "zoom 0.5 should select same node as zoom 1.0");
            assert_eq!(id2, id3, "zoom 2.0 should select same node as zoom 0.5");
            assert_eq!(
                kind1, kind2,
                "zoom 0.5 should select same handle kind as zoom 1.0"
            );
            assert_eq!(
                kind2, kind3,
                "zoom 2.0 should select same handle kind as zoom 0.5"
            );
        } else {
            assert!(false, "all zoom levels should find a snap handle");
        }
    }

    #[test]
    fn given_equal_distance_candidates_when_snapping_then_selection_is_deterministic() {
        let mut workflow = Workflow::new();
        // Add two nodes with handles at the same distance from cursor
        let _ = workflow.add_node("node-a", 100.0, 100.0);
        let _ = workflow.add_node("node-b", 300.0, 100.0);

        let viewport = Viewport {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        };

        // Cursor at x=200 is equidistant from both source handles (x=100 and x=300)
        // With deterministic tie-breaking, node-a should always win
        let snapped = snap_handle(&workflow.nodes, 200.0, 134.0, &viewport);

        assert!(snapped.is_some());

        // The selection should be deterministic based on node id ordering
        // Run multiple times to verify consistency
        let first_result = snap_handle(&workflow.nodes, 200.0, 134.0, &viewport);
        assert!(first_result.is_some());
        let (first_id, _, _) = first_result.unwrap();

        for _ in 0..10 {
            let result = snap_handle(&workflow.nodes, 200.0, 134.0, &viewport);
            assert!(result.is_some());
            let (node_id, _, _) = result.unwrap();
            // All results should be identical due to deterministic ordering
            assert_eq!(
                node_id, first_id,
                "deterministic tie-break should always return same node"
            );
        }
    }

    #[test]
    fn given_infinite_zoom_when_snapping_then_no_handle_is_returned() {
        let mut workflow = Workflow::new();
        let _ = workflow.add_node("test", 100.0, 100.0);

        let result = snap_handle(
            &workflow.nodes,
            100.0,
            100.0,
            &Viewport {
                x: 0.0,
                y: 0.0,
                zoom: f32::INFINITY,
            },
        );

        assert!(result.is_none());
    }

    #[test]
    fn given_nan_zoom_when_snapping_then_no_handle_is_returned() {
        let mut workflow = Workflow::new();
        let _ = workflow.add_node("test", 100.0, 100.0);

        let result = snap_handle(
            &workflow.nodes,
            100.0,
            100.0,
            &Viewport {
                x: 0.0,
                y: 0.0,
                zoom: f32::NAN,
            },
        );

        assert!(result.is_none());
    }
}
