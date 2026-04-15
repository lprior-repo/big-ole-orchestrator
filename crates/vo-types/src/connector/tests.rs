//! BDD tests for ADR-041 Managed Connector Runtime Contract.
//!
//! Tests the prepare → commit → reconcile lifecycle, state machine transitions,
//! ambiguity resolution, and the full runtime contract defined in ADR-041.

#[cfg(test)]
mod bdd_lifecycle {
    //! BDD Scenario: Full lifecycle from Idle to terminal states.

    use crate::connector::{
        apply_connector_transition, ConnectorState, ConnectorTransition,
    };

    #[tokio::test]
    async fn bdd_lifecycle_full_happy_path_to_succeeded() {
        // GIVEN connector starts in Idle state
        let mut state = ConnectorState::Idle;

        // WHEN transitions applied through the full ADR-041 durability sequence
        state = apply_connector_transition(state, ConnectorTransition::Prepare).unwrap();
        assert_eq!(state, ConnectorState::Preparing);

        state = apply_connector_transition(state, ConnectorTransition::Prepared).unwrap();
        assert_eq!(state, ConnectorState::Prepared);

        state = apply_connector_transition(state, ConnectorTransition::Commit).unwrap();
        assert_eq!(state, ConnectorState::Executing);

        state = apply_connector_transition(state, ConnectorTransition::Succeed).unwrap();
        // THEN state reaches Succeeded (terminal)
        assert_eq!(state, ConnectorState::Succeeded);
        assert!(state.is_terminal());
    }

    #[tokio::test]
    async fn bdd_lifecycle_full_happy_path_to_failed() {
        // GIVEN connector starts in Idle state
        let mut state = ConnectorState::Idle;

        // WHEN transitions applied through the durability sequence
        state = apply_connector_transition(state, ConnectorTransition::Prepare).unwrap();
        state = apply_connector_transition(state, ConnectorTransition::Prepared).unwrap();
        state = apply_connector_transition(state, ConnectorTransition::Commit).unwrap();

        // WHEN Fail transition applied during Executing
        state = apply_connector_transition(state, ConnectorTransition::Fail).unwrap();
        // THEN state reaches Failed (terminal)
        assert_eq!(state, ConnectorState::Failed);
        assert!(state.is_terminal());
    }

    #[tokio::test]
    async fn given_idle_when_prepare_applied_then_state_progresses_to_preparing() {
        // GIVEN connector starts in Idle state
        let state = ConnectorState::Idle;

        // WHEN Prepare transition applied
        let next = apply_connector_transition(state, ConnectorTransition::Prepare).unwrap();

        // THEN state progresses to Preparing
        assert_eq!(next, ConnectorState::Preparing);
        assert!(!next.is_terminal());
    }

    #[tokio::test]
    async fn given_preparing_when_prepared_applied_then_state_progresses_to_prepared() {
        // GIVEN connector in Preparing state (after prepare phase)
        let state = ConnectorState::Preparing;

        // WHEN Prepared transition applied
        let next = apply_connector_transition(state, ConnectorTransition::Prepared).unwrap();

        // THEN state progresses to Prepared
        assert_eq!(next, ConnectorState::Prepared);
        assert!(!next.is_terminal());
    }

    #[tokio::test]
    async fn given_prepared_when_commit_applied_then_state_progresses_to_executing() {
        // GIVEN connector in Prepared state (effect derived, not yet committed)
        let state = ConnectorState::Prepared;

        // WHEN Commit transition applied
        let next = apply_connector_transition(state, ConnectorTransition::Commit).unwrap();

        // THEN state progresses to Executing
        assert_eq!(next, ConnectorState::Executing);
        assert!(!next.is_terminal());
    }
}

#[cfg(test)]
mod bdd_reconciliation {
    //! BDD Scenario: Execution failure and reconciliation routing.
    //!
    //! ADR-041 §3: A connector timeout does not mean the effect failed.
    //! On timeout or transport ambiguity, recovery must call reconcile()
    //! before any retry.

    use crate::connector::{
        apply_connector_transition, reconcile_ambiguous, Connector, ConnectorError,
        ConnectorResult, ConnectorState, ConnectorTransition, ReconcileAction,
        ReconciliationResult,
    };

    struct ReconcileConnector {
        result: ReconciliationResult,
    }

    impl ReconcileConnector {
        fn new(result: ReconciliationResult) -> Self {
            Self { result }
        }
    }

    impl Connector for ReconcileConnector {
        async fn prepare(&mut self) -> Result<ConnectorResult, ConnectorError> {
            Ok(ConnectorResult::Success)
        }

        async fn commit(&mut self) -> Result<ConnectorResult, ConnectorError> {
            Ok(ConnectorResult::Ambiguous)
        }

        async fn reconcile(&mut self) -> Result<ReconciliationResult, ConnectorError> {
            Ok(self.result)
        }

        async fn rollback(&mut self) -> Result<ConnectorResult, ConnectorError> {
            Ok(ConnectorResult::Success)
        }
    }

    #[tokio::test]
    async fn given_execution_failure_when_reconcile_determines_committed_then_action_is_commit() {
        // GIVEN connector in Ambiguous state (execution timed out)
        let state = ConnectorState::Ambiguous;
        let mut connector = ReconcileConnector::new(ReconciliationResult::Committed);

        // WHEN reconcile_ambiguous() called
        let action = reconcile_ambiguous(&mut connector, state).await.unwrap();

        // THEN ReconcileAction is Commit (effect was committed, proceed)
        assert_eq!(action, ReconcileAction::Commit);

        // AND the state machine transitions to Succeeded
        let next =
            apply_connector_transition(state, ConnectorTransition::ReconcileSucceeded).unwrap();
        assert_eq!(next, ConnectorState::Succeeded);
        assert!(next.is_terminal());
    }

    #[tokio::test]
    async fn given_execution_failure_when_reconcile_determines_not_committed_then_action_is_rollback() {
        // GIVEN connector in Ambiguous state (execution timed out)
        let state = ConnectorState::Ambiguous;
        let mut connector = ReconcileConnector::new(ReconciliationResult::NotCommitted);

        // WHEN reconcile_ambiguous() called
        let action = reconcile_ambiguous(&mut connector, state).await.unwrap();

        // THEN ReconcileAction is Rollback (effect was not committed, roll back)
        assert_eq!(action, ReconcileAction::Rollback);

        // AND the state machine transitions to Failed
        let next =
            apply_connector_transition(state, ConnectorTransition::ReconcileFailed).unwrap();
        assert_eq!(next, ConnectorState::Failed);
        assert!(next.is_terminal());
    }

    #[tokio::test]
    async fn given_execution_failure_when_reconcile_returns_unknown_then_action_is_retry() {
        // GIVEN connector in Ambiguous state (execution timed out)
        let state = ConnectorState::Ambiguous;
        let mut connector = ReconcileConnector::new(ReconciliationResult::Unknown);

        // WHEN reconcile_ambiguous() called
        let action = reconcile_ambiguous(&mut connector, state).await.unwrap();

        // THEN ReconcileAction is Retry (unable to determine, retry with backoff)
        assert_eq!(action, ReconcileAction::Retry);

        // AND the state machine transitions back to Prepared for retry
        let next = apply_connector_transition(state, ConnectorTransition::ReconcileRetry).unwrap();
        assert_eq!(next, ConnectorState::Prepared);
        assert!(!next.is_terminal());
    }

    #[tokio::test]
    async fn given_reconcile_called_on_non_ambiguous_state_then_returns_error() {
        // GIVEN connector NOT in Ambiguous state (e.g., Executing)
        let state = ConnectorState::Executing;
        let mut connector = ReconcileConnector::new(ReconciliationResult::Unknown);

        // WHEN reconcile_ambiguous() called on non-Ambiguous state
        let result = reconcile_ambiguous(&mut connector, state).await;

        // THEN error is returned — reconcile is only valid from Ambiguous
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ConnectorError::InvalidState { .. }));
    }

    #[tokio::test]
    async fn given_reconcile_called_on_idle_state_then_returns_error() {
        // GIVEN connector in Idle state
        let state = ConnectorState::Idle;
        let mut connector = ReconcileConnector::new(ReconciliationResult::Committed);

        // WHEN reconcile_ambiguous() called
        let result = reconcile_ambiguous(&mut connector, state).await;

        // THEN error is returned
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn given_reconcile_called_on_succeeded_state_then_returns_error() {
        // GIVEN connector in Succeeded (terminal) state
        let state = ConnectorState::Succeeded;
        let mut connector = ReconcileConnector::new(ReconciliationResult::Committed);

        // WHEN reconcile_ambiguous() called
        let result = reconcile_ambiguous(&mut connector, state).await;

        // THEN error is returned
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod bdd_ambiguity_resolution {
    //! BDD Scenario: Ambiguous result (timeout) safe resolution.
    //!
    //! ADR-041 §3: Retrying commit without reconciliation is forbidden
    //! unless the connector contract explicitly proves it is safe.

    use crate::connector::{
        apply_connector_transition, reconcile_ambiguous, Connector, ConnectorError,
        ConnectorResult, ConnectorState, ConnectorTransition, ReconcileAction,
        ReconciliationResult,
    };

    /// Connector that returns Ambiguous on commit, simulating a timeout.
    struct AmbiguousOnCommitConnector {
        reconcile_result: ReconciliationResult,
    }

    impl AmbiguousOnCommitConnector {
        fn new(reconcile_result: ReconciliationResult) -> Self {
            Self { reconcile_result }
        }
    }

    impl Connector for AmbiguousOnCommitConnector {
        async fn prepare(&mut self) -> Result<ConnectorResult, ConnectorError> {
            Ok(ConnectorResult::Success)
        }

        async fn commit(&mut self) -> Result<ConnectorResult, ConnectorError> {
            Ok(ConnectorResult::Ambiguous)
        }

        async fn reconcile(&mut self) -> Result<ReconciliationResult, ConnectorError> {
            Ok(self.reconcile_result)
        }

        async fn rollback(&mut self) -> Result<ConnectorResult, ConnectorError> {
            Ok(ConnectorResult::Success)
        }
    }

    #[tokio::test]
    async fn given_ambiguous_commit_when_reconcile_resolves_committed_then_safe_resolution() {
        // GIVEN connector enters Ambiguous state after commit timeout
        let mut state = ConnectorState::Idle;
        state = apply_connector_transition(state, ConnectorTransition::Prepare).unwrap();
        state = apply_connector_transition(state, ConnectorTransition::Prepared).unwrap();
        state = apply_connector_transition(state, ConnectorTransition::Commit).unwrap();
        state = apply_connector_transition(state, ConnectorTransition::Ambiguate).unwrap();
        assert_eq!(state, ConnectorState::Ambiguous);

        let mut connector = AmbiguousOnCommitConnector::new(ReconciliationResult::Committed);

        // WHEN reconcile_ambiguous() called to determine true outcome
        let action = reconcile_ambiguous(&mut connector, state).await.unwrap();

        // THEN safe resolution chosen: Commit (server confirms effect was applied)
        assert_eq!(action, ReconcileAction::Commit);
    }

    #[tokio::test]
    async fn given_ambiguous_commit_when_reconcile_resolves_not_committed_then_safe_rollback() {
        // GIVEN connector in Ambiguous state
        let state = ConnectorState::Ambiguous;
        let mut connector = AmbiguousOnCommitConnector::new(ReconciliationResult::NotCommitted);

        // WHEN reconcile determines effect was NOT committed
        let action = reconcile_ambiguous(&mut connector, state).await.unwrap();

        // THEN safe resolution: Rollback (no double-commit risk)
        assert_eq!(action, ReconcileAction::Rollback);
    }

    #[tokio::test]
    async fn given_repeatedly_ambiguous_commit_when_max_retries_exceeded_then_error() {
        use crate::connector::execute_with_reconciliation;

        // GIVEN connector that always returns Ambiguous and Unknown on reconcile
        let mut connector = AmbiguousOnCommitConnector::new(ReconciliationResult::Unknown);

        // WHEN execute_with_reconciliation called with max_retries=2
        let result = execute_with_reconciliation(&mut connector, false, 2).await;

        // THEN MaxRetriesExceeded error returned (safe failure, not silent data loss)
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ConnectorError::MaxRetriesExceeded { max_retries: 2 }));
    }

    #[tokio::test]
    async fn given_ambiguous_then_retry_then_succeed_full_lifecycle() {
        use crate::connector::execute_with_reconciliation;

        // GIVEN connector that is ambiguous on first commit, reconcile says Unknown (retry),
        // then succeeds on second commit attempt
        struct RetryThenSucceedConnector {
            attempt: std::sync::atomic::AtomicU32,
        }

        impl Connector for RetryThenSucceedConnector {
            async fn prepare(&mut self) -> Result<ConnectorResult, ConnectorError> {
                Ok(ConnectorResult::Success)
            }

            async fn commit(&mut self) -> Result<ConnectorResult, ConnectorError> {
                let attempt = self.attempt.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if attempt == 0 {
                    Ok(ConnectorResult::Ambiguous)
                } else {
                    Ok(ConnectorResult::Success)
                }
            }

            async fn reconcile(&mut self) -> Result<ReconciliationResult, ConnectorError> {
                Ok(ReconciliationResult::Unknown)
            }

            async fn rollback(&mut self) -> Result<ConnectorResult, ConnectorError> {
                Ok(ConnectorResult::Success)
            }
        }

        let mut connector = RetryThenSucceedConnector {
            attempt: std::sync::atomic::AtomicU32::new(0),
        };

        // WHEN execute_with_reconciliation runs (prepare + commit with ambiguity handling)
        let result = execute_with_reconciliation(&mut connector, true, 3).await.unwrap();

        // THEN ultimately succeeds after reconciliation returned Unknown (retry) then commit succeeded
        assert_eq!(result, ConnectorResult::Success);
    }
}

#[cfg(test)]
mod bdd_state_transition_invariants {
    //! BDD Scenario: State transition invariants enforcement.
    //!
    //! INV-C02: Transitions follow ADR-041 durability sequence strictly.
    //! INV-C03: Terminal states reject all transitions.
    //! INV-C04: Ambiguous only transitions via reconciliation events.

    use crate::connector::{
        apply_connector_transition, ConnectorState, ConnectorTransition, ConnectorTransitionError,
    };

    #[tokio::test]
    async fn given_idle_when_non_prepare_transition_then_rejected() {
        // GIVEN connector in Idle state
        let state = ConnectorState::Idle;

        // WHEN any transition other than Prepare is applied
        let invalid_events = [
            ConnectorTransition::Prepared,
            ConnectorTransition::Commit,
            ConnectorTransition::Succeed,
            ConnectorTransition::Fail,
            ConnectorTransition::Ambiguate,
            ConnectorTransition::ReconcileSucceeded,
            ConnectorTransition::ReconcileFailed,
            ConnectorTransition::ReconcileRetry,
        ];

        // THEN all are rejected as InvalidTransition
        for event in invalid_events {
            let result = apply_connector_transition(state, event);
            assert_eq!(
                result,
                Err(ConnectorTransitionError::InvalidTransition),
                "Idle + {:?} should be invalid",
                event
            );
        }
    }

    #[tokio::test]
    async fn given_succeeded_when_any_transition_then_terminal_error() {
        // GIVEN connector in Succeeded state (terminal)
        let state = ConnectorState::Succeeded;

        // WHEN any transition is applied
        for event in ConnectorTransition::all_variants() {
            let result = apply_connector_transition(state, *event);
            assert_eq!(
                result,
                Err(ConnectorTransitionError::TerminalStateTransition),
                "Succeeded + {:?} should be TerminalStateTransition",
                event
            );
        }
    }

    #[tokio::test]
    async fn given_failed_when_any_transition_then_terminal_error() {
        // GIVEN connector in Failed state (terminal)
        let state = ConnectorState::Failed;

        // WHEN any transition is applied
        for event in ConnectorTransition::all_variants() {
            let result = apply_connector_transition(state, *event);
            assert_eq!(
                result,
                Err(ConnectorTransitionError::TerminalStateTransition),
                "Failed + {:?} should be TerminalStateTransition",
                event
            );
        }
    }

    #[tokio::test]
    async fn given_ambiguous_when_non_reconcile_transition_then_rejected() {
        // GIVEN connector in Ambiguous state (INV-C04)
        let state = ConnectorState::Ambiguous;

        // WHEN non-reconciliation transitions are applied
        let non_reconcile_events = [
            ConnectorTransition::Prepare,
            ConnectorTransition::Prepared,
            ConnectorTransition::Commit,
            ConnectorTransition::Succeed,
            ConnectorTransition::Fail,
            ConnectorTransition::Ambiguate,
        ];

        // THEN all are rejected
        for event in non_reconcile_events {
            let result = apply_connector_transition(state, event);
            assert_eq!(
                result,
                Err(ConnectorTransitionError::InvalidTransition),
                "Ambiguous + {:?} should be invalid (INV-C04)",
                event
            );
        }
    }

    #[tokio::test]
    async fn given_ambiguous_when_reconcile_events_then_valid_transitions() {
        // GIVEN connector in Ambiguous state
        let state = ConnectorState::Ambiguous;

        // WHEN ReconcileSucceeded applied THEN transitions to Succeeded
        let result =
            apply_connector_transition(state, ConnectorTransition::ReconcileSucceeded).unwrap();
        assert_eq!(result, ConnectorState::Succeeded);
        assert!(result.is_terminal());

        // WHEN ReconcileFailed applied THEN transitions to Failed
        let result =
            apply_connector_transition(state, ConnectorTransition::ReconcileFailed).unwrap();
        assert_eq!(result, ConnectorState::Failed);
        assert!(result.is_terminal());

        // WHEN ReconcileRetry applied THEN transitions to Prepared (for retry)
        let result = apply_connector_transition(state, ConnectorTransition::ReconcileRetry).unwrap();
        assert_eq!(result, ConnectorState::Prepared);
        assert!(!result.is_terminal());
    }
}
