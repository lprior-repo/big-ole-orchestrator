//! Connector runtime types for managed effects (ADR-041).
//!
//! Architecture: Data (ConnectorState, ConnectorResult, ReconcileAction)
//!             → Calc (apply_connector_transition, is_terminal, all_variants).
//!
//! This module defines the type system for the managed connector lifecycle.
//! No I/O, no engine integration — pure types and state machine logic.

// ============================================================================
// Data Layer: Type Definitions
// ============================================================================

/// Lifecycle state of a managed connector (ADR-041).
///
/// Follows the prepare → commit → reconcile sequence defined in ADR-041 §2.
/// Ambiguous is distinct from both Success and Failure (INV-C01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ConnectorState {
    /// Connector is idle, no operation in progress.
    Idle,

    /// Engine is preparing an effect (deriving PreparedEffect without committing).
    Preparing,

    /// Effect has been prepared, not yet committed.
    Prepared,

    /// Connector is executing commit.
    Executing,

    /// Effect committed successfully (terminal).
    Succeeded,

    /// Effect failed (terminal).
    Failed,

    /// Outcome is ambiguous — reconcile required before retry (ADR-041 §3).
    Ambiguous,
}

/// Result of a connector operation (commit, compensate, reconcile).
///
/// INV-C01: Ambiguous is distinct from both Success and Failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ConnectorResult {
    /// Operation succeeded unambiguously.
    Success,

    /// Operation failed unambiguously.
    Failure,

    /// Operation outcome is ambiguous — reconcile required (ADR-041 §3).
    Ambiguous,
}

/// Action the Engine must take after reconciliation (ADR-041 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ReconcileAction {
    /// Effect was committed — proceed.
    Commit,

    /// Effect was not committed — roll back.
    Rollback,

    /// Unable to determine — retry with backoff.
    Retry,
}

/// Events that drive ConnectorState transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectorTransition {
    /// Begin preparing an effect.
    Prepare,

    /// Preparation complete, ready to commit.
    Prepared,

    /// Begin committing the prepared effect.
    Commit,

    /// Commit succeeded.
    Succeed,

    /// Commit failed.
    Fail,

    /// Timeout or transport ambiguity detected.
    Ambiguate,

    /// Reconciliation determines the effect was committed.
    ReconcileSucceeded,

    /// Reconciliation determines the effect was not committed.
    ReconcileFailed,

    /// Reconciliation unable to determine — needs retry.
    ReconcileRetry,
}

/// Error for invalid connector state transitions.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConnectorTransitionError {
    #[error("Cannot transition from terminal connector state")]
    TerminalStateTransition,
    #[error("Invalid connector state transition")]
    InvalidTransition,
}
