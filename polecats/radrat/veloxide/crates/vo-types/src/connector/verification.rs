//! Kani verification harnesses for connector state machine (kani-gated).

#[cfg(kani)]
mod verification {
    use crate::connector::transition::apply_connector_transition;
    use crate::connector::types::*;

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
