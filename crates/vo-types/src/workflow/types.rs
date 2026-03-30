use serde::{Deserialize, Serialize};

use crate::NodeName;

// ---------------------------------------------------------------------------
// StepOutcome
// ---------------------------------------------------------------------------

/// Outcome of executing a single DAG node.
/// Defined locally in vo-types to avoid circular deps with vo-icg.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StepOutcome {
    Success,
    Failure,
}

// ---------------------------------------------------------------------------
// EdgeCondition
// ---------------------------------------------------------------------------

/// Condition on which an edge is traversed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeCondition {
    /// Always traverse this edge, regardless of step outcome.
    Always,
    /// Traverse only if the source node succeeded.
    OnSuccess,
    /// Traverse only if the source node failed.
    OnFailure,
}

// ---------------------------------------------------------------------------
// RetryPolicyError
// ---------------------------------------------------------------------------

/// Errors returned by `RetryPolicy::new`.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RetryPolicyError {
    /// max_attempts was zero.
    #[error("max_attempts must be >= 1, got 0")]
    ZeroAttempts,

    /// backoff_multiplier was less than 1.0.
    #[error("backoff_multiplier must be >= 1.0, got {got}")]
    InvalidMultiplier { got: f32 },
}

// ---------------------------------------------------------------------------
// RetryPolicy
// ---------------------------------------------------------------------------

/// Per-node retry configuration with exponential backoff.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of execution attempts (minimum 1).
    pub max_attempts: u8,
    /// Initial backoff delay in milliseconds.
    pub backoff_ms: u64,
    /// Multiplier applied to backoff after each retry (minimum 1.0).
    pub backoff_multiplier: f32,
}

impl RetryPolicy {
    /// Construct a new RetryPolicy with validation.
    pub fn new(
        max_attempts: u8,
        backoff_ms: u64,
        backoff_multiplier: f32,
    ) -> Result<Self, RetryPolicyError> {
        if max_attempts == 0 {
            return Err(RetryPolicyError::ZeroAttempts);
        }
        if backoff_multiplier < 1.0 || backoff_multiplier.is_nan() {
            return Err(RetryPolicyError::InvalidMultiplier {
                got: backoff_multiplier,
            });
        }
        Ok(RetryPolicy {
            max_attempts,
            backoff_ms,
            backoff_multiplier,
        })
    }
}

// ---------------------------------------------------------------------------
// DagNode
// ---------------------------------------------------------------------------

/// A single step in the workflow DAG.
/// Per ADR-009: binary_path is NOT stored here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DagNode {
    pub node_name: NodeName,
    pub retry_policy: RetryPolicy,
}

// ---------------------------------------------------------------------------
// Edge
// ---------------------------------------------------------------------------

/// A directed edge from one node to another with a traversal condition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub source_node: NodeName,
    pub target_node: NodeName,
    pub condition: EdgeCondition,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_policy_new_returns_zero_attempts_error_when_max_attempts_is_zero() {
        let err = RetryPolicy::new(0, 100, 2.0).unwrap_err();
        assert!(matches!(err, RetryPolicyError::ZeroAttempts));
        assert_eq!(err.to_string(), "max_attempts must be >= 1, got 0");
    }

    #[test]
    fn retry_policy_new_returns_invalid_multiplier_error_when_multiplier_is_less_than_one() {
        let err = RetryPolicy::new(3, 100, 0.5).unwrap_err();
        assert!(matches!(err, RetryPolicyError::InvalidMultiplier { got } if got == 0.5));
        assert_eq!(
            err.to_string(),
            "backoff_multiplier must be >= 1.0, got 0.5"
        );
    }

    #[test]
    fn retry_policy_new_returns_invalid_multiplier_error_when_multiplier_is_nan() {
        let err = RetryPolicy::new(3, 100, f32::NAN).unwrap_err();
        assert!(matches!(err, RetryPolicyError::InvalidMultiplier { .. }));
        assert!(err
            .to_string()
            .contains("backoff_multiplier must be >= 1.0"));
    }
}
