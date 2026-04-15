//! Scheduler error types
//!
//! Error taxonomy aligned to ADR-047:
//! - SchedulerError: Configuration and operations errors
//! - ExecutionError: Job execution errors
//! - RetryExhaustedError: Retry policy exhaustion errors

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SchedulerError {
    #[error("Job not found: {0}")]
    JobNotFound(super::JobId),

    #[error("Queue is full")]
    QueueFull,

    #[error("Scheduler is stopped")]
    SchedulerStopped,

    #[error("Invalid schedule: {0}")]
    InvalidSchedule(String),

    #[error("Concurrency limit reached")]
    ConcurrencyLimitReached,

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Invalid transition from {from_state} via {event}")]
    InvalidTransition { from_state: String, event: String },

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("Job {job_id} panicked: {reason}")]
    Panicked {
        job_id: super::JobId,
        reason: String,
    },

    #[error("Job {job_id} timed out after {timeout_ms}ms")]
    TimedOut {
        job_id: super::JobId,
        timeout_ms: u64,
    },

    #[error("Job {job_id} was cancelled")]
    Cancelled { job_id: super::JobId },

    #[error("Job {job_id} exhausted resources")]
    ResourceExhausted { job_id: super::JobId },
}

#[derive(Debug, Error)]
pub enum RetryExhaustedError {
    #[error("Job {job_id} reached max attempts ({max_attempts})")]
    MaxAttemptsReached {
        job_id: super::JobId,
        max_attempts: u32,
    },

    #[error("Backoff calculation overflow for job {job_id}")]
    BackoffOverflow { job_id: super::JobId },

    #[error("Job {job_id} kind does not support retries")]
    RetryNotAllowed { job_id: super::JobId },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum JobRunError {
    #[error("Job {job_id} failed: {reason}")]
    Failed {
        job_id: super::JobId,
        reason: String,
    },

    #[error("Job {job_id} exceeded retries ({attempts} attempts)")]
    ExceededRetries { job_id: super::JobId, attempts: u32 },

    #[error("Job {job_id} cancelled")]
    Cancelled { job_id: super::JobId },
}
