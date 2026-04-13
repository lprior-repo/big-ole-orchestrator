//! Connector runtime trait and ambiguity handling (ADR-041).
//!
//! Architecture: Data (ConnectorState, ConnectorResult, ReconcileAction)
//!             → Calc (apply_connector_transition, is_terminal, all_variants).
//!             → Runtime (Connector trait, reconcile_ambiguous).
//!
//! This module provides the runtime interface for managed connectors.
//! When a connector operation returns Ambiguous (timeout with unknown server state),
//! the system routes through reconciliation to determine the true outcome,
//! rather than blindly retrying.

use crate::connector::types::{ConnectorResult, ConnectorState, ReconcileAction};

/// Error type for connector operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectorError {
    /// Connector is in a terminal state (Succeeded or Failed).
    TerminalState(ConnectorState),
    /// Connector is not in a valid state for the requested operation.
    InvalidState {
        current: ConnectorState,
        expected: &'static [ConnectorState],
    },
    /// Reconciliation failed to determine the outcome.
    ReconciliationUncertain,
    /// Transport or communication error.
    Transport(String),
}

impl std::fmt::Display for ConnectorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectorError::TerminalState(state) => {
                write!(f, "connector in terminal state: {state:?}")
            }
            ConnectorError::InvalidState { current, expected } => {
                write!(f, "invalid state: {current:?}, expected one of {expected:?}")
            }
            ConnectorError::ReconciliationUncertain => {
                write!(f, "reconciliation could not determine outcome")
            }
            ConnectorError::Transport(msg) => {
                write!(f, "transport error: {msg}")
            }
        }
    }
}

impl std::error::Error for ConnectorError {}

/// Result of a reconciliation query to determine the true outcome
/// of an ambiguous connector operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconciliationResult {
    /// The effect was committed on the server.
    Committed,
    /// The effect was not committed (rolled back or never applied).
    NotCommitted,
    /// Unable to determine the outcome even after reconciliation.
    Unknown,
}

/// Trait for managed connectors that support prepare/commit/reconcile lifecycle (ADR-041).
///
/// Implementors of this trait represent external systems that can be coordinated
/// using the prepare-commit-reconcile pattern. This enables exactly-once semantics
/// by allowing the system to query the true state of the connector when an
/// operation's outcome is ambiguous (e.g., timeout with unknown server state).
///
/// # Ambiguity Handling
///
/// When [`Connector::commit`] returns `Ambiguous`, the caller should invoke
/// [`Connector::reconcile`] to query the server's true state rather than
/// blindly retrying. Blind retry could cause duplicate commits if the original
/// commit actually succeeded.
///
/// # Example
///
/// ```ignore
/// impl Connector for MyDatabase {
///     async fn prepare(&mut self) -> Result<ConnectorResult, ConnectorError> {
///         // Prepare the effect without committing
///         Ok(ConnectorResult::Success)
///     }
///
///     async fn commit(&mut self) -> Result<ConnectorResult, ConnectorError> {
///         // Commit the prepared effect
///         // Returns Ambiguous if timeout with unknown server state
///         Ok(ConnectorResult::Ambiguous)
///     }
///
///     async fn reconcile(&mut self) -> Result<ReconciliationResult, ConnectorError> {
///         // Query server to determine if commit actually succeeded
///         Ok(ReconciliationResult::Committed)
///     }
/// }
/// ```
pub trait Connector: Send + Sync {
    /// Prepare the effect without committing it.
    ///
    /// Returns `Ok(ConnectorResult::Success)` if preparation succeeded.
    /// Returns `Ok(ConnectorResult::Failure)` if preparation failed.
    /// Returns `Ok(ConnectorResult::Ambiguous)` if the outcome is unclear
    /// (should not happen during prepare, but some connectors may).
    fn prepare(&mut self) -> impl std::future::Future<Output = Result<ConnectorResult, ConnectorError>> + Send;

    /// Commit the prepared effect.
    ///
    /// Returns `Ok(ConnectorResult::Success)` if commit succeeded.
    /// Returns `Ok(ConnectorResult::Failure)` if commit failed.
    /// Returns `Ok(ConnectorResult::Ambiguous)` if the outcome is ambiguous
    /// due to timeout with unknown server state. In this case, the caller
    /// MUST call [`Connector::reconcile`] to determine the true outcome
    /// rather than blindly retrying.
    fn commit(&mut self) -> impl std::future::Future<Output = Result<ConnectorResult, ConnectorError>> + Send;

    /// Reconcile to determine the true outcome of a commit that returned `Ambiguous`.
    ///
    /// This queries the server to determine whether the effect was actually committed.
    ///
    /// Returns `Ok(ReconciliationResult::Committed)` if the effect was committed.
    /// Returns `Ok(ReconciliationResult::NotCommitted)` if the effect was not committed.
    /// Returns `Ok(ReconciliationResult::Unknown)` if the outcome cannot be determined.
    fn reconcile(&mut self) -> impl std::future::Future<Output = Result<ReconciliationResult, ConnectorError>> + Send;

    /// Roll back a prepared effect.
    ///
    /// Returns `Ok(ConnectorResult::Success)` if rollback succeeded.
    /// Returns `Ok(ConnectorResult::Failure)` if rollback failed.
    /// Returns `Ok(ConnectorResult::Ambiguous)` if the outcome is ambiguous.
    fn rollback(&mut self) -> impl std::future::Future<Output = Result<ConnectorResult, ConnectorError>> + Send;
}

/// Handle an ambiguous connector result by routing through reconciliation.
///
/// When a connector operation returns `Ambiguous` (typically due to timeout
/// with unknown server state), this function queries the server to determine
/// the true outcome and returns the appropriate action to take.
///
/// # Arguments
///
/// * `connector` - The connector to reconcile
/// * `current_state` - The current connector state (should be `Ambiguous`)
///
/// # Returns
///
/// * `Ok(ReconcileAction::Commit)` - Effect was committed, proceed
/// * `Ok(ReconcileAction::Rollback)` - Effect was not committed, roll back
/// * `Ok(ReconcileAction::Retry)` - Outcome unknown, retry with backoff
/// * `Err(ConnectorError)` - Reconciliation failed
///
/// # Invariants
///
/// INV-C05: When reconciliation returns Committed, the connector state machine
/// transitions to Succeeded via ReconcileSucceeded.
/// INV-C06: When reconciliation returns NotCommitted, the connector state machine
/// transitions to Failed via ReconcileFailed.
/// INV-C07: When reconciliation returns Unknown, the connector state machine
/// transitions to Prepared via ReconcileRetry (enabling retry with backoff).
pub async fn reconcile_ambiguous<C: Connector>(
    connector: &mut C,
    current_state: ConnectorState,
) -> Result<ReconcileAction, ConnectorError> {
    assert_eq!(
        current_state,
        ConnectorState::Ambiguous,
        "reconcile_ambiguous called with non-Ambiguous state: {current_state:?}"
    );

    let result = connector.reconcile().await?;

    match result {
        ReconciliationResult::Committed => Ok(ReconcileAction::Commit),
        ReconciliationResult::NotCommitted => Ok(ReconcileAction::Rollback),
        ReconciliationResult::Unknown => Ok(ReconcileAction::Retry),
    }
}

/// Execute a connector operation with automatic ambiguity handling.
///
/// This function wraps the commit operation and automatically routes
/// ambiguous results through reconciliation, preventing blind retry
/// that could cause duplicate commits.
///
/// # Arguments
///
/// * `connector` - The connector to use
/// * `prepare_first` - Whether to call prepare before commit
///
/// # Returns
///
/// * `Ok(ConnectorResult::Success)` - Operation succeeded
/// * `Ok(ConnectorResult::Failure)` - Operation failed
/// * `Err(ConnectorError)` - Operation error (including reconciliation failure)
pub async fn execute_with_reconciliation<C: Connector>(
    connector: &mut C,
    prepare_first: bool,
) -> Result<ConnectorResult, ConnectorError> {
    if prepare_first {
        let prep_result = connector.prepare().await?;
        match prep_result {
            ConnectorResult::Success => {}
            ConnectorResult::Failure => return Ok(ConnectorResult::Failure),
            ConnectorResult::Ambiguous => {
                let action = reconcile_ambiguous(connector, ConnectorState::Ambiguous).await?;
                return apply_reconcile_action(action);
            }
        }
    }

    let commit_result = connector.commit().await?;

    match commit_result {
        ConnectorResult::Success => Ok(ConnectorResult::Success),
        ConnectorResult::Failure => Ok(ConnectorResult::Failure),
        ConnectorResult::Ambiguous => {
            let state = ConnectorState::Ambiguous;
            let action = reconcile_ambiguous(connector, state).await?;
            apply_reconcile_action(action)
        }
    }
}

fn apply_reconcile_action(action: ReconcileAction) -> Result<ConnectorResult, ConnectorError> {
    match action {
        ReconcileAction::Commit => Ok(ConnectorResult::Success),
        ReconcileAction::Rollback => Ok(ConnectorResult::Failure),
        ReconcileAction::Retry => Ok(ConnectorResult::Ambiguous),
    }
}

#[cfg(test)]
mod tests {
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
        let result = execute_with_reconciliation(&mut connector, true).await.unwrap();
        assert_eq!(result, ConnectorResult::Success);
    }

    #[tokio::test]
    async fn execute_with_reconciliation_resolves_ambiguous() {
        let mut connector = MockConnector::new(ReconciliationResult::Committed);
        let result = execute_with_reconciliation(&mut connector, false).await.unwrap();
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
        let _ = reconcile_ambiguous(&mut connector, ConnectorState::Executing).await;
    }
}