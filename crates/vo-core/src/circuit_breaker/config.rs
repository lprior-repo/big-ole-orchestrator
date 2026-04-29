//! Circuit breaker configuration with validated construction.
//!
//! This module defines the [`CircuitBreakerConfig`] struct, which holds the
//! tunable parameters for the circuit breaker system. All values are validated
//! at construction time via [`CircuitBreakerConfig::new()`].
//!
//! # Default Configuration
//!
//! The default configuration provides sensible values for production use:
//!
//! | Parameter | Default | Description |
//! |-----------|---------|-------------|
//! | `rate_limit_window` | 60s | Cooldown between registrations per workflow |
//! | `failure_window` | 10min (600s) | Sliding window for failure tracking |
//! | `failure_threshold` | 5 | Unique binary hash failures to trigger quarantine |
//!
//! These defaults can be obtained via [`CircuitBreakerConfig::default_config()`].
//!
//! # Configuration Rationale
//!
//! - **60s rate limit window**: Prevents rapid re-registration of faulty binaries
//!   while allowing operators to retry after a brief cooldown.
//! - **10min failure window**: A 10-minute sliding window captures repeated
//!   failures from a single deployment iteration without being overly sensitive
//!   to transient failures.
//! - **5 unique hash threshold**: Requires 5 distinct binary builds to fail
//!   before quarantine, reducing false positives from single-fluke failures.
//!
//! # Examples
//!
//! ## Using defaults
//!
//! ```
//! use vo_core::circuit_breaker::CircuitBreakerConfig;
//!
//! let config = CircuitBreakerConfig::default_config().unwrap();
//! assert_eq!(config.rate_limit_window.as_secs(), 60);
//! assert_eq!(config.failure_window.as_secs(), 600);
//! assert_eq!(config.failure_threshold, 5);
//! ```
//!
//! ## Custom configuration
//!
//! ```
//! use vo_core::circuit_breaker::{CircuitBreakerConfig, ConfigValidationError};
//! use std::time::Duration;
//!
//! let config = CircuitBreakerConfig::new(
//!     Duration::from_secs(30),   // 30s rate limit
//!     Duration::from_secs(300),  // 5min failure window
//!     3,                         // 3 unique failures → quarantine
//! ).unwrap();
//!
//! assert_eq!(config.rate_limit_window, Duration::from_secs(30));
//! ```
//!
//! ## Invalid configuration
//!
//! ```
//! use vo_core::circuit_breaker::{CircuitBreakerConfig, ConfigValidationError};
//! use std::time::Duration;
//!
//! let result = CircuitBreakerConfig::new(Duration::ZERO, Duration::from_secs(60), 5);
//! assert_eq!(result, Err(ConfigValidationError::ZeroRateLimitWindow));
//! ```

use std::time::Duration;

/// Configuration constants for the circuit breaker.
///
/// All values are validated at construction time. Zero values are rejected to
/// prevent misconfiguration (e.g., zero rate limit window would block all
/// registrations indefinitely).
///
/// # Fields
///
/// | Field | Type | Default | Description |
/// |-------|------|---------|-------------|
/// | `rate_limit_window` | `Duration` | 60s | Minimum interval between registrations per workflow. |
/// | `failure_window` | `Duration` | 10min | Sliding window duration for failure tracking. |
/// | `failure_threshold` | `u8` | 5 | Number of unique-hash failures to trigger quarantine. |
///
/// # Validation Rules
///
/// - `rate_limit_window > Duration::ZERO`
/// - `failure_window > Duration::ZERO`
/// - `failure_threshold >= 1`
///
/// Use [`CircuitBreakerConfig::new()`] for validated construction or
/// [`CircuitBreakerConfig::default_config()`] for pre-validated defaults.
///
/// # Immutability
///
/// The struct derives `Copy`, making it cheap to pass around. Configuration
/// is set once at startup and never changed during the lifetime of the process.
///
/// # Examples
///
/// ```
/// use vo_core::circuit_breaker::CircuitBreakerConfig;
///
/// let config = CircuitBreakerConfig::default_config().unwrap();
/// println!(
///     "Rate limit: {:?}, Failure window: {:?}, Threshold: {}",
///     config.rate_limit_window,
///     config.failure_window,
///     config.failure_threshold,
/// );
/// // Rate limit: 60s, Failure window: 600s, Threshold: 5
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CircuitBreakerConfig {
    /// Minimum interval between registrations per workflow.
    ///
    /// After a successful registration, the workflow cannot be registered again
    /// until this duration has elapsed. This prevents rapid re-registration of
    /// faulty binaries.
    ///
    /// # Constraints
    ///
    /// Must be greater than `Duration::ZERO`. A zero or negative value would
    /// effectively block all registrations.
    ///
    /// # Default
    ///
    /// 60 seconds.
    pub rate_limit_window: Duration,

    /// Sliding window for failure tracking.
    ///
    /// Failure records older than this duration are evicted from the
    /// [`FailureWindow`][crate::circuit_breaker::FailureWindow]. The window
    /// slides continuously — at any point in time, only failures within the
    /// last `failure_window` seconds are counted toward the quarantine threshold.
    ///
    /// # Constraints
    ///
    /// Must be greater than `Duration::ZERO`. A zero window would immediately
    /// evict all failure records, making quarantine impossible.
    ///
    /// # Default
    ///
    /// 10 minutes (600 seconds).
    pub failure_window: Duration,

    /// Number of unique-hash failures to trigger quarantine.
    ///
    /// When a workflow's [`FailureWindow`][crate::circuit_breaker::FailureWindow]
    /// contains this many distinct binary hashes that have failed, the workflow
    /// is automatically quarantined.
    ///
    /// # Uniqueness
    ///
    /// Only **unique** binary hashes count toward the threshold. If the same
    /// binary fails multiple times, it counts as one failure. This prevents
    /// repeated failures from the same build from triggering quarantine too
    /// quickly.
    ///
    /// # Constraints
    ///
    /// Must be at least 1. A threshold of 0 would quarantine every workflow
    /// on its first failure.
    ///
    /// # Default
    ///
    /// 5 unique failures.
    pub failure_threshold: u8,
}

/// Error returned when `CircuitBreakerConfig::new` receives invalid parameters.
///
/// This enum identifies which specific field failed validation. The errors are
/// returned in a fixed priority order: `rate_limit_window`, then `failure_window`,
/// then `failure_threshold`. Only the first invalid field is reported.
///
/// # Variants
///
/// | Variant | Condition | Fix |
/// |---------|-----------|-----|
/// | `ZeroRateLimitWindow` | `rate_limit_window == Duration::ZERO` | Use a positive duration |
/// | `ZeroFailureWindow` | `failure_window == Duration::ZERO` | Use a positive duration |
/// | `ZeroFailureThreshold` | `failure_threshold == 0` | Use a value ≥ 1 |
///
/// # Examples
///
/// ```
/// use vo_core::circuit_breaker::{CircuitBreakerConfig, ConfigValidationError};
/// use std::time::Duration;
///
/// let result = CircuitBreakerConfig::new(
///     Duration::ZERO,
///     Duration::from_secs(600),
///     5,
/// );
/// assert_eq!(result, Err(ConfigValidationError::ZeroRateLimitWindow));
/// ```
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
    /// All three parameters are validated. If any parameter is invalid, the
    /// first invalid field (in priority order: rate_limit_window → failure_window
    /// → failure_threshold) is returned as an error.
    ///
    /// # Arguments
    ///
    /// * `rate_limit_window` — Minimum interval between registrations per workflow.
    ///   Must be > `Duration::ZERO`.
    /// * `failure_window` — Sliding window duration for failure tracking.
    ///   Must be > `Duration::ZERO`.
    /// * `failure_threshold` — Number of unique-hash failures to trigger quarantine.
    ///   Must be ≥ 1.
    ///
    /// # Errors
    ///
    /// Returns the specific `ConfigValidationError` variant for the first
    /// invalid field encountered, checked in the order listed above.
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::circuit_breaker::CircuitBreakerConfig;
    /// use std::time::Duration;
    ///
    /// let config = CircuitBreakerConfig::new(
    ///     Duration::from_secs(30),
    ///     Duration::from_secs(300),
    ///     3,
    /// ).unwrap();
    /// ```
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
    /// This is a convenience constructor that returns a pre-validated configuration
    /// with the recommended production defaults.
    ///
    /// # Default Values
    ///
    /// | Parameter | Value |
    /// |-----------|-------|
    /// | `rate_limit_window` | 60 seconds |
    /// | `failure_window` | 10 minutes (600 seconds) |
    /// | `failure_threshold` | 5 |
    ///
    /// # Errors
    ///
    /// Returns `ConfigValidationError` if the hardcoded defaults are invalid
    /// (should never happen — the defaults are verified at build time by tests).
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::circuit_breaker::CircuitBreakerConfig;
    /// use std::time::Duration;
    ///
    /// let config = CircuitBreakerConfig::default_config().unwrap();
    /// assert_eq!(config.rate_limit_window, Duration::from_secs(60));
    /// assert_eq!(config.failure_window, Duration::from_secs(600));
    /// assert_eq!(config.failure_threshold, 5);
    /// ```
    pub fn default_config() -> Result<Self, ConfigValidationError> {
        Self::new(Duration::from_mins(1), Duration::from_mins(10), 5)
    }
}
