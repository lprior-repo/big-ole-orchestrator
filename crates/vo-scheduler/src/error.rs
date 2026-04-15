use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum SchedulerError {
    #[error("scheduler queue is full")]
    QueueFull,

    #[error("invalid schedule policy: {reason}")]
    InvalidSchedule { reason: String },

    #[error("job not found: {0}")]
    JobNotFound(String),

    #[error("invalid state transition")]
    InvalidTransition,

    #[error("serialization error: {0}")]
    SerializationError(String),
}
