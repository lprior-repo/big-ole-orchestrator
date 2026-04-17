use thiserror::Error;

#[derive(Debug, Error)]
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

#[derive(Debug, Error)]
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

#[derive(Debug, Error)]
pub enum RetryExhaustedError {
    #[error("max retry attempts reached")]
    MaxAttemptsReached,
    #[error("backoff calculation overflowed")]
    BackoffOverflow,
    #[error("retry not allowed for this job kind")]
    RetryNotAllowed,
}

impl SchedulerError {
    pub fn is_transient(&self) -> bool {
        matches!(self, Self::SerializationError(_) | Self::QueueFull)
    }

    pub fn is_permanent(&self) -> bool {
        matches!(self, Self::InvalidSchedule | Self::InvalidTransition)
    }
}

impl ExecutionError {
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::ResourceExhausted)
    }

    pub fn is_transient(&self) -> bool {
        matches!(self, Self::ResourceExhausted)
    }
}
