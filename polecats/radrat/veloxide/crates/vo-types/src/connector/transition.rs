//! Calc layer: pure state machine logic for connector transitions (ADR-041).

use crate::connector::types::{
    ConnectorResult, ConnectorState, ConnectorTransition, ConnectorTransitionError, ReconcileAction,
};

// ============================================================================
// Calc Layer: Pure Functions
// ============================================================================

impl ConnectorState {
    /// Check if this state is terminal (Succeeded or Failed).
    ///
    /// INV-C03: Only Succeeded and Failed are terminal.
    /// Note: Ambiguous is NOT terminal — it can be reconciled.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, ConnectorState::Succeeded | ConnectorState::Failed)
    }

    /// Returns all ConnectorState variants in declaration order.
    #[must_use]
    pub const fn all_variants() -> &'static [ConnectorState] {
        &[
            ConnectorState::Idle,
            ConnectorState::Preparing,
            ConnectorState::Prepared,
            ConnectorState::Executing,
            ConnectorState::Succeeded,
            ConnectorState::Failed,
            ConnectorState::Ambiguous,
        ]
    }
}

impl ConnectorResult {
    /// Returns all ConnectorResult variants in declaration order.
    #[must_use]
    pub const fn all_variants() -> &'static [ConnectorResult] {
        &[
            ConnectorResult::Success,
            ConnectorResult::Failure,
            ConnectorResult::Ambiguous,
        ]
    }
}

impl ReconcileAction {
    /// Returns all ReconcileAction variants in declaration order.
    #[must_use]
    pub const fn all_variants() -> &'static [ReconcileAction] {
        &[
            ReconcileAction::Commit,
            ReconcileAction::Rollback,
            ReconcileAction::Retry,
        ]
    }
}

impl ConnectorTransition {
    /// Returns all ConnectorTransition variants in declaration order.
    #[must_use]
    pub const fn all_variants() -> &'static [ConnectorTransition] {
        &[
            ConnectorTransition::Prepare,
            ConnectorTransition::Prepared,
            ConnectorTransition::Commit,
            ConnectorTransition::Succeed,
            ConnectorTransition::Fail,
            ConnectorTransition::Ambiguate,
            ConnectorTransition::ReconcileSucceeded,
            ConnectorTransition::ReconcileFailed,
            ConnectorTransition::ReconcileRetry,
        ]
    }
}

/// Apply a state transition to a ConnectorState (ADR-041).
///
/// # Errors
///
/// Returns `ConnectorTransitionError::TerminalStateTransition` if the current state
/// is Succeeded or Failed (INV-C03).
/// Returns `ConnectorTransitionError::InvalidTransition` if the event is not valid
/// for the current state.
///
/// # Invariants Enforced
/// - INV-C02: Transitions follow ADR-041 durability sequence strictly.
/// - INV-C03: Terminal states reject all transitions.
/// - INV-C04: Ambiguous only transitions via reconciliation events.
/// - INV-C06: Never panics — always returns Result.
pub fn apply_connector_transition(
    current: ConnectorState,
    event: ConnectorTransition,
) -> Result<ConnectorState, ConnectorTransitionError> {
    match (current, event) {
        // Valid transitions (INV-C02): follow ADR-041 durability sequence
        (ConnectorState::Idle, ConnectorTransition::Prepare) => Ok(ConnectorState::Preparing),
        (ConnectorState::Preparing, ConnectorTransition::Prepared) => Ok(ConnectorState::Prepared),
        (ConnectorState::Prepared, ConnectorTransition::Commit) => Ok(ConnectorState::Executing),
        (ConnectorState::Executing, ConnectorTransition::Succeed) => Ok(ConnectorState::Succeeded),
        (ConnectorState::Executing, ConnectorTransition::Fail) => Ok(ConnectorState::Failed),
        (ConnectorState::Executing, ConnectorTransition::Ambiguate) => {
            Ok(ConnectorState::Ambiguous)
        }

        // Reconciliation transitions (INV-C04): only from Ambiguous
        (ConnectorState::Ambiguous, ConnectorTransition::ReconcileSucceeded) => {
            Ok(ConnectorState::Succeeded)
        }
        (ConnectorState::Ambiguous, ConnectorTransition::ReconcileFailed) => {
            Ok(ConnectorState::Failed)
        }
        (ConnectorState::Ambiguous, ConnectorTransition::ReconcileRetry) => {
            Ok(ConnectorState::Prepared)
        }

        // Terminal states reject all transitions (INV-C03)
        (ConnectorState::Succeeded | ConnectorState::Failed, _) => {
            Err(ConnectorTransitionError::TerminalStateTransition)
        }

        // All other combinations are invalid
        _ => Err(ConnectorTransitionError::InvalidTransition),
    }
}
