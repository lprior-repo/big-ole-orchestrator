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
    pub jitter_factor: f64,
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
            jitter_factor: 0.1,
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
            jitter_factor: 0.1,
        })
    }

    /// Configure the jitter factor for this retry policy.
    ///
    /// Jitter helps prevent thundering herd by randomizing retry timing.
    /// A jitter_factor of 0.0 means no jitter (deterministic backoff).
    /// A jitter_factor of 0.5 means the retry delay can vary by ±50%.
    pub fn with_jitter(mut self, jitter_factor: f64) -> Self {
        self.jitter_factor = jitter_factor;
        self
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

    /// Calculate jittered delay by adding randomization to base duration.
    ///
    /// Jitter prevents thundering herd by spreading out retry times.
    /// If jitter_factor is 0.0, returns base_duration unchanged.
    #[must_use]
    pub fn calculate_jitter(&self, base_duration: u64) -> u64 {
        if self.jitter_factor <= 0.0 {
            return base_duration;
        }
        let base_ms = base_duration as f64;
        let jitter_range = base_ms * self.jitter_factor;
        let jitter_ms = rand_jitter(jitter_range);
        let total_ms = (base_ms + jitter_ms).abs().min(u64::MAX as f64);
        total_ms as u64
    }
}

fn rand_jitter(range: f64) -> f64 {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let normalized: f64 = rng.gen_range(-1.0..1.0);
    normalized * range
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
    fn calculate_jitter_zero_factor_returns_base() {
        let p = RetryPolicy::new(3, 1000, 2.0).unwrap().with_jitter(0.0);
        assert_eq!(p.calculate_jitter(1000), 1000);
    }

    #[test]
    fn calculate_jitter_with_factor_adds_variation() {
        let p = RetryPolicy::new(3, 1000, 2.0).unwrap().with_jitter(0.5);
        let base = 1000u64;
        let jittered = p.calculate_jitter(base);
        let diff = (jittered as i64 - base as i64).abs();
        let allowed_range = 1000i64 * 50 / 100;
        assert!(
            diff <= allowed_range,
            "Jitter {} out of range ±50%: diff={}, allowed={}",
            jittered,
            diff,
            allowed_range
        );
    }

    #[test]
    fn calculate_jitter_distribution_reasonable() {
        let p = RetryPolicy::new(3, 1000, 2.0).unwrap().with_jitter(0.3);
        let base = 1000u64;
        let jitter_range = 1000.0 * 0.3;

        let mut min_sample = u64::MAX;
        let mut max_sample = u64::MIN;
        let num_samples = 100;

        for _ in 0..num_samples {
            let jittered = p.calculate_jitter(base);
            min_sample = min_sample.min(jittered);
            max_sample = max_sample.max(jittered);
        }

        let min_expected = base as i64 - jitter_range as i64;
        let max_expected = base as i64 + jitter_range as i64;
        assert!(
            min_sample as i64 >= min_expected,
            "Min sample {} below expected {}",
            min_sample,
            min_expected
        );
        assert!(
            max_sample as i64 <= max_expected,
            "Max sample {} above expected {}",
            max_sample,
            max_expected
        );
    }

    #[test]
    fn calculate_jitter_concurrent_differentiation() {
        use std::sync::Arc;
        use std::thread;

        let p = Arc::new(RetryPolicy::new(3, 1000, 2.0).unwrap().with_jitter(0.5));
        let base = 1000u64;

        let mut handles: Vec<thread::JoinHandle<u64>> = Vec::new();
        for _ in 0..100 {
            let p = p.clone();
            handles.push(thread::spawn(move || p.calculate_jitter(base)));
        }

        let mut values: Vec<u64> = Vec::new();
        for h in handles {
            values.push(h.join().unwrap());
        }

        let total_count = values.len();
        let unique_values: std::collections::HashSet<u64> = values.into_iter().collect();
        let uniqueness_ratio = unique_values.len() as f64 / total_count as f64;
        assert!(
            uniqueness_ratio > 0.5,
            "Too many duplicate jitter values: {}/{} unique ({:.1}%), expected >50%",
            unique_values.len(),
            total_count,
            uniqueness_ratio * 100.0
        );
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
