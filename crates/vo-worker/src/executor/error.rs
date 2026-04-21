//! Managed-effect execution error types (ADR-030).

/// Errors produced by the managed-effect execution path.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ManagedEffectError {
    #[error("connector prepare failed: {0}")]
    PrepareFailed(String),

    #[error("connector commit failed: {0}")]
    CommitFailed(String),

    #[error("reconciliation failed after ambiguous outcome: {0}")]
    ReconciliationFailed(String),

    #[error("effect already in terminal state: {current:?}")]
    TerminalState { current: vo_types::EffectIntent },

    #[error("fence violation: expected >= {expected}, got {actual}")]
    FenceViolation { expected: u64, actual: u64 },

    #[error("connector not found: {0}")]
    ConnectorNotFound(String),
}

impl ManagedEffectError {
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::CommitFailed(_) | Self::ReconciliationFailed(_))
    }
}
