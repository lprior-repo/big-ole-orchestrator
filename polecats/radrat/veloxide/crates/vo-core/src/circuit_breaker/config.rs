//! Circuit breaker configuration with validated construction.

use std::time::Duration;

/// Configuration constants for the circuit breaker.
/// All values are compile-time or startup-configurable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CircuitBreakerConfig {
    /// Minimum interval between registrations per workflow.
    /// Default: 60 seconds.
    pub rate_limit_window: Duration,

    /// Sliding window for failure tracking.
    /// Default: 10 minutes (600 seconds).
    pub failure_window: Duration,

    /// Number of unique-hash failures to trigger quarantine.
    /// Default: 5.
    pub failure_threshold: u8,
}

/// Error returned when `CircuitBreakerConfig::new` receives invalid parameters.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigValidationError {
    /// `rate_limit_window` must be greater than `Duration::ZERO`.
    #[error("invalid config: rate_limit_window must be > 0")]
    ZeroRateLimitWindow,

    /// `failure_window` must be greater than `Duration::ZERO`.
    #[error("invalid config: failure_window must be > 0")]
    ZeroFailureWindow,

    /// `failure_threshold` must be >= 1.
    #[error("invalid config: failure_threshold must be >= 1")]
    ZeroFailureThreshold,
}

impl CircuitBreakerConfig {
    /// Create a new validated config.
    ///
    /// # Errors
    /// Returns the specific `ConfigValidationError` variant for the first
    /// invalid field encountered.
    pub fn new(
        rate_limit_window: Duration,
        failure_window: Duration,
        failure_threshold: u8,
    ) -> Result<Self, ConfigValidationError> {
        if rate_limit_window == Duration::ZERO {
            return Err(ConfigValidationError::ZeroRateLimitWindow);
        }
        if failure_window == Duration::ZERO {
            return Err(ConfigValidationError::ZeroFailureWindow);
        }
        if failure_threshold == 0 {
            return Err(ConfigValidationError::ZeroFailureThreshold);
        }
        Ok(Self {
            rate_limit_window,
            failure_window,
            failure_threshold,
        })
    }

    /// Returns the default config: 60s rate limit, 10min failure window, threshold 5.
    ///
    /// # Errors
    /// Returns `ConfigValidationError` if the hardcoded defaults are invalid (should never happen).
    pub fn default_config() -> Result<Self, ConfigValidationError> {
        Self::new(Duration::from_mins(1), Duration::from_mins(10), 5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // B-41: Config rejects zero rate_limit_window
    #[test]
    fn config_rejects_zero_rate_limit_window() {
        let result = CircuitBreakerConfig::new(Duration::ZERO, Duration::from_secs(600), 5);
        assert_eq!(result, Err(ConfigValidationError::ZeroRateLimitWindow));
    }

    // B-42: Config rejects zero failure_window
    #[test]
    fn config_rejects_zero_failure_window() {
        let result = CircuitBreakerConfig::new(Duration::from_secs(60), Duration::ZERO, 5);
        assert_eq!(result, Err(ConfigValidationError::ZeroFailureWindow));
    }

    // B-43: Config rejects zero failure_threshold
    #[test]
    fn config_rejects_zero_failure_threshold() {
        let result =
            CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 0);
        assert_eq!(result, Err(ConfigValidationError::ZeroFailureThreshold));
    }

    // B-44: Config accepts valid non-zero values
    #[test]
    fn config_accepts_valid_non_zero_values() {
        let result =
            CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 5);
        assert_eq!(
            result,
            Ok(CircuitBreakerConfig {
                rate_limit_window: Duration::from_secs(60),
                failure_window: Duration::from_secs(600),
                failure_threshold: 5,
            })
        );
    }

    // Combinatorial: all zero -> first zero field detected (rate_limit_window checked first)
    #[test]
    fn config_rejects_all_zero_with_first_zero_field() {
        let result = CircuitBreakerConfig::new(Duration::ZERO, Duration::ZERO, 0);
        assert_eq!(result, Err(ConfigValidationError::ZeroRateLimitWindow));
    }

    // Combinatorial: min valid (1ns, 1ns, 1)
    #[test]
    fn config_accepts_minimum_valid_values() {
        let result = CircuitBreakerConfig::new(Duration::from_nanos(1), Duration::from_nanos(1), 1);
        assert_eq!(
            result,
            Ok(CircuitBreakerConfig {
                rate_limit_window: Duration::from_nanos(1),
                failure_window: Duration::from_nanos(1),
                failure_threshold: 1,
            })
        );
    }

    // Combinatorial: max valid (Duration::MAX, Duration::MAX, 255)
    #[test]
    fn config_accepts_maximum_valid_values() {
        let result = CircuitBreakerConfig::new(Duration::MAX, Duration::MAX, 255);
        assert_eq!(
            result,
            Ok(CircuitBreakerConfig {
                rate_limit_window: Duration::MAX,
                failure_window: Duration::MAX,
                failure_threshold: 255,
            })
        );
    }

    // PROP-11: CircuitBreakerConfig construction property
    // For Red phase, we use proptest to check the iff condition
    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn config_new_succeeds_iff_all_fields_nonzero(
                rl_secs in 0u64..=600,
                fw_secs in 0u64..=1200,
                threshold in 0u8..=255,
            ) {
                let rl = Duration::from_secs(rl_secs);
                let fw = Duration::from_secs(fw_secs);
                let result = CircuitBreakerConfig::new(rl, fw, threshold);

                let all_nonzero = rl_secs > 0 && fw_secs > 0 && threshold > 0;
                if all_nonzero {
                    assert_eq!(
                        result,
                        Ok(CircuitBreakerConfig {
                            rate_limit_window: rl,
                            failure_window: fw,
                            failure_threshold: threshold,
                        })
                    );
                } else {
                    // Verify the error matches the first zero field (priority order)
                    if rl_secs == 0 {
                        assert_eq!(result, Err(ConfigValidationError::ZeroRateLimitWindow));
                    } else if fw_secs == 0 {
                        assert_eq!(result, Err(ConfigValidationError::ZeroFailureWindow));
                    } else {
                        assert_eq!(result, Err(ConfigValidationError::ZeroFailureThreshold));
                    }
                }
            }
        }
    }
}
