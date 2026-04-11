//! Error types for the Reanimator Loop.

use std::time::Duration;

use thiserror::Error;
use vo_types::InstanceId;

/// Errors that can occur in the Reanimator Loop.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReanimatorError {
    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Corrupt key format: {0}")]
    CorruptKey(String),

    #[error("Atomicity violation: {0}")]
    AtomicityViolation(String),

    #[error("Instance not found: {0}")]
    InstanceNotFound(InstanceId),

    #[error("Budget exceeded: {0}")]
    BudgetExceeded(String),

    #[error("Failed to enqueue resume work: {0}")]
    EnqueueFailed(String),

    #[error("Reanimator is already running")]
    AlreadyRunning,

    #[error("Storage initialization failed: {0}")]
    StorageInitFailed(String),

    #[error("Failed to spawn reanimator task: {0}")]
    TaskSpawnFailed(String),

    #[error("Reanimator has already shut down")]
    AlreadyShutdown,

    #[error("Shutdown timed out after {0:?}")]
    ShutdownTimeout(Duration),
}

impl ReanimatorError {
    /// Returns true if this error indicates a transient failure that may succeed on retry.
    #[must_use]
    pub const fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::StorageError(_)
                | Self::EnqueueFailed(_)
                | Self::AtomicityViolation(_)
                | Self::BudgetExceeded(_)
        )
    }

    /// Returns true if this error indicates the operation should not be retried.
    #[must_use]
    pub const fn is_fatal(&self) -> bool {
        matches!(
            self,
            Self::CorruptKey(_)
                | Self::InstanceNotFound(_)
                | Self::AlreadyRunning
                | Self::AlreadyShutdown
        )
    }
}
