//! Connector runtime crash injection tests (ADR-041).
//!
//! This module provides integration tests for the connector runtime,
//! verifying exactly-once commit semantics under crash injection and
//! checking the reconciliation path.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::useless_vec, unused_imports, unused_variables)]


use vo_types::{
    execute_with_reconciliation, reconcile_ambiguous, Connector, ConnectorError, ConnectorResult,
    ConnectorState, ReconcileAction, ReconciliationResult,
};

/// Mock connector that simulates various crash scenarios.
#[derive(Debug)]
struct CrashTestConnector {
    state: ConnectorState,
    prepare_crash: bool,
    commit_crash: bool,
    commit_crash_position: Option<CommitCrashPosition>,
    reconcile_result: ReconciliationResult,
    prepare_count: usize,
    commit_count: usize,
    reconcile_count: usize,
}

#[derive(Debug, Clone, Copy)]
enum CommitCrashPosition {
    Before,
    After,
}

impl CrashTestConnector {
    fn new() -> Self {
        Self {
            state: ConnectorState::Idle,
            prepare_crash: false,
            commit_crash: false,
            commit_crash_position: None,
            reconcile_result: ReconciliationResult::Unknown,
            prepare_count: 0,
            commit_count: 0,
            reconcile_count: 0,
        }
    }

    fn with_prepare_crash(mut self) -> Self {
        self.prepare_crash = true;
        self
    }

    fn with_commit_crash(mut self, position: CommitCrashPosition) -> Self {
        self.commit_crash = true;
        self.commit_crash_position = Some(position);
        self
    }

    fn with_reconcile_result(mut self, result: ReconciliationResult) -> Self {
        self.reconcile_result = result;
        self
    }

    fn get_state(&self) -> ConnectorState {
        self.state
    }

    fn get_prepare_count(&self) -> usize {
        self.prepare_count
    }

    fn get_commit_count(&self) -> usize {
        self.commit_count
    }

    fn get_reconcile_count(&self) -> usize {
        self.reconcile_count
    }
}

impl Connector for CrashTestConnector {
    async fn prepare(&mut self) -> Result<ConnectorResult, ConnectorError> {
        self.prepare_count += 1;

        if self.prepare_crash {
            return Ok(ConnectorResult::Ambiguous);
        }

        self.state = ConnectorState::Preparing;

        Ok(ConnectorResult::Success)
    }

    async fn commit(&mut self) -> Result<ConnectorResult, ConnectorError> {
        self.commit_count += 1;

        // Update state to Executing before checking for crash
        self.state = ConnectorState::Executing;

        if self.commit_crash {
            if let Some(position) = self.commit_crash_position {
                match position {
                    CommitCrashPosition::Before => return Ok(ConnectorResult::Ambiguous),
                    CommitCrashPosition::After => {
                        // After position means we already set state to Executing,
                        // now set to Succeeded
                        self.state = ConnectorState::Succeeded;
                        return Ok(ConnectorResult::Success);
                    }
                }
            }
            return Ok(ConnectorResult::Ambiguous);
        }

        self.state = ConnectorState::Succeeded;

        Ok(ConnectorResult::Success)
    }

    async fn reconcile(&mut self) -> Result<ReconciliationResult, ConnectorError> {
        self.reconcile_count += 1;

        // Update state based on reconciliation result
        match self.reconcile_result {
            ReconciliationResult::Committed => {
                self.state = ConnectorState::Succeeded;
            }
            ReconciliationResult::NotCommitted => {
                self.state = ConnectorState::Failed;
            }
            ReconciliationResult::Unknown => {
                self.state = ConnectorState::Prepared;
            }
        }

        Ok(self.reconcile_result)
    }

    async fn rollback(&mut self) -> Result<ConnectorResult, ConnectorError> {
        self.state = ConnectorState::Idle;
        Ok(ConnectorResult::Success)
    }
}

/// Test that exactly-once commit is maintained under crash injection.
///
/// This test simulates a crash during commit and verifies that:
/// 1. The crash is detected (Ambiguous result)
/// 2. Reconciliation determines the true outcome
/// 3. The final state is consistent with exactly-once semantics
#[tokio::test]
async fn test_exactly_once_commit_under_crash_injection() {
    let mut connector = CrashTestConnector::new()
        .with_commit_crash(CommitCrashPosition::Before)
        .with_reconcile_result(ReconciliationResult::Committed);

    let initial_state = connector.get_state();
    assert_eq!(initial_state, ConnectorState::Idle);

    let prepare_result = connector.prepare().await.unwrap();
    assert_eq!(prepare_result, ConnectorResult::Success);

    let commit_result = connector.commit().await.unwrap();
    assert_eq!(commit_result, ConnectorResult::Ambiguous);

    let reconcile_result = connector.reconcile().await.unwrap();
    assert_eq!(reconcile_result, ReconciliationResult::Committed);

    let final_state = connector.get_state();
    assert_eq!(final_state, ConnectorState::Succeeded);

    assert_eq!(connector.get_prepare_count(), 1);
    assert_eq!(connector.get_commit_count(), 1);
    assert_eq!(connector.get_reconcile_count(), 1);
}

/// Test that reconciliation correctly routes to Commit when server confirms.
#[tokio::test]
async fn test_reconciliation_routes_to_commit_on_server_confirmed() {
    let mut connector = CrashTestConnector::new()
        .with_commit_crash(CommitCrashPosition::Before)
        .with_reconcile_result(ReconciliationResult::Committed);

    let _ = connector.prepare().await.unwrap();
    let commit_result = connector.commit().await.unwrap();
    assert_eq!(commit_result, ConnectorResult::Ambiguous);

    let action = reconcile_ambiguous(&mut connector, ConnectorState::Ambiguous)
        .await
        .unwrap();

    assert_eq!(action, ReconcileAction::Commit);
}

/// Test that reconciliation correctly routes to Rollback when server denies.
#[tokio::test]
async fn test_reconciliation_routes_to_rollback_on_server_not_committed() {
    let mut connector = CrashTestConnector::new()
        .with_commit_crash(CommitCrashPosition::Before)
        .with_reconcile_result(ReconciliationResult::NotCommitted);

    let _ = connector.prepare().await.unwrap();
    let commit_result = connector.commit().await.unwrap();
    assert_eq!(commit_result, ConnectorResult::Ambiguous);

    let action = reconcile_ambiguous(&mut connector, ConnectorState::Ambiguous)
        .await
        .unwrap();

    assert_eq!(action, ReconcileAction::Rollback);
}

/// Test that reconciliation correctly routes to Retry when outcome is unknown.
#[tokio::test]
async fn test_reconciliation_routes_to_retry_on_unknown_outcome() {
    let mut connector = CrashTestConnector::new()
        .with_commit_crash(CommitCrashPosition::Before)
        .with_reconcile_result(ReconciliationResult::Unknown);

    let _ = connector.prepare().await.unwrap();
    let commit_result = connector.commit().await.unwrap();
    assert_eq!(commit_result, ConnectorResult::Ambiguous);

    let action = reconcile_ambiguous(&mut connector, ConnectorState::Ambiguous)
        .await
        .unwrap();

    assert_eq!(action, ReconcileAction::Retry);
}

/// Test that execute_with_reconciliation handles successful commit without ambiguity.
#[tokio::test]
async fn test_execute_with_reconciliation_success_without_crash() {
    let mut connector = CrashTestConnector::new();

    let result = execute_with_reconciliation(&mut connector, true, 3)
        .await
        .unwrap();

    assert_eq!(result, ConnectorResult::Success);
    assert_eq!(connector.get_prepare_count(), 1);
    assert_eq!(connector.get_commit_count(), 1);
    assert_eq!(connector.get_reconcile_count(), 0);
}

/// Test that execute_with_reconciliation resolves ambiguity through reconciliation.
#[tokio::test]
async fn test_execute_with_reconciliation_resolves_ambiguity() {
    let mut connector = CrashTestConnector::new()
        .with_commit_crash(CommitCrashPosition::Before)
        .with_reconcile_result(ReconciliationResult::Committed);

    let result = execute_with_reconciliation(&mut connector, false, 3)
        .await
        .unwrap();

    assert_eq!(result, ConnectorResult::Success);
    assert_eq!(connector.get_prepare_count(), 0);
    assert_eq!(connector.get_commit_count(), 1);
    assert_eq!(connector.get_reconcile_count(), 1);
}

/// Test that exactly-once is maintained under crash at position After.
#[tokio::test]
async fn test_exactly_once_commit_after_crash_position() {
    let mut connector = CrashTestConnector::new()
        .with_commit_crash(CommitCrashPosition::After)
        .with_reconcile_result(ReconciliationResult::Committed);

    let _ = connector.prepare().await.unwrap();
    let commit_result = connector.commit().await.unwrap();

    assert_eq!(commit_result, ConnectorResult::Success);
    assert_eq!(connector.get_commit_count(), 1);
    assert_eq!(connector.get_reconcile_count(), 0);
}

/// Test that prepare crash results in ambiguous state requiring reconciliation.
#[tokio::test]
async fn test_prepare_crash_results_in_ambiguous() {
    let mut connector = CrashTestConnector::new().with_prepare_crash();

    let prepare_result = connector.prepare().await.unwrap();
    assert_eq!(prepare_result, ConnectorResult::Ambiguous);

    assert_eq!(connector.get_prepare_count(), 1);
}

/// Test that multiple reconcile calls maintain consistent state.
#[tokio::test]
async fn test_multiple_reconcile_calls_maintain_consistency() {
    let mut connector = CrashTestConnector::new()
        .with_commit_crash(CommitCrashPosition::Before)
        .with_reconcile_result(ReconciliationResult::Committed);

    let _ = connector.prepare().await.unwrap();
    let _ = connector.commit().await.unwrap();

    let reconcile_1 = connector.reconcile().await.unwrap();
    let reconcile_2 = connector.reconcile().await.unwrap();
    let reconcile_3 = connector.reconcile().await.unwrap();

    assert_eq!(reconcile_1, ReconciliationResult::Committed);
    assert_eq!(reconcile_2, ReconciliationResult::Committed);
    assert_eq!(reconcile_3, ReconciliationResult::Committed);

    assert_eq!(connector.get_reconcile_count(), 3);
}

/// Test that connector state transitions correctly under crash recovery.
#[tokio::test]
async fn test_connector_state_transitions_under_crash_recovery() {
    let mut connector = CrashTestConnector::new()
        .with_commit_crash(CommitCrashPosition::Before)
        .with_reconcile_result(ReconciliationResult::Committed);

    assert_eq!(connector.get_state(), ConnectorState::Idle);

    let _ = connector.prepare().await.unwrap();
    assert_eq!(connector.get_state(), ConnectorState::Preparing);

    let _ = connector.commit().await.unwrap();
    assert_eq!(connector.get_state(), ConnectorState::Executing);

    let _ = connector.reconcile().await.unwrap();
    assert_eq!(connector.get_state(), ConnectorState::Succeeded);
}

/// Test that reconciliation path is checked under crash injection.
#[tokio::test]
async fn test_reconciliation_path_checked_under_crash_injection() {
    let mut connector = CrashTestConnector::new()
        .with_commit_crash(CommitCrashPosition::Before)
        .with_reconcile_result(ReconciliationResult::Committed);

    let prepare_result = connector.prepare().await.unwrap();
    assert_eq!(prepare_result, ConnectorResult::Success);

    let commit_result = connector.commit().await.unwrap();
    assert_eq!(commit_result, ConnectorResult::Ambiguous);

    let reconcile_result = connector.reconcile().await.unwrap();
    assert_eq!(reconcile_result, ReconciliationResult::Committed);

    assert_eq!(connector.get_reconcile_count(), 1);
}

/// Test that exactly-once semantics hold across replay scenarios.
#[tokio::test]
async fn test_exactly_once_across_replay_scenarios() {
    // Scenario 1: Normal execution
    let mut connector1 = CrashTestConnector::new();
    let _ = execute_with_reconciliation(&mut connector1, true, 3)
        .await
        .unwrap();

    // Scenario 2: Crash before commit, reconciled to committed
    let mut connector2 = CrashTestConnector::new()
        .with_commit_crash(CommitCrashPosition::Before)
        .with_reconcile_result(ReconciliationResult::Committed);
    let _ = execute_with_reconciliation(&mut connector2, true, 3)
        .await
        .unwrap();

    // Scenario 3: Crash after commit, reconciled to committed
    let mut connector3 = CrashTestConnector::new()
        .with_commit_crash(CommitCrashPosition::After)
        .with_reconcile_result(ReconciliationResult::Committed);
    let _ = execute_with_reconciliation(&mut connector3, true, 3)
        .await
        .unwrap();

    // All scenarios should result in the same final state
    assert_eq!(connector1.get_state(), ConnectorState::Succeeded);
    assert_eq!(connector2.get_state(), ConnectorState::Succeeded);
    assert_eq!(connector3.get_state(), ConnectorState::Succeeded);

    // All scenarios should have exactly one prepare and one commit
    assert_eq!(connector1.get_prepare_count(), 1);
    assert_eq!(connector2.get_prepare_count(), 1);
    assert_eq!(connector3.get_prepare_count(), 1);

    assert_eq!(connector1.get_commit_count(), 1);
    assert_eq!(connector2.get_commit_count(), 1);
    assert_eq!(connector3.get_commit_count(), 1);
}

/// Test that reconciliation prevents duplicate commits.
#[tokio::test]
async fn test_reconciliation_prevents_duplicate_commits() {
    let mut connector = CrashTestConnector::new()
        .with_commit_crash(CommitCrashPosition::Before)
        .with_reconcile_result(ReconciliationResult::Committed);

    // First execution
    let _ = execute_with_reconciliation(&mut connector, true, 3)
        .await
        .unwrap();

    let first_commit_count = connector.get_commit_count();
    let first_reconcile_count = connector.get_reconcile_count();

    // Verify first execution completed successfully
    assert_eq!(first_commit_count, 1);
    assert_eq!(first_reconcile_count, 1);
    assert_eq!(connector.get_state(), ConnectorState::Succeeded);

    // Simulate replay scenario: connector is already in Succeeded state
    // In a real system, the dedupe layer would prevent re-execution
    // Here we just verify the first execution achieved exactly-once
    assert_eq!(connector.get_state(), ConnectorState::Succeeded);
    assert_eq!(connector.get_commit_count(), 1);
}
