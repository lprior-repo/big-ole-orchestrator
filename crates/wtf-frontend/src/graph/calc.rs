#[must_use]
pub fn calculate_zoom_delta(delta: f32, current_zoom: f32) -> f32 {
    if !delta.is_finite() {
        return current_zoom.clamp(0.1, 5.0);
    }
    if !current_zoom.is_finite() || current_zoom <= 0.0 {
        return 1.0;
    }

    let new_zoom = current_zoom * (1.0 + delta);
    new_zoom.clamp(0.1, 5.0)
}

#[must_use]
pub fn calculate_pan_offset(
    viewport_x: f32,
    viewport_y: f32,
    center_x: f32,
    center_y: f32,
    old_zoom: f32,
    new_zoom: f32,
) -> (f32, f32) {
    if !viewport_x.is_finite()
        || !viewport_y.is_finite()
        || !center_x.is_finite()
        || !center_y.is_finite()
        || !old_zoom.is_finite()
        || !new_zoom.is_finite()
        || old_zoom <= 0.0
    {
        return (viewport_x, viewport_y);
    }

    let factor = new_zoom / old_zoom;
    if !factor.is_finite() {
        return (viewport_x, viewport_y);
    }

    let new_x = (center_x - viewport_x).mul_add(-factor, center_x);
    let new_y = (center_y - viewport_y).mul_add(-factor, center_y);

    if !new_x.is_finite() || !new_y.is_finite() {
        return (viewport_x, viewport_y);
    }

    (new_x, new_y)
}

#[must_use]
pub fn calculate_fit_view(
    nodes: &[(f32, f32)],
    viewport_width: f32,
    viewport_height: f32,
    padding: f32,
) -> Option<(f32, f32, f32)> {
    if nodes.is_empty() {
        return None;
    }

    if !viewport_width.is_finite()
        || !viewport_height.is_finite()
        || !padding.is_finite()
        || viewport_width <= 0.0
        || viewport_height <= 0.0
        || padding < 0.0
    {
        return None;
    }

    if nodes.iter().any(|(x, y)| !x.is_finite() || !y.is_finite()) {
        return None;
    }

    let (min_x, min_y, max_x, max_y) = nodes.iter().fold(
        (
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ),
        |(min_x, min_y, max_x, max_y), &(x, y)| {
            let right = x + 220.0;
            let bottom = y + 68.0;
            (
                min_x.min(x),
                min_y.min(y),
                max_x.max(right),
                max_y.max(bottom),
            )
        },
    );

    let width = max_x - min_x;
    let height = max_y - min_y;

    let scale_x = (viewport_width - padding) / width.max(1.0);
    let scale_y = (viewport_height - padding) / height.max(1.0);
    let zoom = scale_x.min(scale_y).clamp(0.15, 1.5);

    let center_x = f32::midpoint(min_x, max_x);
    let center_y = f32::midpoint(min_y, max_y);

    let viewport_x = center_x.mul_add(-zoom, viewport_width / 2.0);
    let viewport_y = center_y.mul_add(-zoom, viewport_height / 2.0);

    Some((viewport_x, viewport_y, zoom))
}

#[must_use]
pub fn find_safe_position(
    existing_positions: &[(f32, f32)],
    desired_x: f32,
    desired_y: f32,
    step: f32,
) -> (f32, f32) {
    let mut current_x = desired_x;
    let mut current_y = desired_y;

    while existing_positions
        .iter()
        .any(|&(ex, ey)| (ex - current_x).abs() < 10.0 && (ey - current_y).abs() < 10.0)
    {
        current_x += step;
        current_y += step;
    }

    (current_x, current_y)
}

#[must_use]
pub fn update_node_position(current_x: f32, current_y: f32, dx: f32, dy: f32) -> (f32, f32) {
    // Safety check: if any value is NaN or infinite, don't update
    if !dx.is_finite() || !dy.is_finite() || !current_x.is_finite() || !current_y.is_finite() {
        return (current_x, current_y);
    }

    let new_x = ((current_x + dx) / 10.0).round() * 10.0;
    let new_y = ((current_y + dy) / 10.0).round() * 10.0;

    // Additional safety: clamp to reasonable bounds
    let new_x = new_x.clamp(-100_000.0, 100_000.0);
    let new_y = new_y.clamp(-100_000.0, 100_000.0);

    (new_x, new_y)
}

#[must_use]
pub const fn calculate_rect_center(rect: (f32, f32, f32, f32)) -> (f32, f32) {
    let (min_x, min_y, max_x, max_y) = rect;
    (f32::midpoint(min_x, max_x), f32::midpoint(min_y, max_y))
}

#[must_use]
pub fn calculate_rect_size(rect: (f32, f32, f32, f32)) -> (f32, f32) {
    let (min_x, min_y, max_x, max_y) = rect;
    (max_x - min_x, max_y - min_y)
}

#[cfg(test)]
mod tests {
    use super::{
        calculate_fit_view, calculate_pan_offset, calculate_zoom_delta, update_node_position,
    };

    #[test]
    fn given_small_drag_delta_when_updating_node_position_then_position_moves_by_snap_grid() {
        let (x, y) = update_node_position(350.0, 170.0, 6.0, -4.0);

        assert_eq!((x, y), (360.0, 170.0));
    }

    #[test]
    fn given_zero_drag_delta_when_updating_node_position_then_position_stays_unchanged() {
        let (x, y) = update_node_position(420.0, 240.0, 0.0, 0.0);

        assert_eq!((x, y), (420.0, 240.0));
    }

    #[test]
    fn given_non_finite_zoom_inputs_when_calculating_zoom_delta_then_result_is_deterministic() {
        assert_eq!(calculate_zoom_delta(f32::NAN, 1.2), 1.2);
        assert_eq!(calculate_zoom_delta(0.2, f32::NAN), 1.0);
    }

    #[test]
    fn given_invalid_pan_zoom_inputs_when_calculating_offset_then_viewport_is_unchanged() {
        let result = calculate_pan_offset(10.0, 20.0, 200.0, 120.0, 0.0, 2.0);

        assert_eq!(result, (10.0, 20.0));
    }

    #[test]
    fn given_non_positive_viewport_when_calculating_fit_view_then_result_is_none() {
        let nodes = [(10.0, 20.0), (40.0, 60.0)];

        assert_eq!(calculate_fit_view(&nodes, 0.0, 500.0, 24.0), None);
        assert_eq!(calculate_fit_view(&nodes, 800.0, -1.0, 24.0), None);
    }

    #[test]
    fn given_negative_padding_when_calculating_fit_view_then_result_is_none() {
        let nodes = [(10.0, 20.0), (40.0, 60.0)];

        assert_eq!(calculate_fit_view(&nodes, 800.0, 600.0, -10.0), None);
    }

    #[test]
    fn given_non_finite_nodes_when_calculating_fit_view_then_result_is_none() {
        let nodes = [(10.0, 20.0), (f32::NAN, 60.0)];

        assert_eq!(calculate_fit_view(&nodes, 800.0, 600.0, 24.0), None);
    }
}
