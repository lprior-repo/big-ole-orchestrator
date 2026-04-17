//! Error types for ghost workflow lifecycle operations.

use vo_types::{RegistrationStatus, WorkflowName};

/// Errors for ghost workflow lifecycle operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GhostWorkflowError {
    #[error("invalid transition: cannot go from {from:?} to {to:?} for workflow {workflow}")]
    InvalidTransition {
        workflow: String,
        from: RegistrationStatus,
        to: RegistrationStatus,
    },

    #[error("trigger rejected: workflow {workflow} is {status:?} (HTTP 404)")]
    TriggerRejected {
        workflow: String,
        status: RegistrationStatus,
    },

    #[error("cannot reactivate deleted workflow: {workflow}")]
    CannotReactivateDeleted { workflow: String },

    #[error("reaper: workflow {workflow} is {status:?}, expected Deactivated")]
    ReaperNotDeactivated {
        workflow: String,
        status: RegistrationStatus,
    },
}
