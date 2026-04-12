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
