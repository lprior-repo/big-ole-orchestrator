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
    /// `max_attempts` was zero.
    #[error("max_attempts must be >= 1, got 0")]
    ZeroAttempts,

    /// `backoff_multiplier` was less than 1.0 or non-finite.
    #[error("backoff_multiplier must be >= 1.0, got {got}")]
    InvalidMultiplier { got: f64 },

    /// `max_backoff_ms` was less than `backoff_ms`.
    #[error("max_backoff_ms ({max}) must be >= backoff_ms ({ms})")]
    MaxBackoffTooSmall { max: u64, ms: u64 },
}

// ---------------------------------------------------------------------------
// RetryPolicy
// ---------------------------------------------------------------------------

fn default_max_backoff_ms() -> u64 {
    u64::MAX
}

/// Per-node retry configuration with exponential backoff.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of execution attempts (minimum 1).
    pub max_attempts: u8,
    /// Initial backoff delay in milliseconds.
    pub backoff_ms: u64,
    /// Multiplier applied to backoff after each retry (minimum 1.0).
    pub backoff_multiplier: f64,
    /// Cap on backoff delay in milliseconds (prevents unbounded growth).
    /// Defaults to `u64::MAX` when deserializing from formats that omit it.
    #[serde(default = "default_max_backoff_ms")]
    pub max_backoff_ms: u64,
}

impl RetryPolicy {
    /// Construct a new `RetryPolicy` with validation.
    ///
    /// # Errors
    ///
    /// Returns `RetryPolicyError::ZeroAttempts` if `max_attempts` is 0,
    /// `RetryPolicyError::InvalidMultiplier` if `backoff_multiplier` < 1.0, NaN, or infinite,
    /// or `RetryPolicyError::MaxBackoffTooSmall` if `max_backoff_ms` < `backoff_ms`.
    pub fn new(
        max_attempts: u8,
        backoff_ms: u64,
        backoff_multiplier: f64,
    ) -> Result<Self, RetryPolicyError> {
        if max_attempts == 0 {
            return Err(RetryPolicyError::ZeroAttempts);
        }
        if !backoff_multiplier.is_finite() || backoff_multiplier < 1.0 {
            return Err(RetryPolicyError::InvalidMultiplier {
                got: backoff_multiplier,
            });
        }
        Ok(RetryPolicy {
            max_attempts,
            backoff_ms,
            backoff_multiplier,
            max_backoff_ms: u64::MAX,
        })
    }

    /// Construct with an explicit `max_backoff_ms` cap.
    ///
    /// # Errors
    ///
    /// Same as [`RetryPolicy::new`], plus
    /// `RetryPolicyError::MaxBackoffTooSmall` if `max_backoff_ms` < `backoff_ms`.
    pub fn with_max_backoff(
        max_attempts: u8,
        backoff_ms: u64,
        backoff_multiplier: f64,
        max_backoff_ms: u64,
    ) -> Result<Self, RetryPolicyError> {
        if max_attempts == 0 {
            return Err(RetryPolicyError::ZeroAttempts);
        }
        if !backoff_multiplier.is_finite() || backoff_multiplier < 1.0 {
            return Err(RetryPolicyError::InvalidMultiplier {
                got: backoff_multiplier,
            });
        }
        if max_backoff_ms < backoff_ms {
            return Err(RetryPolicyError::MaxBackoffTooSmall {
                max: max_backoff_ms,
                ms: backoff_ms,
            });
        }
        Ok(RetryPolicy {
            max_attempts,
            backoff_ms,
            backoff_multiplier,
            max_backoff_ms,
        })
    }

    /// Calculate the backoff delay for a given attempt (1-indexed).
    ///
    /// Formula: `min(backoff_ms * multiplier^(attempt - 1), max_backoff_ms)`
    ///
    /// Returns 0 for attempt 0. Returns `max_backoff_ms` if the calculation
    /// would exceed it or overflow `u64`.
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn calculate_backoff_delay(&self, attempt: u32) -> u64 {
        if attempt == 0 || self.backoff_ms == 0 {
            return 0;
        }
        let exponent = attempt.saturating_sub(1);
        let multiplier_pow = self.backoff_multiplier.powi(exponent as i32);
        #[allow(clippy::cast_precision_loss)]
        let product = self.backoff_ms as f64 * multiplier_pow;
        #[allow(clippy::cast_precision_loss)]
        let capped = product.min(self.max_backoff_ms as f64).min(u64::MAX as f64);
        capped as u64
    }
}

// ---------------------------------------------------------------------------
// DagNode
// ---------------------------------------------------------------------------

/// A single step in the workflow DAG.
/// Per ADR-009: `binary_path` is NOT stored here.
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
        let err = RetryPolicy::new(3, 100, f64::NAN).unwrap_err();
        assert!(matches!(err, RetryPolicyError::InvalidMultiplier { .. }));
        assert!(err
            .to_string()
            .contains("backoff_multiplier must be >= 1.0"));
    }

    #[test]
    fn retry_policy_new_returns_invalid_multiplier_error_when_multiplier_is_infinite() {
        let err = RetryPolicy::new(3, 100, f64::INFINITY).unwrap_err();
        assert!(matches!(err, RetryPolicyError::InvalidMultiplier { .. }));
    }

    #[test]
    fn retry_policy_new_sets_max_backoff_to_u64_max() {
        let policy = RetryPolicy::new(3, 100, 2.0).unwrap();
        assert_eq!(policy.max_backoff_ms, u64::MAX);
    }

    #[test]
    fn retry_policy_with_max_backoff_rejects_too_small_max() {
        let err = RetryPolicy::with_max_backoff(3, 500, 2.0, 100).unwrap_err();
        assert!(matches!(
            err,
            RetryPolicyError::MaxBackoffTooSmall { max: 100, ms: 500 }
        ));
        assert_eq!(
            err.to_string(),
            "max_backoff_ms (100) must be >= backoff_ms (500)"
        );
    }

    #[test]
    fn retry_policy_with_max_backoff_accepts_equal_values() {
        let policy = RetryPolicy::with_max_backoff(3, 100, 2.0, 100).unwrap();
        assert_eq!(policy.max_backoff_ms, 100);
    }

    #[test]
    fn calculate_backoff_delay_returns_zero_for_attempt_zero() {
        let policy = RetryPolicy::new(3, 100, 2.0).unwrap();
        assert_eq!(policy.calculate_backoff_delay(0), 0);
    }

    #[test]
    fn calculate_backoff_delay_returns_zero_when_backoff_ms_is_zero() {
        let policy = RetryPolicy::new(3, 0, 2.0).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 0);
        assert_eq!(policy.calculate_backoff_delay(5), 0);
    }

    #[test]
    fn calculate_backoff_delay_is_constant_when_multiplier_is_one() {
        let policy = RetryPolicy::new(5, 100, 1.0).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 100);
        assert_eq!(policy.calculate_backoff_delay(2), 100);
        assert_eq!(policy.calculate_backoff_delay(5), 100);
    }

    #[test]
    fn calculate_backoff_delay_grows_exponentially() {
        let policy = RetryPolicy::new(5, 100, 2.0).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 100);
        assert_eq!(policy.calculate_backoff_delay(2), 200);
        assert_eq!(policy.calculate_backoff_delay(3), 400);
        assert_eq!(policy.calculate_backoff_delay(4), 800);
    }

    #[test]
    fn calculate_backoff_delay_is_capped_by_max_backoff() {
        let policy = RetryPolicy::with_max_backoff(5, 100, 2.0, 300).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 100);
        assert_eq!(policy.calculate_backoff_delay(2), 200);
        assert_eq!(policy.calculate_backoff_delay(3), 300);
        assert_eq!(policy.calculate_backoff_delay(4), 300);
    }

    #[test]
    fn calculate_backoff_delay_does_not_overflow_u64() {
        let policy = RetryPolicy::new(255, u64::MAX, 2.0).unwrap();
        let delay = policy.calculate_backoff_delay(100);
        assert_eq!(delay, u64::MAX);
    }
}
