//! Error types for vo-common.
//!
//! Provides `VoError` for general errors and the shared scheduler/execution
//! error taxonomy used across vo-scheduler and vo-executor.

use serde::{Deserialize, Serialize};
use thiserror::Error;

// -- General errors ----------------------------------------------------------

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

// -- Scheduler error taxonomy ------------------------------------------------
// Shared across vo-scheduler (simple variants) and vo-executor (which adds
// per-crate JobId-typed variants locally).

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Error)]
pub enum SchedulerError {
    #[error("scheduler queue full")]
    QueueFull,
    #[error("invalid schedule policy")]
    InvalidSchedule,
    #[error("job not found")]
    JobNotFound,
    #[error("invalid state transition")]
    InvalidTransition,
    #[error("serialization error: {0}")]
    SerializationError(String),
    #[error("duration overflow: {0}")]
    DurationOverflow(String),
}

impl SchedulerError {
    pub fn is_transient(&self) -> bool {
        matches!(self, Self::SerializationError(_) | Self::QueueFull)
    }

    pub fn is_permanent(&self) -> bool {
        matches!(self, Self::InvalidSchedule | Self::InvalidTransition)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Error)]
pub enum ExecutionError {
    #[error("job panicked during execution")]
    Panicked,
    #[error("job timed out")]
    TimedOut,
    #[error("job cancelled during execution")]
    Cancelled,
    #[error("job exhausted available resources")]
    ResourceExhausted,
}

impl ExecutionError {
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::ResourceExhausted)
    }

    pub fn is_transient(&self) -> bool {
        matches!(self, Self::ResourceExhausted)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Error)]
pub enum RetryExhaustedError {
    #[error("max retry attempts reached")]
    MaxAttemptsReached,
    #[error("backoff calculation overflowed")]
    BackoffOverflow,
    #[error("retry not allowed for this job kind")]
    RetryNotAllowed,
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
    fn scheduler_error_classification() {
        assert!(SchedulerError::QueueFull.is_transient());
        assert!(!SchedulerError::QueueFull.is_permanent());
        assert!(SchedulerError::InvalidSchedule.is_permanent());
        assert!(!SchedulerError::InvalidSchedule.is_transient());
    }

    #[test]
    fn execution_error_classification() {
        assert!(ExecutionError::ResourceExhausted.is_retryable());
        assert!(!ExecutionError::Panicked.is_retryable());
    }
}
