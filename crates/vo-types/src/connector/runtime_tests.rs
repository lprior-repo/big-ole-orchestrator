use super::*;

struct MockConnector {
    reconcile_result: ReconciliationResult,
}

impl MockConnector {
    fn new(reconcile_result: ReconciliationResult) -> Self {
        Self { reconcile_result }
    }
}

impl Connector for MockConnector {
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
async fn reconcile_ambiguous_returns_commit_when_server_committed() {
    let mut connector = MockConnector::new(ReconciliationResult::Committed);
    let action = reconcile_ambiguous(&mut connector, ConnectorState::Ambiguous)
        .await
        .unwrap();
    assert_eq!(action, ReconcileAction::Commit);
}

#[tokio::test]
async fn reconcile_ambiguous_returns_rollback_when_server_not_committed() {
    let mut connector = MockConnector::new(ReconciliationResult::NotCommitted);
    let action = reconcile_ambiguous(&mut connector, ConnectorState::Ambiguous)
        .await
        .unwrap();
    assert_eq!(action, ReconcileAction::Rollback);
}

#[tokio::test]
async fn reconcile_ambiguous_returns_retry_when_outcome_unknown() {
    let mut connector = MockConnector::new(ReconciliationResult::Unknown);
    let action = reconcile_ambiguous(&mut connector, ConnectorState::Ambiguous)
        .await
        .unwrap();
    assert_eq!(action, ReconcileAction::Retry);
}

#[tokio::test]
async fn execute_with_reconciliation_commits_on_success() {
    struct SuccessConnector;
    impl Connector for SuccessConnector {
        async fn prepare(&mut self) -> Result<ConnectorResult, ConnectorError> {
            Ok(ConnectorResult::Success)
        }
        async fn commit(&mut self) -> Result<ConnectorResult, ConnectorError> {
            Ok(ConnectorResult::Success)
        }
        async fn reconcile(&mut self) -> Result<ReconciliationResult, ConnectorError> {
            Ok(ReconciliationResult::Unknown)
        }
        async fn rollback(&mut self) -> Result<ConnectorResult, ConnectorError> {
            Ok(ConnectorResult::Success)
        }
    }

    let mut connector = SuccessConnector;
<<<<<<< HEAD
    let result = execute_with_reconciliation(&mut connector, true, 3).await.unwrap();
=======
    let result = execute_with_reconciliation(&mut connector, true, 0)
        .await
        .unwrap();
>>>>>>> 7e356012 (style: apply consistent rustfmt formatting)
    assert_eq!(result, ConnectorResult::Success);
}

#[tokio::test]
async fn execute_with_reconciliation_resolves_ambiguous() {
    let mut connector = MockConnector::new(ReconciliationResult::Committed);
<<<<<<< HEAD
    let result = execute_with_reconciliation(&mut connector, false, 3).await.unwrap();
=======
    let result = execute_with_reconciliation(&mut connector, false, 0)
        .await
        .unwrap();
>>>>>>> 7e356012 (style: apply consistent rustfmt formatting)
    assert_eq!(result, ConnectorResult::Success);
}

#[tokio::test]
#[should_panic(expected = "reconcile_ambiguous called with non-Ambiguous state")]
async fn reconcile_ambiguous_panics_on_non_ambiguous_state() {
    struct DummyConnector;
    impl Connector for DummyConnector {
        async fn prepare(&mut self) -> Result<ConnectorResult, ConnectorError> {
            Ok(ConnectorResult::Success)
        }
        async fn commit(&mut self) -> Result<ConnectorResult, ConnectorError> {
            Ok(ConnectorResult::Success)
        }
        async fn reconcile(&mut self) -> Result<ReconciliationResult, ConnectorError> {
            Ok(ReconciliationResult::Unknown)
        }
        async fn rollback(&mut self) -> Result<ConnectorResult, ConnectorError> {
            Ok(ConnectorResult::Success)
        }
    }

    let mut connector = DummyConnector;
<<<<<<< HEAD
    let _ = reconcile_ambiguous(&mut connector, ConnectorState::Executing).await;
=======
    let result = reconcile_ambiguous(&mut connector, ConnectorState::Executing).await;
    assert!(
        result.is_err(),
        "reconcile_ambiguous on non-Ambiguous state should return Err"
    );
    match result {
        Err(ConnectorError::InvalidState { current, expected }) => {
            assert_eq!(current, ConnectorState::Executing);
            assert!(expected.contains(&ConnectorState::Ambiguous));
        }
        other => panic!("Expected InvalidState error, got {:?}", other),
    }
>>>>>>> 7e356012 (style: apply consistent rustfmt formatting)
}
