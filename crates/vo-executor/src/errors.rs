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
    #[error("max_backoff_ms ({max}) must be >= backoff_ms ({ms})")]
    MaxBackoffTooSmall { max: u64, ms: u64 },
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn execute_node_error_step_not_found_display() {
        let err = ExecuteNodeError::StepNotFound {
            step_id: StepId::new("missing".to_string()),
        };
        let msg = err.to_string();
        assert!(msg.contains("missing"));
        assert!(msg.contains("Step not found"));
    }

    #[test]
    fn execute_node_error_invalid_timeout_display() {
        let err = ExecuteNodeError::InvalidTimeout {
            value: 0,
            reason: "must be > 0ms".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("0"));
        assert!(msg.contains("must be > 0ms"));
    }

    #[test]
    fn execute_node_error_timeout_exceeded_display() {
        let err = ExecuteNodeError::TimeoutExceeded {
            elapsed_ms: 5000,
            limit_ms: 3000,
        };
        let msg = err.to_string();
        assert!(msg.contains("5000ms"));
        assert!(msg.contains("3000ms"));
    }

    #[test]
    fn execute_node_error_invalid_transition_display() {
        let err = ExecuteNodeError::InvalidTransition {
            from_state: "Executing".to_string(),
            action: "execute_step".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("Executing"));
        assert!(msg.contains("execute_step"));
    }

    #[test]
    fn execute_node_error_retry_exhausted_display() {
        let inner = ExecuteNodeError::TransientError {
            reason: "conn reset".to_string(),
            recoverable: true,
        };
        let err = ExecuteNodeError::RetryExhausted {
            attempts: 5,
            last_error: Box::new(inner),
        };
        let msg = err.to_string();
        assert!(msg.contains("5"));
        assert!(msg.contains("conn reset"));
    }

    #[test]
    fn execute_node_error_invalid_retry_policy_display() {
        let err = ExecuteNodeError::InvalidRetryPolicy {
            node_name: "step-a".to_string(),
            reason: RetryPolicyError::ZeroAttempts,
        };
        let msg = err.to_string();
        assert!(msg.contains("step-a"));
        assert!(msg.contains("Zero attempts"));
    }

    #[test]
    fn execute_node_error_execution_cancelled_display() {
        let err = ExecuteNodeError::ExecutionCancelled {
            reason: "user request".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("user request"));
    }

    #[test]
    fn execute_node_error_transient_error_display() {
        let err = ExecuteNodeError::TransientError {
            reason: "network timeout".to_string(),
            recoverable: true,
        };
        let msg = err.to_string();
        assert!(msg.contains("network timeout"));
        assert!(msg.contains("recoverable=true"));
    }

    #[test]
    fn retry_policy_error_zero_attempts_display() {
        let err = RetryPolicyError::ZeroAttempts;
        assert_eq!(err.to_string(), "Zero attempts not allowed");
    }

    #[test]
    fn retry_policy_error_invalid_multiplier_display() {
        let err = RetryPolicyError::InvalidMultiplier { got: 0.5 };
        let msg = err.to_string();
        assert!(msg.contains("0.5"));
        assert!(msg.contains(">= 1.0"));
    }

    #[test]
    fn retry_policy_error_max_backoff_too_small_display() {
        let err = RetryPolicyError::MaxBackoffTooSmall { max: 50, ms: 100 };
        let msg = err.to_string();
        assert!(msg.contains("50"));
        assert!(msg.contains("100"));
    }

    #[test]
    fn error_equality_and_clone() {
        let err1 = ExecuteNodeError::StepNotFound {
            step_id: StepId::new("x".to_string()),
        };
        let err2 = err1.clone();
        assert_eq!(err1, err2);
    }

    #[test]
    fn retry_policy_error_equality() {
        let e1 = RetryPolicyError::ZeroAttempts;
        let e2 = RetryPolicyError::ZeroAttempts;
        assert_eq!(e1, e2);

        let e3 = RetryPolicyError::InvalidMultiplier { got: 1.5 };
        let e4 = RetryPolicyError::InvalidMultiplier { got: 1.5 };
        assert_eq!(e3, e4);

        let e5 = RetryPolicyError::InvalidMultiplier { got: 2.0 };
        assert_ne!(e3, e5);
    }

    #[test]
    fn all_execute_node_error_variants_construct() {
        let _ = ExecuteNodeError::StepNotFound {
            step_id: StepId::new("a".to_string()),
        };
        let _ = ExecuteNodeError::InvalidTimeout {
            value: 0,
            reason: "r".to_string(),
        };
        let _ = ExecuteNodeError::TimeoutExceeded {
            elapsed_ms: 1,
            limit_ms: 2,
        };
        let _ = ExecuteNodeError::InvalidTransition {
            from_state: "s".to_string(),
            action: "a".to_string(),
        };
        let _ = ExecuteNodeError::RetryExhausted {
            attempts: 1,
            last_error: Box::new(ExecuteNodeError::ExecutionCancelled {
                reason: "r".to_string(),
            }),
        };
        let _ = ExecuteNodeError::InvalidRetryPolicy {
            node_name: "n".to_string(),
            reason: RetryPolicyError::ZeroAttempts,
        };
        let _ = ExecuteNodeError::ExecutionCancelled {
            reason: "r".to_string(),
        };
        let _ = ExecuteNodeError::TransientError {
            reason: "r".to_string(),
            recoverable: false,
        };
    }
}
