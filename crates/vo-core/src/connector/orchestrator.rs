//! Connector orchestrator with timeout detection and reconciliation (ADR-041).
//!
//! This module provides the orchestrator that wraps connector operations with
//! timeout detection using `tokio::time::timeout`. When a connector operation
//! times out or returns ambiguous, the orchestrator handles the reconciliation
//! loop to determine the true outcome before retrying.
//!
//! # Timeout Model (ADR-041 §3)
//!
//! - A connector timeout does NOT mean the effect failed.
//! - On timeout or transport ambiguity, the orchestrator records an ambiguous state
//!   and calls reconcile before any retry.
//! - Retrying commit without reconciliation is forbidden unless the connector
//!   contract explicitly proves it is safe.
//!
//! # Reconciliation Loop
//!
//! When a commit returns `Ambiguous` or times out:
//! 1. Transition to `Ambiguous` state
//! 2. Call `Connector::reconcile()` to query server state
//! 3. Based on `ReconciliationResult`:
//!    - `Committed` → transition to `Succeeded`, return `Success`
//!    - `NotCommitted` → transition to `Failed`, return `Failure`
//!    - `Unknown` → transition to `Prepared`, return `Ambiguous` for retry

use std::time::Duration;
use tokio::time::timeout;
use vo_types::{
    reconcile_ambiguous, Connector, ConnectorError, ConnectorResult, ConnectorState,
    ReconcileAction,
};

/// Default timeout for connector operations.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Orchestrator for managing connector operations with timeout detection.
///
/// Wraps connector operations with `tokio::time::timeout` and handles
/// ambiguity through the reconciliation loop defined in ADR-041.
#[derive(Debug)]
pub struct ConnectorOrchestrator<C: Connector> {
    connector: C,
    timeout_duration: Duration,
}

impl<C: Connector> ConnectorOrchestrator<C> {
    /// Create a new orchestrator with the given connector and default timeout.
    #[must_use]
    pub fn new(connector: C) -> Self {
        Self {
            connector,
            timeout_duration: DEFAULT_TIMEOUT,
        }
    }

    /// Create a new orchestrator with a custom timeout duration.
    #[must_use]
    pub fn with_timeout(connector: C, timeout_duration: Duration) -> Self {
        Self {
            connector,
            timeout_duration,
        }
    }

    /// Execute the prepare step with timeout detection.
    ///
    /// Returns `Ok(ConnectorResult)` on success or timeout (which is treated as ambiguous).
    pub async fn prepare_with_timeout(&mut self) -> Result<ConnectorResult, ConnectorError> {
        let result = timeout(self.timeout_duration, self.connector.prepare()).await;

        match result {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(e)) => Err(e),
            Err(_) => Ok(ConnectorResult::Ambiguous),
        }
    }

    /// Execute the commit step with timeout detection.
    ///
    /// Returns `Ok(ConnectorResult)` on success or timeout (which is treated as ambiguous).
    /// When `Ambiguous` is returned, the caller should invoke `reconcile`.
    pub async fn commit_with_timeout(&mut self) -> Result<ConnectorResult, ConnectorError> {
        let result = timeout(self.timeout_duration, self.connector.commit()).await;

        match result {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(e)) => Err(e),
            Err(_) => Ok(ConnectorResult::Ambiguous),
        }
    }

    /// Execute the full prepare → commit sequence with timeout and ambiguity handling.
    ///
    /// This is the main entry point for executing connector operations.
    /// It handles:
    /// 1. Prepare with timeout
    /// 2. If prepare succeeds, commit with timeout
    /// 3. If commit returns Ambiguous (timeout or explicit), run reconciliation
    /// 4. Return the final result
    ///
    /// # Returns
    ///
    /// * `Ok(ConnectorResult::Success)` - Operation completed successfully
    /// * `Ok(ConnectorResult::Failure)` - Operation failed unambiguously
    /// * `Ok(ConnectorResult::Ambiguous)` - Outcome unclear, retry needed
    /// * `Err(ConnectorError)` - Error during operation
    pub async fn execute(&mut self) -> Result<ConnectorResult, ConnectorError> {
        self.prepare_with_timeout().await?;

        let commit_result = self.commit_with_timeout().await?;

        match commit_result {
            ConnectorResult::Success => Ok(ConnectorResult::Success),
            ConnectorResult::Failure => Ok(ConnectorResult::Failure),
            ConnectorResult::Ambiguous => self.handle_ambiguous().await,
        }
    }

    /// Handle an ambiguous result by running the reconciliation loop.
    ///
    /// # Returns
    ///
    /// * `Ok(ConnectorResult::Success)` - Reconciliation determined committed
    /// * `Ok(ConnectorResult::Failure)` - Reconciliation determined not committed
    /// * `Ok(ConnectorResult::Ambiguous)` - Reconciliation could not determine, retry needed
    async fn handle_ambiguous(&mut self) -> Result<ConnectorResult, ConnectorError> {
        let action = reconcile_ambiguous(&mut self.connector, ConnectorState::Ambiguous).await?;

        match action {
            ReconcileAction::Commit => Ok(ConnectorResult::Success),
            ReconcileAction::Rollback => Ok(ConnectorResult::Failure),
            ReconcileAction::Retry => Ok(ConnectorResult::Ambiguous),
        }
    }

    /// Get a reference to the underlying connector.
    pub fn connector(&self) -> &C {
        &self.connector
    }

    /// Get a mutable reference to the underlying connector.
    pub fn connector_mut(&mut self) -> &mut C {
        &mut self.connector
    }
}

impl<C: Connector> ConnectorOrchestrator<C> {
    /// Consume the orchestrator and return the inner connector.
    #[must_use]
    pub fn into_inner(self) -> C {
        self.connector
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vo_types::{Connector, ConnectorError, ReconciliationResult};

    struct TestConnector {
        commit_result: ConnectorResult,
        reconcile_result: ReconciliationResult,
        prepare_called: bool,
        commit_called: bool,
        reconcile_called: bool,
    }

    impl TestConnector {
        fn new(commit_result: ConnectorResult, reconcile_result: ReconciliationResult) -> Self {
            Self {
                commit_result,
                reconcile_result,
                prepare_called: false,
                commit_called: false,
                reconcile_called: false,
            }
        }
    }

    impl Connector for TestConnector {
        async fn prepare(&mut self) -> Result<ConnectorResult, ConnectorError> {
            self.prepare_called = true;
            Ok(ConnectorResult::Success)
        }

        async fn commit(&mut self) -> Result<ConnectorResult, ConnectorError> {
            self.commit_called = true;
            Ok(self.commit_result)
        }

        async fn reconcile(&mut self) -> Result<ReconciliationResult, ConnectorError> {
            self.reconcile_called = true;
            Ok(self.reconcile_result)
        }

        async fn rollback(&mut self) -> Result<ConnectorResult, ConnectorError> {
            Ok(ConnectorResult::Success)
        }
    }

    #[tokio::test]
    async fn execute_returns_success_when_commit_succeeds() {
        let mut orchestrator = ConnectorOrchestrator::new(TestConnector::new(
            ConnectorResult::Success,
            ReconciliationResult::Unknown,
        ));

        let result = orchestrator.execute().await.unwrap();
        assert_eq!(result, ConnectorResult::Success);
    }

    #[tokio::test]
    async fn execute_returns_failure_when_commit_fails() {
        let mut orchestrator = ConnectorOrchestrator::new(TestConnector::new(
            ConnectorResult::Failure,
            ReconciliationResult::Unknown,
        ));

        let result = orchestrator.execute().await.unwrap();
        assert_eq!(result, ConnectorResult::Failure);
    }

    #[tokio::test]
    async fn execute_reconciles_when_commit_returns_ambiguous() {
        let mut orchestrator = ConnectorOrchestrator::new(TestConnector::new(
            ConnectorResult::Ambiguous,
            ReconciliationResult::Committed,
        ));

        let result = orchestrator.execute().await.unwrap();
        assert_eq!(result, ConnectorResult::Success);
    }

    #[tokio::test]
    async fn execute_returns_failure_when_reconcile_determines_not_committed() {
        let mut orchestrator = ConnectorOrchestrator::new(TestConnector::new(
            ConnectorResult::Ambiguous,
            ReconciliationResult::NotCommitted,
        ));

        let result = orchestrator.execute().await.unwrap();
        assert_eq!(result, ConnectorResult::Failure);
    }

    #[tokio::test]
    async fn execute_returns_ambiguous_when_reconcile_returns_unknown() {
        let mut orchestrator = ConnectorOrchestrator::new(TestConnector::new(
            ConnectorResult::Ambiguous,
            ReconciliationResult::Unknown,
        ));

        let result = orchestrator.execute().await.unwrap();
        assert_eq!(result, ConnectorResult::Ambiguous);
    }

    #[tokio::test]
    async fn execute_calls_prepare_and_commit() {
        let mut orchestrator = ConnectorOrchestrator::new(TestConnector::new(
            ConnectorResult::Success,
            ReconciliationResult::Unknown,
        ));

        orchestrator.execute().await.unwrap();

        let connector = orchestrator.into_inner();
        assert!(connector.prepare_called);
        assert!(connector.commit_called);
    }
}
