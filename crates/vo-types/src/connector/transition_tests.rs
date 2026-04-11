//! Unit tests for apply_connector_transition state machine logic.

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod transition_tests {
    use crate::connector::transition::apply_connector_transition;
    use crate::connector::types::{ConnectorState, ConnectorTransition, ConnectorTransitionError};
    use rstest::rstest;

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
