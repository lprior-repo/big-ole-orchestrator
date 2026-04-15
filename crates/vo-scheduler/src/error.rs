//! Scheduler error types per ADR-047 §4.

use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum SchedulerError {
    #[error("scheduler queue is at capacity")]
    QueueFull,

    #[error("invalid schedule policy: {0}")]
    InvalidSchedule(String),

    #[error("job not found: {0:?}")]
    JobNotFound(String),

    #[error("invalid state transition for job {0:?}")]
    InvalidTransition(String),

    #[error("serialization error: {0}")]
    SerializationError(String),
}

#[derive(Debug, Clone, Error)]
pub enum ExecutionError {
    #[error("job task panicked")]
    Panicked,

    #[error("job exceeded time limit")]
    TimedOut,

    #[error("job was cancelled")]
    Cancelled,

    #[error("job exhausted available resources")]
    ResourceExhausted,
}

#[derive(Debug, Clone, Error)]
pub enum RetryExhaustedError {
    #[error("maximum retry attempts reached")]
    MaxAttemptsReached,

    #[error("backoff calculation overflowed")]
    BackoffOverflow,

    #[error("job kind does not support retries")]
    RetryNotAllowed,
}
