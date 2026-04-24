//! Domain types for vo-executor

use crate::errors::{ExecuteNodeError, RetryPolicyError};
use serde::{Deserialize, Serialize};

/// A validated step identifier.
///
/// Valid step IDs must be non-empty strings containing only alphanumeric characters,
/// hyphens, and underscores.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StepId(String);

impl StepId {
    #[must_use]
    pub fn new(s: String) -> Self {
        Self(s)
    }

    /// Parse a string into a `StepId`.
    ///
    /// # Errors
    ///
    /// Returns [`ExecuteNodeError::StepNotFound`] if the string is empty
    /// or contains invalid characters (only alphanumeric, hyphens, and underscores allowed).
    pub fn parse(s: &str) -> Result<Self, ExecuteNodeError> {
        if s.is_empty() {
            return Err(ExecuteNodeError::StepNotFound {
                step_id: StepId(s.to_string()),
            });
        }
        if !s
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(ExecuteNodeError::StepNotFound {
                step_id: StepId(s.to_string()),
            });
        }
        Ok(Self(s.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for StepId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<StepId> for String {
    fn from(id: StepId) -> Self {
        id.0
    }
}

impl std::fmt::Display for StepId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Result of a workflow step execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepResult {
    /// Step completed successfully with output.
    Success { output: String },
    /// Step completed with failure (non-zero exit code or error).
    Failure { output: String },
}

impl StepResult {
    /// Check if the step result indicates success.
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(self, StepResult::Success { .. })
    }
}

/// Retry policy configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub backoff_ms: u64,
    pub backoff_multiplier: f64,
    pub max_backoff_ms: u64,
}

impl RetryPolicy {
    /// Create a new `RetryPolicy` with `max_backoff_ms` defaulting to `u64::MAX`.
    ///
    /// # Errors
    ///
    /// Returns [`RetryPolicyError::ZeroAttempts`] if `max_attempts` is 0.
    /// Returns [`RetryPolicyError::InvalidMultiplier`] if `multiplier` is NaN,
    /// infinite, or less than 1.0.
    pub fn new(
        max_attempts: u32,
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
        Ok(Self {
            max_attempts,
            backoff_ms,
            backoff_multiplier,
            max_backoff_ms: u64::MAX,
        })
    }

    /// Create a new `RetryPolicy` with an explicit `max_backoff_ms` cap.
    ///
    /// # Errors
    ///
    /// Returns [`RetryPolicyError::ZeroAttempts`] if `max_attempts` is 0.
    /// Returns [`RetryPolicyError::InvalidMultiplier`] if `multiplier` is NaN,
    /// infinite, or less than 1.0.
    /// Returns [`RetryPolicyError::MaxBackoffTooSmall`] if `max_backoff_ms` < `backoff_ms`.
    pub fn with_max_backoff(
        max_attempts: u32,
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
        Ok(Self {
            max_attempts,
            backoff_ms,
            backoff_multiplier,
            max_backoff_ms,
        })
    }

    /// Calculate the backoff delay for a given attempt (1-indexed).
    ///
    /// Formula: `min(backoff_ms * multiplier^(attempt - 1), max_backoff_ms)`
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

/// Execution status for a step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionStatus {
    Ready,
    Executing { step_id: StepId, elapsed_ms: u64 },
    Completed { output: String },
    Cancelled { reason: String },
}

impl ExecutionStatus {
    /// Check if the status indicates ready state.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        matches!(self, ExecutionStatus::Ready)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn step_id_new_and_as_str() {
        let id = StepId::new("hello".to_string());
        assert_eq!(id.as_str(), "hello");
    }

    #[test]
    fn step_id_parse_valid() {
        let cases = [
            "step-1",
            "step_2",
            "abc123",
            "a-b_c-d",
            "UPPER",
            "lower",
            "x",
            "workflow-step-1",
        ];
        for case in cases {
            let result = StepId::parse(case);
            assert!(result.is_ok(), "expected Ok for {:?}", case);
            assert_eq!(result.unwrap().as_str(), case);
        }
    }

    #[test]
    fn step_id_parse_empty_rejects() {
        let result = StepId::parse("");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ExecuteNodeError::StepNotFound { .. }
        ));
    }

    #[test]
    fn step_id_parse_invalid_chars_rejects() {
        let cases = ["has space", "dot.value", "has/slash", "a@b", "a!b", "a\tb"];
        for case in cases {
            let result = StepId::parse(case);
            assert!(result.is_err(), "expected Err for {:?}", case);
        }
    }

    #[test]
    fn step_id_display() {
        let id = StepId::new("my-step".to_string());
        assert_eq!(format!("{}", id), "my-step");
    }

    #[test]
    fn step_id_into_string() {
        let id = StepId::new("conv".to_string());
        let s: String = id.into();
        assert_eq!(s, "conv");
    }

    #[test]
    fn step_id_as_ref_str() {
        let id = StepId::new("ref".to_string());
        assert_eq!(id.as_ref(), "ref");
    }

    #[test]
    fn step_id_equality() {
        let a = StepId::new("same".to_string());
        let b = StepId::new("same".to_string());
        let c = StepId::new("diff".to_string());
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn step_id_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(StepId::new("x".to_string()));
        set.insert(StepId::new("x".to_string()));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn step_result_success_is_success() {
        let r = StepResult::Success {
            output: "ok".to_string(),
        };
        assert!(r.is_success());
    }

    #[test]
    fn step_result_failure_is_not_success() {
        let r = StepResult::Failure {
            output: "err".to_string(),
        };
        assert!(!r.is_success());
    }

    #[test]
    fn step_result_equality() {
        let a = StepResult::Success {
            output: "ok".to_string(),
        };
        let b = StepResult::Success {
            output: "ok".to_string(),
        };
        let c = StepResult::Failure {
            output: "err".to_string(),
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn step_result_serde_roundtrip() {
        let r = StepResult::Success {
            output: "data".to_string(),
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: StepResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn retry_policy_new_valid() {
        let p = RetryPolicy::new(3, 100, 2.0).unwrap();
        assert_eq!(p.max_attempts, 3);
        assert_eq!(p.backoff_ms, 100);
        assert!((p.backoff_multiplier - 2.0).abs() < f64::EPSILON);
        assert_eq!(p.max_backoff_ms, u64::MAX);
    }

    #[test]
    fn retry_policy_new_zero_attempts_rejects() {
        let result = RetryPolicy::new(0, 100, 2.0);
        assert_eq!(result.unwrap_err(), RetryPolicyError::ZeroAttempts);
    }

    #[test]
    fn retry_policy_new_nan_multiplier_rejects() {
        let result = RetryPolicy::new(3, 100, f64::NAN);
        assert!(matches!(
            result.unwrap_err(),
            RetryPolicyError::InvalidMultiplier { .. }
        ));
    }

    #[test]
    fn retry_policy_new_infinity_multiplier_rejects() {
        let result = RetryPolicy::new(3, 100, f64::INFINITY);
        assert!(matches!(
            result.unwrap_err(),
            RetryPolicyError::InvalidMultiplier { .. }
        ));
    }

    #[test]
    fn retry_policy_new_neg_infinity_multiplier_rejects() {
        let result = RetryPolicy::new(3, 100, f64::NEG_INFINITY);
        assert!(matches!(
            result.unwrap_err(),
            RetryPolicyError::InvalidMultiplier { .. }
        ));
    }

    #[test]
    fn retry_policy_new_multiplier_below_one_rejects() {
        let result = RetryPolicy::new(3, 100, 0.99);
        assert!(matches!(
            result.unwrap_err(),
            RetryPolicyError::InvalidMultiplier { .. }
        ));
    }

    #[test]
    fn retry_policy_new_multiplier_exactly_one_ok() {
        let result = RetryPolicy::new(3, 100, 1.0);
        assert!(result.is_ok());
    }

    #[test]
    fn retry_policy_with_max_backoff_valid() {
        let p = RetryPolicy::with_max_backoff(5, 100, 2.0, 1000).unwrap();
        assert_eq!(p.max_backoff_ms, 1000);
    }

    #[test]
    fn retry_policy_with_max_backoff_equal_ok() {
        let p = RetryPolicy::with_max_backoff(3, 100, 2.0, 100).unwrap();
        assert_eq!(p.max_backoff_ms, 100);
    }

    #[test]
    fn retry_policy_with_max_backoff_too_small_rejects() {
        let result = RetryPolicy::with_max_backoff(3, 100, 2.0, 50);
        assert!(matches!(
            result.unwrap_err(),
            RetryPolicyError::MaxBackoffTooSmall { max: 50, ms: 100 }
        ));
    }

    #[test]
    fn retry_policy_with_max_backoff_zero_attempts_rejects() {
        let result = RetryPolicy::with_max_backoff(0, 100, 2.0, 1000);
        assert_eq!(result.unwrap_err(), RetryPolicyError::ZeroAttempts);
    }

    #[test]
    fn retry_policy_with_max_backoff_invalid_multiplier_rejects() {
        let result = RetryPolicy::with_max_backoff(3, 100, 0.5, 1000);
        assert!(matches!(
            result.unwrap_err(),
            RetryPolicyError::InvalidMultiplier { .. }
        ));
    }

    #[test]
    fn calculate_backoff_delay_attempt_zero() {
        let p = RetryPolicy::new(3, 100, 2.0).unwrap();
        assert_eq!(p.calculate_backoff_delay(0), 0);
    }

    #[test]
    fn calculate_backoff_delay_zero_backoff() {
        let p = RetryPolicy::new(3, 0, 2.0).unwrap();
        assert_eq!(p.calculate_backoff_delay(1), 0);
        assert_eq!(p.calculate_backoff_delay(5), 0);
    }

    #[test]
    fn calculate_backoff_delay_linear_multiplier() {
        let p = RetryPolicy::new(10, 100, 1.0).unwrap();
        assert_eq!(p.calculate_backoff_delay(1), 100);
        assert_eq!(p.calculate_backoff_delay(5), 100);
        assert_eq!(p.calculate_backoff_delay(10), 100);
    }

    #[test]
    fn calculate_backoff_delay_exponential() {
        let p = RetryPolicy::new(10, 100, 2.0).unwrap();
        assert_eq!(p.calculate_backoff_delay(1), 100);
        assert_eq!(p.calculate_backoff_delay(2), 200);
        assert_eq!(p.calculate_backoff_delay(3), 400);
        assert_eq!(p.calculate_backoff_delay(4), 800);
        assert_eq!(p.calculate_backoff_delay(5), 1600);
    }

    #[test]
    fn calculate_backoff_delay_capped() {
        let p = RetryPolicy::with_max_backoff(10, 100, 10.0, 500).unwrap();
        assert_eq!(p.calculate_backoff_delay(1), 100);
        assert_eq!(p.calculate_backoff_delay(2), 500);
        assert_eq!(p.calculate_backoff_delay(3), 500);
    }

    #[test]
    fn execution_status_is_ready() {
        assert!(ExecutionStatus::Ready.is_ready());
        assert!(!ExecutionStatus::Executing {
            step_id: StepId::new("x".to_string()),
            elapsed_ms: 0
        }
        .is_ready());
        assert!(!ExecutionStatus::Completed {
            output: "o".to_string()
        }
        .is_ready());
        assert!(!ExecutionStatus::Cancelled {
            reason: "r".to_string()
        }
        .is_ready());
    }

    #[test]
    fn execution_status_equality() {
        let a = ExecutionStatus::Ready;
        let b = ExecutionStatus::Ready;
        assert_eq!(a, b);

        let c = ExecutionStatus::Executing {
            step_id: StepId::new("s".to_string()),
            elapsed_ms: 10,
        };
        let d = ExecutionStatus::Executing {
            step_id: StepId::new("s".to_string()),
            elapsed_ms: 10,
        };
        assert_eq!(c, d);

        let e = ExecutionStatus::Executing {
            step_id: StepId::new("s".to_string()),
            elapsed_ms: 20,
        };
        assert_ne!(c, e);
    }
}
