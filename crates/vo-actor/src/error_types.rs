/// Error types for orchestrator-level operations.
///
/// Includes `TerminateError`, `SignalError`, `CompensateError`, and `StartError`
/// (which depends on `WorkloadClass` from the `fairness` module).
use crate::fairness::WorkloadClass;

// =============================================================================
// TerminateError
// =============================================================================

#[derive(Debug, thiserror::Error)]
pub enum TerminateError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("failed: {0}")]
    Failed(String),
}

#[cfg(test)]
mod terminate_error_tests {
    use super::*;

    #[test]
    fn terminate_error_variants_can_be_constructed() {
        let err_not_found = TerminateError::NotFound("wf-123".to_string());
        assert!(matches!(err_not_found, TerminateError::NotFound(msg) if msg == "wf-123"));

        let err_failed = TerminateError::Failed("crashed".to_string());
        assert!(matches!(err_failed, TerminateError::Failed(msg) if msg == "crashed"));
    }
}

// =============================================================================
// SignalError
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SignalError {
    #[error("instance not found: {0}")]
    NotFound(String),
    #[error("signal failed: {0}")]
    Failed(String),
}

#[cfg(test)]
mod signal_error_tests {
    use super::*;

    #[test]
    fn signal_error_variants_can_be_constructed() {
        let err = SignalError::NotFound("inst-1".to_string());
        assert!(matches!(err, SignalError::NotFound(msg) if msg == "inst-1"));

        let err = SignalError::Failed("timeout".to_string());
        assert!(matches!(err, SignalError::Failed(msg) if msg == "timeout"));
    }
}

// =============================================================================
// CompensateError
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompensateError {
    #[error("instance not found: {0}")]
    NotFound(String),
    #[error("compensation failed: {0}")]
    Failed(String),
}

// =============================================================================
// StartError
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StartError {
    #[error("Budget exhausted for {class:?}: requested {requested}, available {available}")]
    BudgetExhaustion {
        class: WorkloadClass,
        requested: u32,
        available: u32,
    },
    #[error("Invalid config: {0}")]
    InvalidConfig(String),
    #[error("At capacity: {running}/{max} instances running")]
    AtCapacity { running: u32, max: u32 },
    #[error("Instance {0} already exists")]
    AlreadyExists(String),
    #[error("Spawn failed: {0}")]
    SpawnFailed(String),
}
