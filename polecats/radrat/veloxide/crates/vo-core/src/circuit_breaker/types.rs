//! Domain types for the circuit breaker module.

use std::time::Instant;

use vo_types::{BinaryHash, WorkflowName};

use crate::circuit_breaker::RegistrationStatus;

/// A single failure observation for the circuit breaker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureRecord {
    pub hash: BinaryHash,
    pub failed_at: Instant,
}

/// Input for a binary registration attempt.
#[derive(Debug, Clone)]
pub struct RegistrationRequest {
    pub workflow_name: WorkflowName,
    pub binary_hash: BinaryHash,
    /// True if the operator provided `--force`.
    pub force: bool,
}

/// Result of the circuit breaker evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrationOutcome {
    /// Registration is permitted. Proceed with binary registration.
    Allowed,
    /// Registration denied: rate limit exceeded.
    RateLimited { retry_after_secs: u64 },
    /// Registration denied: workflow is quarantined.
    WorkflowQuarantined { workflow_name: WorkflowName },
    /// Registration denied: workflow is deactivated.
    WorkflowDeactivated { workflow_name: WorkflowName },
}

/// Event emitted when a workflow is quarantined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarantineEvent {
    pub workflow_name: WorkflowName,
}

/// Successful result of an unquarantine operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnquarantineResult {
    pub workflow_name: WorkflowName,
    pub previous_status: RegistrationStatus,
    pub new_status: RegistrationStatus,
    pub failures_cleared: usize,
}

/// Errors that can occur during circuit breaker operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CircuitBreakerError {
    /// Attempted to register a binary for a rate-limited workflow.
    #[error("rate_limited: retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },

    /// Attempted to register a binary for a quarantined workflow.
    #[error("workflow_quarantined: {workflow_name}")]
    WorkflowQuarantined { workflow_name: String },

    /// Attempted to register a binary for a deactivated workflow.
    #[error("workflow_deactivated: {workflow_name}")]
    WorkflowDeactivated { workflow_name: String },

    /// Persistence failure when reading/writing quarantine state.
    #[error("storage_error: {reason}")]
    StorageError { reason: String },

    /// Workflow not found when attempting unquarantine.
    #[error("workflow_not_found: {workflow_name}")]
    WorkflowNotFound { workflow_name: String },

    /// Attempted to unquarantine a workflow that is not quarantined.
    #[error("not_quarantined: {workflow_name} is {current_status:?}")]
    NotQuarantined {
        workflow_name: String,
        current_status: RegistrationStatus,
    },
}
