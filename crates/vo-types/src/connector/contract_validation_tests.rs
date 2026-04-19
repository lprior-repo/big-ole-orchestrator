//! Connector contract validation tests (ve-k6try).
//!
//! Tests that connectors must satisfy defined contracts for lifecycle methods.
//! Covers valid connector state machine transitions, invalid transition rejection,
//! terminal state protection, and reconciliation-only constraints.

#[cfg(test)]
mod tests {
    use crate::connector::types::{
        ConnectorResult, ConnectorState, ConnectorTransition, ConnectorTransitionError, ReconcileAction,
    };
    use crate::connector::transition::apply_connector_transition;

    // ── Valid lifecycle transitions ──────────────────────────────────────

    #[test]
    fn valid_full_happy_path() {
        let mut state = ConnectorState::Idle;
        state = apply_connector_transition(state, ConnectorTransition::Prepare).unwrap();
        assert_eq!(state, ConnectorState::Preparing);

        state = apply_connector_transition(state, ConnectorTransition::Prepared).unwrap();
        assert_eq!(state, ConnectorState::Prepared);

        state = apply_connector_transition(state, ConnectorTransition::Commit).unwrap();
        assert_eq!(state, ConnectorState::Executing);

        state = apply_connector_transition(state, ConnectorTransition::Succeed).unwrap();
        assert_eq!(state, ConnectorState::Succeeded);
    }

    #[test]
    fn valid_failure_path() {
        let mut state = ConnectorState::Idle;
        state = apply_connector_transition(state, ConnectorTransition::Prepare).unwrap();
        state = apply_connector_transition(state, ConnectorTransition::Prepared).unwrap();
        state = apply_connector_transition(state, ConnectorTransition::Commit).unwrap();
        state = apply_connector_transition(state, ConnectorTransition::Fail).unwrap();
        assert_eq!(state, ConnectorState::Failed);
    }

    #[test]
    fn valid_ambiguous_then_reconcile_succeeded() {
        let mut state = ConnectorState::Idle;
        state = apply_connector_transition(state, ConnectorTransition::Prepare).unwrap();
        state = apply_connector_transition(state, ConnectorTransition::Prepared).unwrap();
        state = apply_connector_transition(state, ConnectorTransition::Commit).unwrap();
        state = apply_connector_transition(state, ConnectorTransition::Ambiguate).unwrap();
        assert_eq!(state, ConnectorState::Ambiguous);

        state = apply_connector_transition(state, ConnectorTransition::ReconcileSucceeded).unwrap();
        assert_eq!(state, ConnectorState::Succeeded);
    }

    #[test]
    fn valid_ambiguous_then_reconcile_failed() {
        let mut state = ConnectorState::Idle;
        state = apply_connector_transition(state, ConnectorTransition::Prepare).unwrap();
        state = apply_connector_transition(state, ConnectorTransition::Prepared).unwrap();
        state = apply_connector_transition(state, ConnectorTransition::Commit).unwrap();
        state = apply_connector_transition(state, ConnectorTransition::Ambiguate).unwrap();

        state = apply_connector_transition(state, ConnectorTransition::ReconcileFailed).unwrap();
        assert_eq!(state, ConnectorState::Failed);
    }

    #[test]
    fn valid_ambiguous_then_reconcile_retry() {
        let mut state = ConnectorState::Idle;
        state = apply_connector_transition(state, ConnectorTransition::Prepare).unwrap();
        state = apply_connector_transition(state, ConnectorTransition::Prepared).unwrap();
        state = apply_connector_transition(state, ConnectorTransition::Commit).unwrap();
        state = apply_connector_transition(state, ConnectorTransition::Ambiguate).unwrap();

        state = apply_connector_transition(state, ConnectorTransition::ReconcileRetry).unwrap();
        assert_eq!(state, ConnectorState::Prepared);
    }

    // ── Invalid transition rejection ─────────────────────────────────────

    #[test]
    fn reject_skip_prepare_to_prepared() {
        let result = apply_connector_transition(ConnectorState::Idle, ConnectorTransition::Prepared);
        assert!(matches!(result, Err(ConnectorTransitionError::InvalidTransition)));
    }

    #[test]
    fn reject_skip_prepare_to_commit() {
        let result = apply_connector_transition(ConnectorState::Idle, ConnectorTransition::Commit);
        assert!(matches!(result, Err(ConnectorTransitionError::InvalidTransition)));
    }

    #[test]
    fn reject_idle_direct_succeed() {
        let result = apply_connector_transition(ConnectorState::Idle, ConnectorTransition::Succeed);
        assert!(matches!(result, Err(ConnectorTransitionError::InvalidTransition)));
    }

    #[test]
    fn reject_preparing_direct_succeed() {
        let result = apply_connector_transition(ConnectorState::Preparing, ConnectorTransition::Succeed);
        assert!(matches!(result, Err(ConnectorTransitionError::InvalidTransition)));
    }

    #[test]
    fn reject_prepared_direct_succeed() {
        let result = apply_connector_transition(ConnectorState::Prepared, ConnectorTransition::Succeed);
        assert!(matches!(result, Err(ConnectorTransitionError::InvalidTransition)));
    }

    #[test]
    fn reject_double_prepare() {
        let state = ConnectorState::Preparing;
        let result = apply_connector_transition(state, ConnectorTransition::Prepare);
        assert!(matches!(result, Err(ConnectorTransitionError::InvalidTransition)));
    }

    // ── Terminal state protection (INV-C03) ─────────────────────────────

    #[test]
    fn reject_all_transitions_from_succeeded() {
        let transitions = ConnectorTransition::all_variants();
        for &transition in transitions {
            let result = apply_connector_transition(ConnectorState::Succeeded, transition);
            assert!(
                matches!(result, Err(ConnectorTransitionError::TerminalStateTransition)),
                "Succeeded should reject {:?}",
                transition
            );
        }
    }

    #[test]
    fn reject_all_transitions_from_failed() {
        let transitions = ConnectorTransition::all_variants();
        for &transition in transitions {
            let result = apply_connector_transition(ConnectorState::Failed, transition);
            assert!(
                matches!(result, Err(ConnectorTransitionError::TerminalStateTransition)),
                "Failed should reject {:?}",
                transition
            );
        }
    }

    // ── Reconciliation-only from Ambiguous (INV-C04) ────────────────────

    #[test]
    fn reject_non_reconciliation_from_ambiguous() {
        let non_reconciliation = [
            ConnectorTransition::Prepare,
            ConnectorTransition::Prepared,
            ConnectorTransition::Commit,
            ConnectorTransition::Succeed,
            ConnectorTransition::Fail,
            ConnectorTransition::Ambiguate,
        ];
        for transition in non_reconciliation {
            let result = apply_connector_transition(ConnectorState::Ambiguous, transition);
            assert!(
                matches!(result, Err(ConnectorTransitionError::InvalidTransition)),
                "Ambiguous should reject non-reconciliation {:?}",
                transition
            );
        }
    }

    #[test]
    fn allow_all_reconciliation_from_ambiguous() {
        let reconciliation = [
            ConnectorTransition::ReconcileSucceeded,
            ConnectorTransition::ReconcileFailed,
            ConnectorTransition::ReconcileRetry,
        ];
        for transition in reconciliation {
            let result = apply_connector_transition(ConnectorState::Ambiguous, transition);
            assert!(result.is_ok(), "Ambiguous should allow {:?}", transition);
        }
    }

    // ── Variant exhaustiveness ───────────────────────────────────────────

    #[test]
    fn connector_state_has_seven_variants() {
        let all = ConnectorState::all_variants();
        assert_eq!(all.len(), 7);
    }

    #[test]
    fn connector_transition_has_nine_variants() {
        let all = ConnectorTransition::all_variants();
        assert_eq!(all.len(), 9);
    }

    #[test]
    fn connector_result_has_three_variants() {
        let all = ConnectorResult::all_variants();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn reconcile_action_has_three_variants() {
        let all = ReconcileAction::all_variants();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn only_succeeded_and_failed_are_terminal() {
        for &state in ConnectorState::all_variants() {
            let expected = matches!(state, ConnectorState::Succeeded | ConnectorState::Failed);
            assert_eq!(
                state.is_terminal(),
                expected,
                "{:?} terminal check mismatch",
                state
            );
        }
    }
}
