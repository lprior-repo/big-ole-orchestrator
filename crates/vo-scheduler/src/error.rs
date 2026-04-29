//! Error types for vo-scheduler.
//!
//! Re-exports unified error types from vo-common with backwards-compatible
//! type aliases for existing code.

pub use vo_common::{ExecutionError, RetryError as RetryExhaustedError, SchedulerError};

/// Backwards-compatible alias for the unified scheduler error.
pub type SchedulerErrorType = SchedulerError;

/// Backwards-compatible alias for the unified execution error.
pub type ExecutionErrorType = ExecutionError;

/// Backwards-compatible alias for the unified retry error.
pub type RetryExhaustedErrorType = RetryExhaustedError;



    /// Returns true if this error is permanent.
    #[inline]
    pub fn is_permanent(&self) -> bool {
        <vo_common::SchedulerError as std::ops::Deref>::deref(self).is_permanent()
    }
}

/// Helper to create a SchedulerError::JobNotFound from a string.
pub fn job_not_found(job_id: impl Into<String>) -> SchedulerError {
    SchedulerError::JobNotFound { job_id: job_id.into() }
}

/// Helper to create a SchedulerError::InvalidTransition.
pub fn invalid_transition(from_state: impl Into<String>, action: impl Into<String>) -> SchedulerError {
    SchedulerError::InvalidTransition {
        from_state: from_state.into(),
        action: action.into(),
    }
}

/// Helper to create an ExecutionError::Cancelled.
pub fn execution_cancelled(reason: impl Into<String>) -> ExecutionError {
    ExecutionError::Cancelled { reason: reason.into() }
}
