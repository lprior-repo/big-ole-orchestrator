use crate::ui::edges::graph_types::{NodeId};

#[derive(Debug, Clone, PartialEq)]
pub struct ParallelGroup {
    pub parallel_node_id: NodeId,
    pub branch_node_ids: Vec<NodeId>,
    pub bounding_box: BoundingBox,
    pub branch_count: usize,
    pub aggregate_status: AggregateStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateStatus {
    Pending,
    Running,
    Completed,
    PartialFailure,
    Failed,
}

impl AggregateStatus {
    pub fn stroke_color(self) -> &'static str {
        match self {
            Self::Pending => "rgba(148, 163, 184, 0.6)",
            Self::Running => "rgba(37, 99, 235, 0.7)",
            Self::Completed => "rgba(16, 185, 129, 0.6)",
            Self::PartialFailure => "rgba(245, 158, 11, 0.7)",
            Self::Failed => "rgba(244, 63, 94, 0.7)",
        }
    }

    pub fn badge_bg_color(self) -> &'static str {
        match self {
            Self::Pending => "#94a3b8",
            Self::Running => "#2563eb",
            Self::Completed => "#10b981",
            Self::PartialFailure => "#f59e0b",
            Self::Failed => "#f43f5e",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_status_stroke_colors_are_unique() {
        let colors = [
            AggregateStatus::Pending.stroke_color(),
            AggregateStatus::Running.stroke_color(),
            AggregateStatus::Completed.stroke_color(),
            AggregateStatus::PartialFailure.stroke_color(),
            AggregateStatus::Failed.stroke_color(),
        ];
        let unique: std::collections::HashSet<_> = colors.iter().copied().collect();
        assert_eq!(unique.len(), 5, "each status must have a unique stroke color");
    }

    #[test]
    fn aggregate_status_badge_colors_are_unique() {
        let colors = [
            AggregateStatus::Pending.badge_bg_color(),
            AggregateStatus::Running.badge_bg_color(),
            AggregateStatus::Completed.badge_bg_color(),
            AggregateStatus::PartialFailure.badge_bg_color(),
            AggregateStatus::Failed.badge_bg_color(),
        ];
        let unique: std::collections::HashSet<_> = colors.iter().copied().collect();
        assert_eq!(unique.len(), 5, "each status must have a unique badge color");
    }

    #[test]
    fn bounding_box_has_expected_fields() {
        let bb = BoundingBox {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 50.0,
        };
        assert_eq!(bb.x, 10.0);
        assert_eq!(bb.y, 20.0);
        assert_eq!(bb.width, 100.0);
        assert_eq!(bb.height, 50.0);
    }

    #[test]
    fn parallel_group_default_fields() {
        use crate::ui::edges::graph_types::NodeId;
        let group = ParallelGroup {
            parallel_node_id: NodeId::new(),
            branch_node_ids: vec![],
            bounding_box: BoundingBox {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
            branch_count: 0,
            aggregate_status: AggregateStatus::Pending,
        };
        assert_eq!(group.branch_count, 0);
        assert_eq!(group.aggregate_status, AggregateStatus::Pending);
        assert!(group.branch_node_ids.is_empty());
    }
}
