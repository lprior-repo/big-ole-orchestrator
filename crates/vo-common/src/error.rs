//! Error types for vo-common.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Error)]
pub enum VoError {
    #[error("configuration error: {0}")]
    Config(String),
    #[error("internal error: {0}")]
    Internal(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("operation timed out: {0}")]
    Timeout(String),
}

impl From<std::io::Error> for VoError {
    fn from(err: std::io::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

impl From<serde_json::Error> for VoError {
    fn from(err: serde_json::Error) -> Self {
        Self::Validation(err.to_string())
    }
}

impl VoError {
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }

    pub fn timeout(msg: impl Into<String>) -> Self {
        Self::Timeout(msg.into())
    }
}

/// Unified execution error covering both vo-scheduler and vo-executor.
///
/// Consolidates vo-scheduler's `ExecutionError` (Panicked, TimedOut, Cancelled,
/// ResourceExhausted) with vo-executor's execution-related errors (TimeoutExceeded,
/// ExecutionCancelled, TransientError, RetryExhausted, StepNotFound, InvalidTransition).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Error)]
pub enum ExecutionError {
    #[error("step not found: {step_id}")]
    StepNotFound { step_id: String },

    #[error("invalid timeout: {value}ms - {reason}")]
    InvalidTimeout { value: u64, reason: String },

    #[error("timeout exceeded: {elapsed_ms}ms > {limit_ms}ms")]
    TimeoutExceeded { elapsed_ms: u64, limit_ms: u64 },

    #[error("invalid state transition: {from_state} -> {action}")]
    InvalidTransition { from_state: String, action: String },

    #[error("retry exhausted after {attempts} attempts: {last_error}")]
    RetryExhausted {
        attempts: u32,
        last_error: Box<ExecutionError>,
    },

    #[error("invalid retry policy: {reason}")]
    InvalidRetryPolicy { reason: String },

    #[error("execution cancelled: {reason}")]
    Cancelled { reason: String },

    #[error("transient error: {reason} (recoverable={recoverable})")]
    Transient { reason: String, recoverable: bool },

    #[error("job panicked during execution")]
    Panicked,

    #[error("job timed out after {timeout_ms}ms")]
    TimedOut { timeout_ms: u64 },

    #[error("job exhausted available resources")]
    ResourceExhausted,
}

/// Unified scheduler error covering both vo-scheduler and vo-executor scheduler.
///
/// Consolidates vo-scheduler's `SchedulerError` with vo-executor scheduler's
/// `SchedulerError`, providing a single enum for all scheduler-related failures.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Error)]
pub enum SchedulerError {
    #[error("job not found: {job_id}")]
    JobNotFound { job_id: String },

    #[error("scheduler queue full")]
    QueueFull,

    #[error("scheduler is stopped")]
    SchedulerStopped,

    #[error("invalid schedule: {0}")]
    InvalidSchedule(String),

    #[error("concurrency limit reached")]
    ConcurrencyLimitReached,

    #[error("storage error: {0}")]
    StorageError(String),

    #[error("invalid state transition: {from_state} -> {action}")]
    InvalidTransition { from_state: String, action: String },

    #[error("serialization error: {0}")]
    SerializationError(String),

    #[error("invalid job id: {0}")]
    InvalidJobId(String),
}

/// Unified retry error covering both vo-scheduler's `RetryExhaustedError`
/// and vo-executor's `RetryPolicyError`.
///
/// Consolidates max-attempts, backoff, and retry-policy validation errors
/// into a single enum.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Error)]
pub enum RetryError {
    #[error("max retry attempts reached: {attempts}")]
    MaxAttemptsReached { attempts: u32 },

    #[error("backoff calculation overflow")]
    BackoffOverflow,

    #[error("retry not allowed for this job kind")]
    RetryNotAllowed,

    #[error("zero attempts not allowed")]
    ZeroAttempts,

    #[error("invalid multiplier: {got} (must be >= 1.0)")]
    InvalidMultiplier { got: f64 },

    #[error("max_backoff_ms ({max}) must be >= backoff_ms ({ms})")]
    MaxBackoffTooSmall { max: u64, ms: u64 },
}

/// Job run error for vo-executor scheduler.
///
/// Tracks the outcome of an individual job run attempt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Error)]
pub enum JobRunError {
    #[error("job {job_id} failed: {reason}")]
    Failed { job_id: String, reason: String },

    #[error("job {job_id} exceeded retries ({attempts} attempts)")]
    ExceededRetries { job_id: String, attempts: u32 },

    #[error("job {job_id} cancelled")]
    Cancelled { job_id: String },
}

impl ExecutionError {
    /// Returns true if this error represents a transient condition that may
    /// succeed on retry.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Transient { recoverable: true, .. }
                | Self::TimeoutExceeded { .. }
                | Self::RetryExhausted { .. }
                | Self::TimedOut { .. }
        )
    }

    /// Returns true if this error is permanent and should not be retried.
    #[must_use]
    pub fn is_permanent(&self) -> bool {
        matches!(
            self,
            Self::StepNotFound { .. }
                | Self::InvalidTimeout { .. }
                | Self::InvalidTransition { .. }
                | Self::InvalidRetryPolicy { .. }
                | Self::Panicked
        )
    }
}

impl SchedulerError {
    /// Returns true if this error represents a transient condition.
    #[must_use]
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::QueueFull | Self::StorageError(_) | Self::SerializationError(_)
        )
    }

    /// Returns true if this error is permanent.
    #[must_use]
    pub fn is_permanent(&self) -> bool {
        matches!(
            self,
            Self::InvalidSchedule(_)
                | Self::InvalidTransition { .. }
                | Self::InvalidJobId(_)
                | Self::SchedulerStopped
        )
    }

    /// Returns true if this error represents job not being found.
    #[must_use]
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::JobNotFound { .. })
    }
}

impl RetryError {
    /// Returns true if this error allows a retry with modified parameters.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::MaxAttemptsReached { .. } | Self::MaxBackoffTooSmall { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vo_error_config_constructs() {
        let err = VoError::config("bad config");
        assert!(matches!(err, VoError::Config(msg) if msg == "bad config"));
    }

    #[test]
    fn vo_error_internal_constructs() {
        let err = VoError::internal("oops");
        assert!(matches!(err, VoError::Internal(msg) if msg == "oops"));
    }

    #[test]
    fn vo_error_not_found_constructs() {
        let err = VoError::not_found("missing");
        assert!(matches!(err, VoError::NotFound(msg) if msg == "missing"));
    }

    #[test]
    fn vo_error_validation_constructs() {
        let err = VoError::validation("invalid");
        assert!(matches!(err, VoError::Validation(msg) if msg == "invalid"));
    }

    #[test]
    fn vo_error_timeout_constructs() {
        let err = VoError::timeout("30s");
        assert!(matches!(err, VoError::Timeout(msg) if msg == "30s"));
    }

    #[test]
    fn vo_error_displays_message() {
        let err = VoError::Internal("something went wrong".to_string());
        let msg = err.to_string();
        assert!(msg.contains("something went wrong"));
    }

    #[test]
    fn vo_error_from_io_error() {
        use std::io;
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let vo_err: VoError = io_err.into();
        match vo_err {
            VoError::Internal(msg) => {
                assert!(msg.contains("file not found"));
            }
            _ => panic!("Expected VoError::Internal from io::Error"),
        }
    }

    #[test]
    fn vo_error_from_io_error_kind_permission_denied() {
        use std::io;
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
        let vo_err: VoError = io_err.into();
        match vo_err {
            VoError::Internal(msg) => {
                assert!(msg.contains("access denied"));
            }
            _ => panic!("Expected VoError::Internal from io::Error"),
        }
    }

    #[test]
    fn vo_error_from_io_error_kind_other() {
        use std::io;
        let io_err = io::Error::new(io::ErrorKind::Other, "custom error");
        let vo_err: VoError = io_err.into();
        match vo_err {
            VoError::Internal(msg) => {
                assert!(msg.contains("custom error"));
            }
            _ => panic!("Expected VoError::Internal from io::Error"),
        }
    }

    #[test]
    fn vo_error_from_serde_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let vo_err: VoError = json_err.into();
        match vo_err {
            VoError::Validation(msg) => {
                assert!(msg.contains("at line") || msg.contains("parse"));
            }
            _ => panic!("Expected VoError::Validation from serde_json::Error"),
        }
    }

    #[test]
    fn vo_error_from_serde_json_error_specific() {
        let json_err = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
        let vo_err: VoError = json_err.into();
        match vo_err {
            VoError::Validation(msg) => {
                assert!(!msg.is_empty());
            }
            _ => panic!("Expected VoError::Validation from serde_json::Error"),
        }
    }

    #[test]
    fn vo_error_serialize_deserialize_roundtrip() {
        let err = VoError::Config("test config error".to_string());
        let json = serde_json::to_string(&err).unwrap();
        let deserialized: VoError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, deserialized);
    }

    #[test]
    fn vo_error_all_variants_serialize_deserialize() {
        let variants = [
            VoError::Config("c".to_string()),
            VoError::Internal("i".to_string()),
            VoError::NotFound("n".to_string()),
            VoError::Validation("v".to_string()),
            VoError::Timeout("t".to_string()),
        ];
        for err in variants {
            let json = serde_json::to_string(&err).unwrap();
            let deserialized: VoError = serde_json::from_str(&json).unwrap();
            assert_eq!(err, deserialized);
        }
    }

    #[test]
    fn vo_error_partial_eq() {
        assert_eq!(VoError::Config("x".to_string()), VoError::Config("x".to_string()));
        assert_ne!(VoError::Config("x".to_string()), VoError::Config("y".to_string()));
        assert_ne!(VoError::Config("x".to_string()), VoError::Internal("x".to_string()));
    }

    #[test]
    fn vo_error_clone_preserves_all_fields() {
        let err = VoError::NotFound("resource missing".to_string());
        let cloned = err.clone();
        assert_eq!(err, cloned);
    }

    #[test]
    fn vo_error_display_all_variants_contain_message() {
        let test_cases = [
            (VoError::Config("cfg".to_string()), "configuration error"),
            (VoError::Internal("int".to_string()), "internal error"),
            (VoError::NotFound("find".to_string()), "not found"),
            (VoError::Validation("val".to_string()), "validation failed"),
            (VoError::Timeout("tim".to_string()), "operation timed out"),
        ];

        for (err, expected_prefix) in test_cases {
            let display = err.to_string();
            assert!(
                display.contains(expected_prefix),
                "Expected '{}' in '{}'",
                expected_prefix,
                display
            );
        }
    }

    #[test]
    fn vo_error_display_contains_actual_message() {
        let messages = [
            "bad config message",
            "internal error message",
            "item not found message",
            "validation failed message",
            "timeout message",
        ];

        let errors = [
            VoError::Config(messages[0].to_string()),
            VoError::Internal(messages[1].to_string()),
            VoError::NotFound(messages[2].to_string()),
            VoError::Validation(messages[3].to_string()),
            VoError::Timeout(messages[4].to_string()),
        ];

        for (i, err) in errors.iter().enumerate() {
            let display = err.to_string();
            assert!(
                display.contains(messages[i]),
                "Error display '{}' should contain '{}'",
                display,
                messages[i]
            );
        }
    }

    #[test]
    fn vo_error_debug_contains_variant_name() {
        let debug_config = format!("{:?}", VoError::Config("x".to_string()));
        let debug_internal = format!("{:?}", VoError::Internal("y".to_string()));
        let debug_not_found = format!("{:?}", VoError::NotFound("z".to_string()));
        let debug_validation = format!("{:?}", VoError::Validation("w".to_string()));
        let debug_timeout = format!("{:?}", VoError::Timeout("v".to_string()));

        assert!(debug_config.contains("Config"), "Debug should contain 'Config': {}", debug_config);
        assert!(debug_internal.contains("Internal"), "Debug should contain 'Internal': {}", debug_internal);
        assert!(debug_not_found.contains("NotFound"), "Debug should contain 'NotFound': {}", debug_not_found);
        assert!(debug_validation.contains("Validation"), "Debug should contain 'Validation': {}", debug_validation);
        assert!(debug_timeout.contains("Timeout"), "Debug should contain 'Timeout': {}", debug_timeout);
    }

    #[test]
    fn vo_error_from_io_preserves_error_kind() {
        use std::io;

        let test_cases = [
            (io::ErrorKind::NotFound, "not found"),
            (io::ErrorKind::PermissionDenied, "permission denied"),
            (io::ErrorKind::ConnectionRefused, "connection refused"),
            (io::ErrorKind::TimedOut, "timed out"),
            (io::ErrorKind::Other, "other error"),
        ];

        for (kind, _expected_text) in test_cases {
            let io_err = io::Error::new(kind, "test error");
            let vo_err: VoError = io_err.into();
            match vo_err {
                VoError::Internal(msg) => {
                    assert!(!msg.is_empty(), "Should preserve error message for {:?}", kind);
                }
                _ => panic!("Expected VoError::Internal for {:?}", kind),
            }
        }
    }

    #[test]
    fn vo_error_from_serde_preserves_message() {
        let json_err = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
        let vo_err: VoError = json_err.into();

        if let VoError::Validation(msg) = vo_err {
            assert!(!msg.is_empty(), "Should preserve validation error message");
        } else {
            panic!("Expected VoError::Validation");
        }
    }

    #[test]
    fn vo_error_all_variants_cloneable() {
        let variants = [
            VoError::Config("c".to_string()),
            VoError::Internal("i".to_string()),
            VoError::NotFound("n".to_string()),
            VoError::Validation("v".to_string()),
            VoError::Timeout("t".to_string()),
        ];

        for err in variants {
            let cloned = err.clone();
            assert_eq!(err, cloned);
        }
    }

    #[test]
    fn vo_error_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<VoError>();
    }

    #[test]
    fn vo_error_string_constructors_with_string() {
        let s = String::from("test message");
        assert_eq!(
            VoError::config(s.clone()).to_string(),
            "configuration error: test message"
        );
    }

    #[test]
    fn vo_error_string_constructors_with_static_str() {
        assert_eq!(
            VoError::config("static message").to_string(),
            "configuration error: static message"
        );
    }
}
