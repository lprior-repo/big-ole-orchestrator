//! Error types for vel-bxpg DAG cycle detection.

use thiserror::Error;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur when building a `WorkflowDefinition`.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum WorkflowDefinitionError {
    /// JSON could not be deserialized into the intermediate unvalidated struct.
    #[error("Deserialization failed: {message}")]
    DeserializationFailed { message: String },

    /// The nodes list is empty.
    #[error("Workflow has no nodes")]
    EmptyWorkflow,

    /// The graph contains a cycle.
    #[error("Cycle detected: {cycle_nodes:?}")]
    CycleDetected { cycle_nodes: Vec<String> },

    /// An edge references a node name that does not exist in the nodes list.
    #[error("Unknown node: {edge_source} references unknown node {unknown_target}")]
    UnknownNode {
        edge_source: String,
        unknown_target: String,
    },

    /// A `DagNode` contains an invalid `RetryPolicy`.
    #[error("Invalid retry policy on node {node_name}: {reason}")]
    InvalidRetryPolicy {
        node_name: String,
        reason: RetryPolicyError,
    },
}

/// Errors specific to `--graph` output integration.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GraphOutputError {
    /// Cycle was detected in the DAG before serialization.
    #[error("Cycle detected: {cycle_nodes:?}")]
    CycleDetected { cycle_nodes: Vec<String> },

    /// Failed to serialize `WorkflowDefinition` to JSON.
    #[error("Serialization failed")]
    SerializationFailed,

    /// stdout is not available for writing.
    #[error("stdout unavailable")]
    StdoutUnavailable,
}

/// Error reasons for invalid retry policies.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RetryPolicyError {
    #[error("Negative backoff interval: {0}")]
    NegativeBackoff(i64),
    #[error("Max retries exceeds limit: {0}")]
    MaxRetriesExceeded(u32),
}
