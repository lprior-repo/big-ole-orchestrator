use serde::{Deserialize, Serialize};
use uuid::Uuid;


// Re-export canonical types from crate::ui::graph (ADR-031).
pub use crate::ui::graph::{ExecutionState, PortName};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub Uuid);

impl NodeId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}


#[derive(Debug, Clone, PartialEq)]
pub struct Connection {
    pub id: Uuid,
    pub source: NodeId,
    pub target: NodeId,
    pub source_port: PortName,
    pub target_port: PortName,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct RunConfig {}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ParallelConfig {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WorkflowNode {
    Run(RunConfig),
    Parallel(ParallelConfig),
}

impl WorkflowNode {
    pub fn is_parallel(&self) -> bool {
        matches!(self, WorkflowNode::Parallel(_))
    }
}

impl std::str::FromStr for WorkflowNode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "run" => Ok(WorkflowNode::Run(RunConfig::default())),
            "parallel" => Ok(WorkflowNode::Parallel(ParallelConfig::default())),
            "service-call" => Ok(WorkflowNode::Run(RunConfig::default())),
            _ => Err(format!("unknown workflow node type: {s}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub node: WorkflowNode,
    pub execution_state: ExecutionState,
}

impl Node {
    #[must_use]
    pub fn from_workflow_node(name: String, node: WorkflowNode, x: f32, y: f32) -> Self {
        Self {
            id: NodeId::new(),
            name,
            x,
            y,
            node,
            execution_state: ExecutionState::Idle,
        }
    }
}
