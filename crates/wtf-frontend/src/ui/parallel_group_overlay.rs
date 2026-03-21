use dioxus::prelude::*;
use oya_frontend::graph::{Connection, ExecutionState, Node, NodeId};
use oya_frontend::graph::workflow_node::WorkflowNode;
use std::collections::HashMap;
use std::str::FromStr;

const NODE_WIDTH: f32 = 220.0;
const NODE_HEIGHT: f32 = 68.0;
const PADDING_X: f32 = 24.0;
const PADDING_Y: f32 = 32.0;
const BADGE_OFFSET_X: f32 = 12.0;
const BADGE_OFFSET_Y: f32 = -12.0;

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

fn find_parallel_nodes(nodes: &[Node]) -> Vec<NodeId> {
    nodes
        .iter()
        .filter(|node| matches!(node.node, WorkflowNode::Parallel(_)))
        .map(|node| node.id)
        .collect()
}

fn find_branch_nodes(
    parallel_node_id: NodeId,
    connections: &[Connection],
    nodes: &[Node],
) -> Vec<NodeId> {
    let node_by_id: HashMap<_, _> = nodes.iter().map(|node| (node.id, node)).collect();

    let mut branch_node_ids: Vec<NodeId> = connections
        .iter()
        .filter(|conn| conn.source == parallel_node_id)
        .filter_map(|conn| {
            let target_node = node_by_id.get(&conn.target)?;
            let is_valid_branch = !matches!(target_node.node, WorkflowNode::Parallel(_));
            is_valid_branch.then_some(conn.target)
        })
        .collect();

    branch_node_ids.sort_by(|a, b| {
        let node_a = node_by_id.get(a);
        let node_b = node_by_id.get(b);
        match (node_a, node_b) {
            (Some(na), Some(nb)) => (na.x, na.y)
                .partial_cmp(&(nb.x, nb.y))
                .unwrap_or(std::cmp::Ordering::Equal),
            _ => std::cmp::Ordering::Equal,
        }
    });

    branch_node_ids
}

fn calculate_bounding_box(branch_nodes: &[Node]) -> Option<BoundingBox> {
    if branch_nodes.is_empty() {
        return None;
    }

    let min_x = branch_nodes
        .iter()
        .map(|n| n.x)
        .fold(f32::INFINITY, f32::min);
    let max_x = branch_nodes
        .iter()
        .map(|n| n.x + NODE_WIDTH)
        .fold(f32::NEG_INFINITY, f32::max);
    let min_y = branch_nodes
        .iter()
        .map(|n| n.y)
        .fold(f32::INFINITY, f32::min);
    let max_y = branch_nodes
        .iter()
        .map(|n| n.y + NODE_HEIGHT)
        .fold(f32::NEG_INFINITY, f32::max);

    if !min_x.is_finite() || !max_x.is_finite() || !min_y.is_finite() || !max_y.is_finite() {
        return None;
    }

    Some(BoundingBox {
        x: min_x - PADDING_X,
        y: min_y - PADDING_Y,
        width: (max_x - min_x) + PADDING_X * 2.0,
        height: (max_y - min_y) + PADDING_Y * 2.0,
    })
}

fn compute_aggregate_status(branch_nodes: &[Node]) -> AggregateStatus {
    if branch_nodes.is_empty() {
        return AggregateStatus::Pending;
    }

    let mut has_running = false;
    let mut has_succeeded = false;
    let mut has_failed = false;
    let mut has_pending = false;

    for node in branch_nodes {
        match node.execution_state {
            ExecutionState::Running | ExecutionState::Queued => has_running = true,
            ExecutionState::Completed => has_succeeded = true,
            ExecutionState::Failed => has_failed = true,
            ExecutionState::Idle | ExecutionState::Skipped => has_pending = true,
        }
    }

    if has_running {
        AggregateStatus::Running
    } else if has_failed && has_succeeded {
        AggregateStatus::PartialFailure
    } else if has_failed {
        AggregateStatus::Failed
    } else if has_succeeded && !has_pending {
        AggregateStatus::Completed
    } else {
        AggregateStatus::Pending
    }
}

fn detect_parallel_groups(nodes: &[Node], connections: &[Connection]) -> Vec<ParallelGroup> {
    let parallel_node_ids = find_parallel_nodes(nodes);
    let node_by_id: HashMap<NodeId, &Node> = nodes.iter().map(|node| (node.id, node)).collect();

    parallel_node_ids
        .into_iter()
        .filter_map(|parallel_id| {
            let branch_ids = find_branch_nodes(parallel_id, connections, nodes);
            let branch_nodes: Vec<Node> = branch_ids
                .iter()
                .filter_map(|id| node_by_id.get(id).map(|n| (*n).clone()))
                .collect();

            let bounding_box = calculate_bounding_box(&branch_nodes)?;
            let aggregate_status = compute_aggregate_status(&branch_nodes);

            Some(ParallelGroup {
                parallel_node_id: parallel_id,
                branch_node_ids: branch_ids,
                bounding_box,
                branch_count: branch_nodes.len(),
                aggregate_status,
            })
        })
        .collect()
}

#[component]
pub fn ParallelGroupOverlay(
    nodes: ReadSignal<Vec<Node>>,
    connections: ReadSignal<Vec<Connection>>,
) -> Element {
    let parallel_groups = use_memo(move || {
        let node_list = nodes.read();
        let conn_list = connections.read();
        detect_parallel_groups(&node_list, &conn_list)
    });

    rsx! {
        svg {
            class: "absolute inset-0 overflow-visible pointer-events-none",
            style: "width: 100%; height: 100%; z-index: -1;",
            for group in parallel_groups.read().iter() {
                {
                    let bb = group.bounding_box;
                    let stroke_color = group.aggregate_status.stroke_color();
                    let badge_bg = group.aggregate_status.badge_bg_color();
                    let badge_x = bb.x + BADGE_OFFSET_X;
                    let badge_y = bb.y + BADGE_OFFSET_Y;
                    let branch_text = if group.branch_count == 1 {
                        "1 branch".to_string()
                    } else {
                        format!("{} branches", group.branch_count)
                    };

                    rsx! {
                        g {
                            key: "{group.parallel_node_id}",
                            rect {
                                x: "{bb.x}",
                                y: "{bb.y}",
                                width: "{bb.width}",
                                height: "{bb.height}",
                                rx: "8",
                                ry: "8",
                                fill: "rgba(148, 163, 184, 0.05)",
                                stroke: "{stroke_color}",
                                stroke_width: "2",
                                stroke_dasharray: "8 4",
                            }
                            g {
                                transform: "translate({badge_x}, {badge_y})",
                                rect {
                                    x: "0",
                                    y: "-12",
                                    width: "70",
                                    height: "20",
                                    rx: "10",
                                    ry: "10",
                                    fill: "{badge_bg}",
                                }
                                text {
                                    x: "35",
                                    y: "2",
                                    text_anchor: "middle",
                                    font_size: "10",
                                    font_weight: "600",
                                    fill: "white",
                                    font_family: "Geist, Inter, sans-serif",
                                    "{branch_text}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oya_frontend::graph::workflow_node::WorkflowNode;
    use uuid::Uuid;

    fn make_node(id: Uuid, node_type: &str, x: f32, y: f32) -> Node {
        let wfn = WorkflowNode::from_str(node_type)
            .unwrap_or_else(|_| WorkflowNode::Run(oya_frontend::graph::workflow_node::RunConfig::default()));
        let mut node = Node::from_workflow_node(format!("{node_type} node"), wfn, x, y);
        node.id = NodeId(id);
        node
    }

    fn make_connection(source: Uuid, target: Uuid) -> Connection {
        Connection {
            id: Uuid::new_v4(),
            source: NodeId(source),
            target: NodeId(target),
            source_port: oya_frontend::graph::PortName("main".to_string()),
            target_port: oya_frontend::graph::PortName("main".to_string()),
        }
    }

    #[test]
    fn given_no_parallel_nodes_when_detecting_then_returns_empty() {
        let nodes = vec![
            make_node(Uuid::nil(), "service-call", 0.0, 0.0),
            make_node(Uuid::new_v4(), "run", 100.0, 100.0),
        ];
        let connections = vec![];

        let groups = detect_parallel_groups(&nodes, &connections);

        assert!(groups.is_empty());
    }

    #[test]
    fn given_parallel_node_with_no_branches_when_detecting_then_returns_empty() {
        let parallel_id = Uuid::nil();
        let nodes = vec![make_node(parallel_id, "parallel", 0.0, 0.0)];
        let connections = vec![];

        let groups = detect_parallel_groups(&nodes, &connections);

        assert!(groups.is_empty());
    }

    #[test]
    fn given_parallel_node_with_branches_when_detecting_then_returns_group() {
        let parallel_id = Uuid::nil();
        let branch1_id = Uuid::new_v4();
        let branch2_id = Uuid::new_v4();

        let nodes = vec![
            make_node(parallel_id, "parallel", 0.0, 0.0),
            make_node(branch1_id, "service-call", 100.0, 100.0),
            make_node(branch2_id, "run", 350.0, 100.0),
        ];
        let connections = vec![
            make_connection(parallel_id, branch1_id),
            make_connection(parallel_id, branch2_id),
        ];

        let groups = detect_parallel_groups(&nodes, &connections);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].branch_count, 2);
        assert_eq!(groups[0].parallel_node_id, NodeId(parallel_id));
    }

    #[test]
    fn given_all_idle_branches_when_computing_status_then_returns_pending() {
        let nodes = vec![
            make_node(Uuid::nil(), "service-call", 0.0, 0.0),
            make_node(Uuid::new_v4(), "run", 100.0, 100.0),
        ];

        let status = compute_aggregate_status(&nodes);

        assert_eq!(status, AggregateStatus::Pending);
    }

    #[test]
    fn given_all_succeeded_branches_when_computing_status_then_returns_completed() {
        let mut nodes = vec![
            make_node(Uuid::nil(), "service-call", 0.0, 0.0),
            make_node(Uuid::new_v4(), "run", 100.0, 100.0),
        ];
        for node in &mut nodes {
            node.execution_state = ExecutionState::Completed;
        }

        let status = compute_aggregate_status(&nodes);

        assert_eq!(status, AggregateStatus::Completed);
    }

    #[test]
    fn given_running_branch_when_computing_status_then_returns_running() {
        let mut nodes = vec![make_node(Uuid::nil(), "service-call", 0.0, 0.0)];
        nodes[0].execution_state = ExecutionState::Running;

        let status = compute_aggregate_status(&nodes);

        assert_eq!(status, AggregateStatus::Running);
    }

    #[test]
    fn given_waiting_branch_when_computing_status_then_returns_running() {
        let mut nodes = vec![make_node(Uuid::nil(), "service-call", 0.0, 0.0)];
        nodes[0].execution_state = ExecutionState::Queued;

        let status = compute_aggregate_status(&nodes);

        assert_eq!(status, AggregateStatus::Running);
    }

    #[test]
    fn given_mixed_succeeded_and_failed_when_computing_status_then_returns_partial_failure() {
        let mut node1 = make_node(Uuid::nil(), "service-call", 0.0, 0.0);
        node1.execution_state = ExecutionState::Completed;
        let mut node2 = make_node(Uuid::new_v4(), "run", 100.0, 100.0);
        node2.execution_state = ExecutionState::Failed;

        let nodes = vec![node1, node2];
        let status = compute_aggregate_status(&nodes);

        assert_eq!(status, AggregateStatus::PartialFailure);
    }

    #[test]
    fn given_all_failed_when_computing_status_then_returns_failed() {
        let mut nodes = vec![make_node(Uuid::nil(), "service-call", 0.0, 0.0)];
        nodes[0].execution_state = ExecutionState::Failed;

        let status = compute_aggregate_status(&nodes);

        assert_eq!(status, AggregateStatus::Failed);
    }

    #[test]
    fn given_branches_when_calculating_bbox_then_includes_padding() {
        let nodes = vec![
            make_node(Uuid::nil(), "service-call", 100.0, 100.0),
            make_node(Uuid::new_v4(), "run", 350.0, 100.0),
        ];

        let bbox = calculate_bounding_box(&nodes);

        assert!(bbox.is_some());
        let bbox = bbox.unwrap();
        assert_eq!(bbox.x, 100.0 - PADDING_X);
        assert_eq!(bbox.y, 100.0 - PADDING_Y);
        assert!(bbox.width > NODE_WIDTH);
        assert!(bbox.height > NODE_HEIGHT);
    }

    #[test]
    fn given_empty_nodes_when_calculating_bbox_then_returns_none() {
        let nodes: Vec<Node> = vec![];

        let bbox = calculate_bounding_box(&nodes);

        assert!(bbox.is_none());
    }
}
