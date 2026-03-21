use super::layout::DagLayout;
use super::Workflow;
use crate::graph::calc;

impl Workflow {
    pub fn apply_layout(&mut self) {
        let layout = DagLayout::default();
        layout.apply(self);
    }

    pub fn zoom(&mut self, delta: f32, cx: f32, cy: f32) {
        let old_zoom = self.viewport.zoom;
        let new_zoom = calc::calculate_zoom_delta(delta, old_zoom);
        let (new_x, new_y) = calc::calculate_pan_offset(
            self.viewport.x,
            self.viewport.y,
            cx,
            cy,
            old_zoom,
            new_zoom,
        );
        self.viewport.x = new_x;
        self.viewport.y = new_y;
        self.viewport.zoom = new_zoom;
    }

    pub fn fit_view(&mut self, viewport_width: f32, viewport_height: f32, padding: f32) {
        let node_positions: Vec<(f32, f32)> = self.nodes.iter().map(|n| (n.x, n.y)).collect();

        if let Some((viewport_x, viewport_y, zoom)) =
            calc::calculate_fit_view(&node_positions, viewport_width, viewport_height, padding)
        {
            self.viewport.x = viewport_x;
            self.viewport.y = viewport_y;
            self.viewport.zoom = zoom;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::graph::Workflow;

    #[test]
    fn given_zoom_delta_when_zooming_then_viewport_values_change() {
        let mut workflow = Workflow::new();
        let before = workflow.viewport.clone();

        workflow.zoom(0.8, 100.0, 80.0);

        assert_ne!(workflow.viewport.zoom, before.zoom);
        assert_ne!(workflow.viewport.x, before.x);
        assert_ne!(workflow.viewport.y, before.y);
    }

    #[test]
    fn given_nodes_when_fitting_view_then_zoom_updates_from_default() {
        let mut workflow = Workflow::new();
        let _ = workflow.add_node("start", 0.0, 0.0);
        let _ = workflow.add_node("next", 300.0, 200.0);

        workflow.fit_view(1200.0, 800.0, 48.0);

        assert!(workflow.viewport.zoom > 0.0);
        assert_ne!(workflow.viewport.zoom, 1.0);
    }

    #[test]
    fn given_empty_workflow_when_fitting_view_then_viewport_stays_unchanged() {
        let mut workflow = Workflow::new();
        let before = workflow.viewport.clone();

        workflow.fit_view(1200.0, 800.0, 48.0);

        assert_eq!(workflow.viewport.x, before.x);
        assert_eq!(workflow.viewport.y, before.y);
        assert_eq!(workflow.viewport.zoom, before.zoom);
    }
}
