//! Core types for vel-bxpg DAG cycle detection.

use serde::{Deserialize, Serialize};

// ============================================================================
// Type Aliases
// ============================================================================

/// A node name identifier.
pub type NodeName = String;

// ============================================================================
// NodeHandle
// ============================================================================

/// A node handle that wraps a DAG node with input type `I` and output type `O`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeHandle<I, O> {
    name: NodeName,
    _input: std::marker::PhantomData<I>,
    _output: std::marker::PhantomData<O>,
}

impl<I, O> NodeHandle<I, O> {
    #[must_use]
    pub fn new(name: NodeName) -> Self {
        Self {
            name,
            _input: std::marker::PhantomData,
            _output: std::marker::PhantomData,
        }
    }

    #[must_use]
    pub fn name(&self) -> &NodeName {
        &self.name
    }
}

// ============================================================================
// DagNode & RetryPolicy
// ============================================================================

/// A single node in the DAG.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DagNode {
    pub name: NodeName,
    #[serde(default)]
    pub retry_policy: Option<RetryPolicy>,
}

/// Retry policy configuration for a node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub backoff_ms: u64,
}

// ============================================================================
// Edge
// ============================================================================

/// An edge connecting two nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub source_node: NodeName,
    pub target_node: NodeName,
    #[serde(default)]
    pub condition: Option<String>,
}

// ============================================================================
// WorkflowDefinition
// ============================================================================

/// The final serializable workflow definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub workflow_name: String,
    pub nodes: Vec<DagNode>,
    pub edges: Vec<Edge>,
}
