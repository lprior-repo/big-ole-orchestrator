//! Integration tests for the connector module.
//!
//! Tests the ConnectorOrchestrator with real async timeouts and reconciliation.
//! These tests exercise the full async behavior including tokio::time::timeout.

use std::time::Duration;
use tokio::time::sleep;
use vo_core::connector::ConnectorOrchestrator;
use vo_types::connector::{Connector, ConnectorError, ConnectorResult, ReconciliationResult};

struct SlowConnector {
    prepare_duration: Duration,
    commit_duration: Duration,
    commit_result: ConnectorResult,
    reconcile_result: ReconciliationResult,
    prepare_sleep: bool,
    commit_sleep: bool,
}

impl SlowConnector {
    fn new(
        prepare_duration: Duration,
        commit_duration: Duration,
        commit_result: ConnectorResult,
        reconcile_result: ReconciliationResult,
    ) -> Self {
        Self {
            prepare_duration,
            commit_duration,
            commit_result,
            reconcile_result,
            prepare_sleep: false,
            commit_sleep: false,
        }
    }

    fn with_prepare_sleep(mut self) -> Self {
        self.prepare_sleep = true;
        self
    }

    fn with_commit_sleep(mut self) -> Self {
        self.commit_sleep = true;
        self
    }
}

impl Connector for SlowConnector {
    async fn prepare(&mut self) -> Result<ConnectorResult, ConnectorError> {
        if self.prepare_sleep {
            sleep(self.prepare_duration).await;
        }
        Ok(ConnectorResult::Success)
    }

    async fn commit(&mut self) -> Result<ConnectorResult, ConnectorError> {
        if self.commit_sleep {
            sleep(self.commit_duration).await;
        }
        Ok(self.commit_result)
    }

    async fn reconcile(&mut self) -> Result<ReconciliationResult, ConnectorError> {
        Ok(self.reconcile_result)
    }

    async fn rollback(&mut self) -> Result<ConnectorResult, ConnectorError> {
        Ok(ConnectorResult::Success)
    }
}

struct CommitOnlyConnector {
    result: ConnectorResult,
}

impl CommitOnlyConnector {
    fn new(result: ConnectorResult) -> Self {
        Self { result }
    }
}

impl Connector for CommitOnlyConnector {
    async fn prepare(&mut self) -> Result<ConnectorResult, ConnectorError> {
        Ok(ConnectorResult::Success)
    }

    async fn commit(&mut self) -> Result<ConnectorResult, ConnectorError> {
        Ok(self.result)
    }

    async fn reconcile(&mut self) -> Result<ReconciliationResult, ConnectorError> {
        Ok(ReconciliationResult::Unknown)
    }

    async fn rollback(&mut self) -> Result<ConnectorResult, ConnectorError> {
        Ok(ConnectorResult::Success)
    }
}

mod timeout_handling {
    use super::*;

    #[tokio::test]
    async fn execute_succeeds_when_commit_succeeds_quickly() {
        let mut orchestrator = ConnectorOrchestrator::with_timeout(
            CommitOnlyConnector::new(ConnectorResult::Success),
            Duration::from_secs(30),
        );

        let result = orchestrator
            .execute()
            .await
            .expect("execute should succeed");
        assert_eq!(result, ConnectorResult::Success);
    }

    #[tokio::test]
    async fn execute_fails_when_commit_fails_quickly() {
        let mut orchestrator = ConnectorOrchestrator::with_timeout(
            CommitOnlyConnector::new(ConnectorResult::Failure),
            Duration::from_secs(30),
        );

        let result = orchestrator
            .execute()
            .await
            .expect("execute should succeed");
        assert_eq!(result, ConnectorResult::Failure);
    }

    #[tokio::test]
    async fn execute_reconciles_when_commit_returns_ambiguous() {
        let mut orchestrator = ConnectorOrchestrator::with_timeout(
            CommitOnlyConnector::new(ConnectorResult::Ambiguous),
            Duration::from_secs(30),
        );

        let result = orchestrator
            .execute()
            .await
            .expect("execute should succeed");
        assert_eq!(result, ConnectorResult::Ambiguous);
    }
}

mod reconciliation_loop {
    use super::*;

    struct AmbiguousReconcileConnector {
        reconcile_result: ReconciliationResult,
    }

    impl AmbiguousReconcileConnector {
        fn new(reconcile_result: ReconciliationResult) -> Self {
            Self { reconcile_result }
        }
    }

    impl Connector for AmbiguousReconcileConnector {
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
    async fn execute_returns_success_when_reconcile_reports_committed() {
        let mut orchestrator = ConnectorOrchestrator::with_timeout(
            AmbiguousReconcileConnector::new(ReconciliationResult::Committed),
            Duration::from_secs(30),
        );

        let result = orchestrator
            .execute()
            .await
            .expect("execute should succeed");
        assert_eq!(result, ConnectorResult::Success);
    }

    #[tokio::test]
    async fn execute_returns_failure_when_reconcile_reports_not_committed() {
        let mut orchestrator = ConnectorOrchestrator::with_timeout(
            AmbiguousReconcileConnector::new(ReconciliationResult::NotCommitted),
            Duration::from_secs(30),
        );

        let result = orchestrator
            .execute()
            .await
            .expect("execute should succeed");
        assert_eq!(result, ConnectorResult::Failure);
    }

    #[tokio::test]
    async fn execute_returns_ambiguous_when_reconcile_reports_unknown() {
        let mut orchestrator = ConnectorOrchestrator::with_timeout(
            AmbiguousReconcileConnector::new(ReconciliationResult::Unknown),
            Duration::from_secs(30),
        );

        let result = orchestrator
            .execute()
            .await
            .expect("execute should succeed");
        assert_eq!(result, ConnectorResult::Ambiguous);
    }
}

mod prepare_timeout {
    use super::*;

    #[tokio::test]
    async fn prepare_timeout_returns_ambiguous() {
        let mut orchestrator = ConnectorOrchestrator::with_timeout(
            SlowConnector::new(
                Duration::from_secs(5),
                Duration::from_secs(0),
                ConnectorResult::Success,
                ReconciliationResult::Unknown,
            )
            .with_prepare_sleep(),
            Duration::from_millis(100),
        );

        let result = orchestrator
            .execute()
            .await
            .expect("execute should succeed");
        assert_eq!(result, ConnectorResult::Ambiguous);
    }
}

mod commit_timeout {
    use super::*;

    #[tokio::test]
    async fn commit_timeout_returns_ambiguous_and_reconciles() {
        let mut orchestrator = ConnectorOrchestrator::with_timeout(
            SlowConnector::new(
                Duration::from_secs(0),
                Duration::from_secs(5),
                ConnectorResult::Success,
                ReconciliationResult::Committed,
            )
            .with_commit_sleep(),
            Duration::from_millis(100),
        );

        let result = orchestrator
            .execute()
            .await
            .expect("execute should succeed");
        assert_eq!(result, ConnectorResult::Success);
    }
}

mod custom_timeout {
    use super::*;

    #[tokio::test]
    async fn custom_timeout_allows_slower_operations() {
        let mut orchestrator = ConnectorOrchestrator::with_timeout(
            SlowConnector::new(
                Duration::from_secs(0),
                Duration::from_secs(1),
                ConnectorResult::Success,
                ReconciliationResult::Unknown,
            )
            .with_commit_sleep(),
            Duration::from_secs(5),
        );

        let result = orchestrator
            .execute()
            .await
            .expect("execute should succeed");
        assert_eq!(result, ConnectorResult::Success);
    }

    #[tokio::test]
    async fn short_timeout_fires_on_slow_commit() {
        let mut orchestrator = ConnectorOrchestrator::with_timeout(
            SlowConnector::new(
                Duration::from_secs(0),
                Duration::from_secs(5),
                ConnectorResult::Success,
                ReconciliationResult::NotCommitted,
            )
            .with_commit_sleep(),
            Duration::from_millis(100),
        );

        let result = orchestrator
            .execute()
            .await
            .expect("execute should succeed");
        assert_eq!(result, ConnectorResult::Failure);
    }
}

mod orchestrator_methods {
    use super::*;

    #[tokio::test]
    async fn prepare_with_timeout_success() {
        let mut orchestrator = ConnectorOrchestrator::with_timeout(
            CommitOnlyConnector::new(ConnectorResult::Success),
            Duration::from_secs(30),
        );

        let result = orchestrator
            .prepare_with_timeout()
            .await
            .expect("prepare_with_timeout should succeed");
        assert_eq!(result, ConnectorResult::Success);
    }

    #[tokio::test]
    async fn prepare_with_timeout_returns_ambiguous_on_timeout() {
        let mut orchestrator = ConnectorOrchestrator::with_timeout(
            SlowConnector::new(
                Duration::from_secs(5),
                Duration::from_secs(0),
                ConnectorResult::Success,
                ReconciliationResult::Unknown,
            )
            .with_prepare_sleep(),
            Duration::from_millis(100),
        );

        let result = orchestrator
            .prepare_with_timeout()
            .await
            .expect("prepare_with_timeout should succeed");
        assert_eq!(result, ConnectorResult::Ambiguous);
    }

    #[tokio::test]
    async fn commit_with_timeout_success() {
        let mut orchestrator = ConnectorOrchestrator::with_timeout(
            CommitOnlyConnector::new(ConnectorResult::Success),
            Duration::from_secs(30),
        );

        let result = orchestrator
            .commit_with_timeout()
            .await
            .expect("commit_with_timeout should succeed");
        assert_eq!(result, ConnectorResult::Success);
    }

    #[tokio::test]
    async fn commit_with_timeout_returns_ambiguous_on_timeout() {
        let mut orchestrator = ConnectorOrchestrator::with_timeout(
            SlowConnector::new(
                Duration::from_secs(0),
                Duration::from_secs(5),
                ConnectorResult::Success,
                ReconciliationResult::Unknown,
            )
            .with_commit_sleep(),
            Duration::from_millis(100),
        );

        let result = orchestrator
            .commit_with_timeout()
            .await
            .expect("commit_with_timeout should succeed");
        assert_eq!(result, ConnectorResult::Ambiguous);
    }

    #[tokio::test]
    async fn into_inner_returns_connector() {
        let connector = CommitOnlyConnector::new(ConnectorResult::Success);
        let orchestrator = ConnectorOrchestrator::with_timeout(connector, Duration::from_secs(30));

        let _inner = orchestrator.into_inner();
    }

    #[tokio::test]
    async fn connector_reference_access() {
        let mut orchestrator = ConnectorOrchestrator::with_timeout(
            CommitOnlyConnector::new(ConnectorResult::Success),
            Duration::from_secs(30),
        );

        let _connector = orchestrator.connector();
    }
}
