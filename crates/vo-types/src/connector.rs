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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectorTransitionError {
    /// Attempted transition from a terminal state (INV-C03).
    TerminalStateTransition,

    /// Event not valid for the current state.
    InvalidTransition,
}

impl std::fmt::Display for ConnectorTransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectorTransitionError::TerminalStateTransition => {
                write!(f, "Cannot transition from terminal connector state")
            }
            ConnectorTransitionError::InvalidTransition => {
                write!(f, "Invalid connector state transition")
            }
        }
    }
}

impl std::error::Error for ConnectorTransitionError {}

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

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use rstest::rstest;

    // ========================================================================
    // ConnectorState Derive Tests
    // ========================================================================

    #[test]
    fn connector_state_debug_format_equals_variant_name_for_idle() {
        assert_eq!(format!("{:?}", ConnectorState::Idle), "Idle");
    }

    #[test]
    fn connector_state_debug_format_equals_variant_name_for_preparing() {
        assert_eq!(format!("{:?}", ConnectorState::Preparing), "Preparing");
    }

    #[test]
    fn connector_state_debug_format_equals_variant_name_for_prepared() {
        assert_eq!(format!("{:?}", ConnectorState::Prepared), "Prepared");
    }

    #[test]
    fn connector_state_debug_format_equals_variant_name_for_executing() {
        assert_eq!(format!("{:?}", ConnectorState::Executing), "Executing");
    }

    #[test]
    fn connector_state_debug_format_equals_variant_name_for_succeeded() {
        assert_eq!(format!("{:?}", ConnectorState::Succeeded), "Succeeded");
    }

    #[test]
    fn connector_state_debug_format_equals_variant_name_for_failed() {
        assert_eq!(format!("{:?}", ConnectorState::Failed), "Failed");
    }

    #[test]
    fn connector_state_debug_format_equals_variant_name_for_ambiguous() {
        assert_eq!(format!("{:?}", ConnectorState::Ambiguous), "Ambiguous");
    }

    #[test]
    fn connector_state_clone_copy_semantics_preserve_equality() {
        let state = ConnectorState::Idle;
        let copy = state;
        assert_eq!(state, copy);

        let state1 = ConnectorState::Ambiguous;
        let state2 = state1;
        assert_eq!(state1, state2);
    }

    #[test]
    fn connector_state_partial_eq_distinguishes_all_variants() {
        assert_eq!(ConnectorState::Idle, ConnectorState::Idle);
        assert_ne!(ConnectorState::Idle, ConnectorState::Preparing);
        assert_ne!(ConnectorState::Succeeded, ConnectorState::Failed);
        assert_ne!(ConnectorState::Ambiguous, ConnectorState::Succeeded);
        assert_ne!(ConnectorState::Ambiguous, ConnectorState::Failed);
    }

    #[test]
    fn connector_state_hash_consistency_for_equal_variants() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let state1 = ConnectorState::Ambiguous;
        let state2 = ConnectorState::Ambiguous;

        let mut h1 = DefaultHasher::new();
        state1.hash(&mut h1);
        let mut h2 = DefaultHasher::new();
        state2.hash(&mut h2);
        assert_eq!(
            h1.finish(),
            h2.finish(),
            "Equal states must have equal hashes"
        );
    }

    // ========================================================================
    // ConnectorState Serde Round-Trip (parameterized)
    // ========================================================================

    #[rstest]
    #[case(ConnectorState::Idle, "Idle")]
    #[case(ConnectorState::Preparing, "Preparing")]
    #[case(ConnectorState::Prepared, "Prepared")]
    #[case(ConnectorState::Executing, "Executing")]
    #[case(ConnectorState::Succeeded, "Succeeded")]
    #[case(ConnectorState::Failed, "Failed")]
    #[case(ConnectorState::Ambiguous, "Ambiguous")]
    fn connector_state_serializes_and_deserializes_for_all_variants(
        #[case] variant: ConnectorState,
        #[case] expected_json: &str,
    ) {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, format!("\"{expected_json}\""));
        let recovered: ConnectorState = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, variant);
    }

    // ========================================================================
    // ConnectorState is_terminal
    // ========================================================================

    #[rstest]
    #[case(ConnectorState::Idle, false)]
    #[case(ConnectorState::Preparing, false)]
    #[case(ConnectorState::Prepared, false)]
    #[case(ConnectorState::Executing, false)]
    #[case(ConnectorState::Succeeded, true)]
    #[case(ConnectorState::Failed, true)]
    #[case(ConnectorState::Ambiguous, false)]
    fn connector_state_is_terminal_returns_correct_value_for_all_variants(
        #[case] state: ConnectorState,
        #[case] expected: bool,
    ) {
        assert_eq!(state.is_terminal(), expected);
    }

    // ========================================================================
    // ConnectorState all_variants
    // ========================================================================

    #[test]
    fn connector_state_all_variants_returns_seven_variants_in_declaration_order() {
        let variants = ConnectorState::all_variants();
        assert_eq!(variants.len(), 7);
        assert_eq!(variants[0], ConnectorState::Idle);
        assert_eq!(variants[1], ConnectorState::Preparing);
        assert_eq!(variants[2], ConnectorState::Prepared);
        assert_eq!(variants[3], ConnectorState::Executing);
        assert_eq!(variants[4], ConnectorState::Succeeded);
        assert_eq!(variants[5], ConnectorState::Failed);
        assert_eq!(variants[6], ConnectorState::Ambiguous);
    }

    // ========================================================================
    // ConnectorResult Derive Tests
    // ========================================================================

    #[test]
    fn connector_result_debug_format_equals_variant_name_for_success() {
        assert_eq!(format!("{:?}", ConnectorResult::Success), "Success");
    }

    #[test]
    fn connector_result_debug_format_equals_variant_name_for_failure() {
        assert_eq!(format!("{:?}", ConnectorResult::Failure), "Failure");
    }

    #[test]
    fn connector_result_debug_format_equals_variant_name_for_ambiguous() {
        assert_eq!(format!("{:?}", ConnectorResult::Ambiguous), "Ambiguous");
    }

    #[test]
    fn connector_result_clone_copy_semantics_preserve_equality() {
        let result = ConnectorResult::Success;
        let copy = result;
        assert_eq!(result, copy);
    }

    #[test]
    fn connector_result_partial_eq_distinguishes_all_variants() {
        assert_eq!(ConnectorResult::Success, ConnectorResult::Success);
        assert_ne!(ConnectorResult::Success, ConnectorResult::Failure);
        assert_ne!(ConnectorResult::Ambiguous, ConnectorResult::Success);
        assert_ne!(ConnectorResult::Ambiguous, ConnectorResult::Failure);
    }

    #[test]
    fn connector_result_hash_consistency_for_equal_variants() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let r1 = ConnectorResult::Ambiguous;
        let r2 = ConnectorResult::Ambiguous;
        let mut h1 = DefaultHasher::new();
        r1.hash(&mut h1);
        let mut h2 = DefaultHasher::new();
        r2.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
    }

    // ========================================================================
    // ConnectorResult Serde Round-Trip (parameterized)
    // ========================================================================

    #[rstest]
    #[case(ConnectorResult::Success, "Success")]
    #[case(ConnectorResult::Failure, "Failure")]
    #[case(ConnectorResult::Ambiguous, "Ambiguous")]
    fn connector_result_serializes_and_deserializes_for_all_variants(
        #[case] variant: ConnectorResult,
        #[case] expected_json: &str,
    ) {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, format!("\"{expected_json}\""));
        let recovered: ConnectorResult = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, variant);
    }

    // ========================================================================
    // ConnectorResult all_variants
    // ========================================================================

    #[test]
    fn connector_result_all_variants_returns_three_variants_in_declaration_order() {
        let variants = ConnectorResult::all_variants();
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0], ConnectorResult::Success);
        assert_eq!(variants[1], ConnectorResult::Failure);
        assert_eq!(variants[2], ConnectorResult::Ambiguous);
    }

    // ========================================================================
    // ReconcileAction Derive Tests
    // ========================================================================

    #[test]
    fn reconcile_action_debug_format_equals_variant_name_for_commit() {
        assert_eq!(format!("{:?}", ReconcileAction::Commit), "Commit");
    }

    #[test]
    fn reconcile_action_debug_format_equals_variant_name_for_rollback() {
        assert_eq!(format!("{:?}", ReconcileAction::Rollback), "Rollback");
    }

    #[test]
    fn reconcile_action_debug_format_equals_variant_name_for_retry() {
        assert_eq!(format!("{:?}", ReconcileAction::Retry), "Retry");
    }

    #[test]
    fn reconcile_action_clone_copy_semantics_preserve_equality() {
        let action = ReconcileAction::Retry;
        let copy = action;
        assert_eq!(action, copy);
    }

    #[test]
    fn reconcile_action_partial_eq_distinguishes_all_variants() {
        assert_eq!(ReconcileAction::Commit, ReconcileAction::Commit);
        assert_ne!(ReconcileAction::Commit, ReconcileAction::Rollback);
        assert_ne!(ReconcileAction::Retry, ReconcileAction::Commit);
    }

    #[test]
    fn reconcile_action_hash_consistency_for_equal_variants() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let a1 = ReconcileAction::Commit;
        let a2 = ReconcileAction::Commit;
        let mut h1 = DefaultHasher::new();
        a1.hash(&mut h1);
        let mut h2 = DefaultHasher::new();
        a2.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
    }

    // ========================================================================
    // ReconcileAction Serde Round-Trip (parameterized)
    // ========================================================================

    #[rstest]
    #[case(ReconcileAction::Commit, "Commit")]
    #[case(ReconcileAction::Rollback, "Rollback")]
    #[case(ReconcileAction::Retry, "Retry")]
    fn reconcile_action_serializes_and_deserializes_for_all_variants(
        #[case] variant: ReconcileAction,
        #[case] expected_json: &str,
    ) {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, format!("\"{expected_json}\""));
        let recovered: ReconcileAction = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, variant);
    }

    // ========================================================================
    // ReconcileAction all_variants
    // ========================================================================

    #[test]
    fn reconcile_action_all_variants_returns_three_variants_in_declaration_order() {
        let variants = ReconcileAction::all_variants();
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0], ReconcileAction::Commit);
        assert_eq!(variants[1], ReconcileAction::Rollback);
        assert_eq!(variants[2], ReconcileAction::Retry);
    }

    // ========================================================================
    // ConnectorTransition all_variants
    // ========================================================================

    #[test]
    fn connector_transition_all_variants_returns_nine_variants_in_declaration_order() {
        let variants = ConnectorTransition::all_variants();
        assert_eq!(variants.len(), 9);
        assert_eq!(variants[0], ConnectorTransition::Prepare);
        assert_eq!(variants[1], ConnectorTransition::Prepared);
        assert_eq!(variants[2], ConnectorTransition::Commit);
        assert_eq!(variants[3], ConnectorTransition::Succeed);
        assert_eq!(variants[4], ConnectorTransition::Fail);
        assert_eq!(variants[5], ConnectorTransition::Ambiguate);
        assert_eq!(variants[6], ConnectorTransition::ReconcileSucceeded);
        assert_eq!(variants[7], ConnectorTransition::ReconcileFailed);
        assert_eq!(variants[8], ConnectorTransition::ReconcileRetry);
    }

    // ========================================================================
    // ConnectorTransitionError Tests
    // ========================================================================

    #[test]
    fn connector_transition_error_terminal_state_transition_displays_correct_message() {
        let err = ConnectorTransitionError::TerminalStateTransition;
        assert_eq!(
            err.to_string(),
            "Cannot transition from terminal connector state"
        );
    }

    #[test]
    fn connector_transition_error_invalid_transition_displays_correct_message() {
        let err = ConnectorTransitionError::InvalidTransition;
        assert_eq!(err.to_string(), "Invalid connector state transition");
    }

    #[test]
    fn connector_transition_error_implements_std_error_error() {
        let err: Box<dyn std::error::Error> =
            Box::new(ConnectorTransitionError::TerminalStateTransition);
        assert_eq!(
            err.to_string(),
            "Cannot transition from terminal connector state"
        );
    }

    // ========================================================================
    // apply_connector_transition — Happy Paths (9 valid transitions)
    // ========================================================================

    #[test]
    fn apply_connector_transition_returns_preparing_when_idle_prepare() {
        let result = apply_connector_transition(ConnectorState::Idle, ConnectorTransition::Prepare);
        assert_eq!(result, Ok(ConnectorState::Preparing));
    }

    #[test]
    fn apply_connector_transition_returns_prepared_when_preparing_prepared() {
        let result =
            apply_connector_transition(ConnectorState::Preparing, ConnectorTransition::Prepared);
        assert_eq!(result, Ok(ConnectorState::Prepared));
    }

    #[test]
    fn apply_connector_transition_returns_executing_when_prepared_commit() {
        let result =
            apply_connector_transition(ConnectorState::Prepared, ConnectorTransition::Commit);
        assert_eq!(result, Ok(ConnectorState::Executing));
    }

    #[test]
    fn apply_connector_transition_returns_succeeded_when_executing_succeed() {
        let result =
            apply_connector_transition(ConnectorState::Executing, ConnectorTransition::Succeed);
        assert_eq!(result, Ok(ConnectorState::Succeeded));
    }

    #[test]
    fn apply_connector_transition_returns_failed_when_executing_fail() {
        let result =
            apply_connector_transition(ConnectorState::Executing, ConnectorTransition::Fail);
        assert_eq!(result, Ok(ConnectorState::Failed));
    }

    #[test]
    fn apply_connector_transition_returns_ambiguous_when_executing_ambiguate() {
        let result =
            apply_connector_transition(ConnectorState::Executing, ConnectorTransition::Ambiguate);
        assert_eq!(result, Ok(ConnectorState::Ambiguous));
    }

    #[test]
    fn apply_connector_transition_returns_succeeded_when_ambiguous_reconcile_succeeded() {
        let result = apply_connector_transition(
            ConnectorState::Ambiguous,
            ConnectorTransition::ReconcileSucceeded,
        );
        assert_eq!(result, Ok(ConnectorState::Succeeded));
    }

    #[test]
    fn apply_connector_transition_returns_failed_when_ambiguous_reconcile_failed() {
        let result = apply_connector_transition(
            ConnectorState::Ambiguous,
            ConnectorTransition::ReconcileFailed,
        );
        assert_eq!(result, Ok(ConnectorState::Failed));
    }

    #[test]
    fn apply_connector_transition_returns_prepared_when_ambiguous_reconcile_retry() {
        let result = apply_connector_transition(
            ConnectorState::Ambiguous,
            ConnectorTransition::ReconcileRetry,
        );
        assert_eq!(result, Ok(ConnectorState::Prepared));
    }

    // ========================================================================
    // apply_connector_transition — Terminal State Rejections (INV-C03)
    // Succeeded rejects all 9 events
    // ========================================================================

    #[rstest]
    #[case(ConnectorTransition::Prepare)]
    #[case(ConnectorTransition::Prepared)]
    #[case(ConnectorTransition::Commit)]
    #[case(ConnectorTransition::Succeed)]
    #[case(ConnectorTransition::Fail)]
    #[case(ConnectorTransition::Ambiguate)]
    #[case(ConnectorTransition::ReconcileSucceeded)]
    #[case(ConnectorTransition::ReconcileFailed)]
    #[case(ConnectorTransition::ReconcileRetry)]
    fn apply_connector_transition_returns_terminal_error_when_succeeded_receives_any_event(
        #[case] event: ConnectorTransition,
    ) {
        let result = apply_connector_transition(ConnectorState::Succeeded, event);
        assert_eq!(
            result,
            Err(ConnectorTransitionError::TerminalStateTransition)
        );
    }

    // Failed rejects all 9 events
    #[rstest]
    #[case(ConnectorTransition::Prepare)]
    #[case(ConnectorTransition::Prepared)]
    #[case(ConnectorTransition::Commit)]
    #[case(ConnectorTransition::Succeed)]
    #[case(ConnectorTransition::Fail)]
    #[case(ConnectorTransition::Ambiguate)]
    #[case(ConnectorTransition::ReconcileSucceeded)]
    #[case(ConnectorTransition::ReconcileFailed)]
    #[case(ConnectorTransition::ReconcileRetry)]
    fn apply_connector_transition_returns_terminal_error_when_failed_receives_any_event(
        #[case] event: ConnectorTransition,
    ) {
        let result = apply_connector_transition(ConnectorState::Failed, event);
        assert_eq!(
            result,
            Err(ConnectorTransitionError::TerminalStateTransition)
        );
    }

    // ========================================================================
    // apply_connector_transition — Invalid Transitions
    // ========================================================================

    #[test]
    fn apply_connector_transition_returns_invalid_when_idle_receives_prepared() {
        let result =
            apply_connector_transition(ConnectorState::Idle, ConnectorTransition::Prepared);
        assert_eq!(result, Err(ConnectorTransitionError::InvalidTransition));
    }

    #[test]
    fn apply_connector_transition_returns_invalid_when_idle_receives_commit() {
        let result = apply_connector_transition(ConnectorState::Idle, ConnectorTransition::Commit);
        assert_eq!(result, Err(ConnectorTransitionError::InvalidTransition));
    }

    #[test]
    fn apply_connector_transition_returns_invalid_when_idle_receives_succeed() {
        let result = apply_connector_transition(ConnectorState::Idle, ConnectorTransition::Succeed);
        assert_eq!(result, Err(ConnectorTransitionError::InvalidTransition));
    }

    #[test]
    fn apply_connector_transition_returns_invalid_when_idle_receives_fail() {
        let result = apply_connector_transition(ConnectorState::Idle, ConnectorTransition::Fail);
        assert_eq!(result, Err(ConnectorTransitionError::InvalidTransition));
    }

    #[test]
    fn apply_connector_transition_returns_invalid_when_idle_receives_ambiguate() {
        let result =
            apply_connector_transition(ConnectorState::Idle, ConnectorTransition::Ambiguate);
        assert_eq!(result, Err(ConnectorTransitionError::InvalidTransition));
    }

    #[test]
    fn apply_connector_transition_returns_invalid_when_preparing_receives_commit() {
        let result =
            apply_connector_transition(ConnectorState::Preparing, ConnectorTransition::Commit);
        assert_eq!(result, Err(ConnectorTransitionError::InvalidTransition));
    }

    #[test]
    fn apply_connector_transition_returns_invalid_when_preparing_receives_succeed() {
        let result =
            apply_connector_transition(ConnectorState::Preparing, ConnectorTransition::Succeed);
        assert_eq!(result, Err(ConnectorTransitionError::InvalidTransition));
    }

    #[test]
    fn apply_connector_transition_returns_invalid_when_preparing_receives_fail() {
        let result =
            apply_connector_transition(ConnectorState::Preparing, ConnectorTransition::Fail);
        assert_eq!(result, Err(ConnectorTransitionError::InvalidTransition));
    }

    #[test]
    fn apply_connector_transition_returns_invalid_when_preparing_receives_ambiguate() {
        let result =
            apply_connector_transition(ConnectorState::Preparing, ConnectorTransition::Ambiguate);
        assert_eq!(result, Err(ConnectorTransitionError::InvalidTransition));
    }

    #[test]
    fn apply_connector_transition_returns_invalid_when_prepared_receives_succeed() {
        let result =
            apply_connector_transition(ConnectorState::Prepared, ConnectorTransition::Succeed);
        assert_eq!(result, Err(ConnectorTransitionError::InvalidTransition));
    }

    #[test]
    fn apply_connector_transition_returns_invalid_when_prepared_receives_fail() {
        let result =
            apply_connector_transition(ConnectorState::Prepared, ConnectorTransition::Fail);
        assert_eq!(result, Err(ConnectorTransitionError::InvalidTransition));
    }

    #[test]
    fn apply_connector_transition_returns_invalid_when_prepared_receives_ambiguate() {
        let result =
            apply_connector_transition(ConnectorState::Prepared, ConnectorTransition::Ambiguate);
        assert_eq!(result, Err(ConnectorTransitionError::InvalidTransition));
    }

    #[test]
    fn apply_connector_transition_returns_invalid_when_executing_receives_prepare() {
        let result =
            apply_connector_transition(ConnectorState::Executing, ConnectorTransition::Prepare);
        assert_eq!(result, Err(ConnectorTransitionError::InvalidTransition));
    }

    #[test]
    fn apply_connector_transition_returns_invalid_when_executing_receives_prepared() {
        let result =
            apply_connector_transition(ConnectorState::Executing, ConnectorTransition::Prepared);
        assert_eq!(result, Err(ConnectorTransitionError::InvalidTransition));
    }

    #[test]
    fn apply_connector_transition_returns_invalid_when_executing_receives_commit() {
        let result =
            apply_connector_transition(ConnectorState::Executing, ConnectorTransition::Commit);
        assert_eq!(result, Err(ConnectorTransitionError::InvalidTransition));
    }

    #[rstest]
    #[case(ConnectorTransition::ReconcileSucceeded)]
    #[case(ConnectorTransition::ReconcileFailed)]
    #[case(ConnectorTransition::ReconcileRetry)]
    fn apply_connector_transition_returns_invalid_when_executing_receives_reconcile_event(
        #[case] event: ConnectorTransition,
    ) {
        let result = apply_connector_transition(ConnectorState::Executing, event);
        assert_eq!(result, Err(ConnectorTransitionError::InvalidTransition));
    }

    #[rstest]
    #[case(ConnectorTransition::Prepare)]
    #[case(ConnectorTransition::Prepared)]
    #[case(ConnectorTransition::Commit)]
    #[case(ConnectorTransition::Succeed)]
    #[case(ConnectorTransition::Fail)]
    #[case(ConnectorTransition::Ambiguate)]
    fn apply_connector_transition_returns_invalid_when_ambiguous_receives_non_reconcile_event(
        #[case] event: ConnectorTransition,
    ) {
        let result = apply_connector_transition(ConnectorState::Ambiguous, event);
        assert_eq!(result, Err(ConnectorTransitionError::InvalidTransition));
    }
}

// ============================================================================
// Proptest Invariants
// ============================================================================

#[cfg(feature = "proptest")]
#[allow(clippy::unwrap_used)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest::proptest! {
        /// INV: Serde round-trip preserves ConnectorState equality for all variants.
        #[test]
        fn connector_state_serde_roundtrip_preserves_equality(
            variant in proptest::sample::select(ConnectorState::all_variants())
        ) {
            let json = serde_json::to_string(&variant).unwrap();
            let recovered: ConnectorState = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(variant, recovered);
        }

        /// INV: Serde round-trip preserves ConnectorResult equality for all variants.
        #[test]
        fn connector_result_serde_roundtrip_preserves_equality(
            variant in proptest::sample::select(ConnectorResult::all_variants())
        ) {
            let json = serde_json::to_string(&variant).unwrap();
            let recovered: ConnectorResult = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(variant, recovered);
        }

        /// INV: apply_connector_transition never panics for any (state, event) combination.
        #[test]
        fn apply_connector_transition_never_panics_for_any_combination(
            state_idx in 0usize..7,
            event_idx in 0usize..9,
        ) {
            let states = ConnectorState::all_variants();
            let events = ConnectorTransition::all_variants();
            let state = states[state_idx];
            let event = events[event_idx];
            // Must not panic — all 63 combinations handled
            let _ = apply_connector_transition(state, event);
        }
    }
}

// ============================================================================
// Kani Verification Harnesses
// ============================================================================

#[cfg(kani)]
mod verification {
    use super::*;

    /// K-01: Verify apply_connector_transition exhaustiveness.
    /// All 7×9 = 63 combinations must be covered without panic (INV-C06).
    #[kani::proof]
    fn verify_connector_transition_exhaustiveness() {
        let state: u8 = kani::any();
        let event: u8 = kani::any();
        kani::assume(state < 7);
        kani::assume(event < 9);

        let current = match state {
            0 => ConnectorState::Idle,
            1 => ConnectorState::Preparing,
            2 => ConnectorState::Prepared,
            3 => ConnectorState::Executing,
            4 => ConnectorState::Succeeded,
            5 => ConnectorState::Failed,
            _ => ConnectorState::Ambiguous,
        };
        let evt = match event {
            0 => ConnectorTransition::Prepare,
            1 => ConnectorTransition::Prepared,
            2 => ConnectorTransition::Commit,
            3 => ConnectorTransition::Succeed,
            4 => ConnectorTransition::Fail,
            5 => ConnectorTransition::Ambiguate,
            6 => ConnectorTransition::ReconcileSucceeded,
            7 => ConnectorTransition::ReconcileFailed,
            _ => ConnectorTransition::ReconcileRetry,
        };

        // Must not panic — all combinations handled (INV-C06)
        let _ = apply_connector_transition(current, evt);
    }

    /// K-02: Verify terminal states always return TerminalStateTransition (INV-C03).
    #[kani::proof]
    fn verify_terminal_states_always_reject_transitions() {
        let is_succeeded: bool = kani::any();
        let event: u8 = kani::any();
        kani::assume(event < 9);

        let terminal = if is_succeeded {
            ConnectorState::Succeeded
        } else {
            ConnectorState::Failed
        };
        let evt = match event {
            0 => ConnectorTransition::Prepare,
            1 => ConnectorTransition::Prepared,
            2 => ConnectorTransition::Commit,
            3 => ConnectorTransition::Succeed,
            4 => ConnectorTransition::Fail,
            5 => ConnectorTransition::Ambiguate,
            6 => ConnectorTransition::ReconcileSucceeded,
            7 => ConnectorTransition::ReconcileFailed,
            _ => ConnectorTransition::ReconcileRetry,
        };

        let result = apply_connector_transition(terminal, evt);
        assert!(matches!(
            result,
            Err(ConnectorTransitionError::TerminalStateTransition)
        ));
    }
}
