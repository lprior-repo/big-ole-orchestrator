use oya_frontend::graph::{Connection, Node, NodeId, PortName, WorkflowNode};
use uuid::Uuid;

pub(crate) const NODE_HEIGHT: f32 = 68.0;

pub(crate) fn build_node(id: NodeId, x: f32, y: f32) -> Node {
    let mut node = Node::from_workflow_node(
        format!("Node {}", id),
        WorkflowNode::Run(oya_frontend::graph::workflow_node::RunConfig::default()),
        x,
        y,
    );
    node.id = id;
    node
}

pub(crate) fn build_parallel_node(id: NodeId, x: f32, y: f32) -> Node {
    let mut node = Node::from_workflow_node(
        format!("Parallel {}", id),
        WorkflowNode::Parallel(oya_frontend::graph::workflow_node::ParallelConfig::default()),
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
