//! Scheduler error types

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
