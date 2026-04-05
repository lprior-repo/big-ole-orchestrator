//! Error types for vo-executor

use crate::types::StepId;
use thiserror::Error;

/// Errors from step execution operations.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ExecuteNodeError {
    /// Step does not exist in the workflow.
    #[error("Step not found: {step_id}")]
    StepNotFound { step_id: StepId },

    /// Timeout value is invalid (must be > 0ms).
    #[error("Invalid timeout: {value} - {reason}")]
    InvalidTimeout { value: u64, reason: String },

    /// Timeout exceeded during execution.
    #[error("Timeout exceeded: {elapsed_ms}ms > {limit_ms}ms")]
    TimeoutExceeded { elapsed_ms: u64, limit_ms: u64 },

    /// Invalid state transition attempted.
    #[error("Invalid transition: {from_state} + {action}")]
    InvalidTransition { from_state: String, action: String },

    /// Retry attempts exhausted.
    #[error("Retry exhausted after {attempts} attempts: {last_error}")]
    RetryExhausted {
        attempts: u32,
        last_error: Box<ExecuteNodeError>,
    },

    /// Invalid retry policy configuration.
    #[error("Invalid retry policy on node {node_name}: {reason}")]
    InvalidRetryPolicy {
        node_name: String,
        reason: RetryPolicyError,
    },

    /// Execution was cancelled by user.
    #[error("Execution cancelled: {reason}")]
    ExecutionCancelled { reason: String },

    /// Transient error that may succeed on retry.
    #[error("Transient error: {reason} (recoverable={recoverable})")]
    TransientError { reason: String, recoverable: bool },
}

/// Errors for invalid retry policy configuration.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum RetryPolicyError {
    #[error("Zero attempts not allowed")]
    ZeroAttempts,
    #[error("Invalid multiplier: {got} (must be >= 1.0)")]
    InvalidMultiplier { got: f64 },
}
