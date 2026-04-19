use crate::ui::edges::graph_types::{Connection, Node, NodeId, PortName, WorkflowNode};
use uuid::Uuid;

pub(crate) use crate::ui::edges::types::NODE_HEIGHT;

pub(crate) fn build_node(id: NodeId, x: f32, y: f32) -> Node {
    let mut node = Node::from_workflow_node(
        format!("Node {}", id),
        WorkflowNode::Run(crate::ui::edges::graph_types::RunConfig::default()),
        x,
        y,
    );
    node.id = id;
    node
}

pub(crate) fn build_parallel_node(id: NodeId, x: f32, y: f32) -> Node {
    let mut node = Node::from_workflow_node(
        format!("Parallel {}", id),
        WorkflowNode::Parallel(crate::ui::edges::graph_types::ParallelConfig::default()),
        x,
        y,
    );
    node.id = id;
    node
}

pub(crate) fn build_connection(id: Uuid, source: NodeId, target: NodeId) -> Connection {
    Connection {
        id,
        source,
        target,
        source_port: PortName::from("out"),
        target_port: PortName::from("in"),
    }
}

pub(crate) fn build_node_with_id(id: NodeId, x: f32, y: f32) -> Node {
    let mut node = build_node(id, x, y);
    node.id = id;
    node
}
