use super::error::ManagedEffectError;
use super::port::{DefaultManagedEffectExecutor, ManagedEffectExecutor};
use super::task::{ExecutionOutcome, ManagedEffectTask};
use crate::connector::{
    CommitOutcome, Connector, ConnectorError, ConnectorRegistry, PreparedEffect, ReconcileOutcome,
};
use serde_json::json;

struct StubConnector {
    commit_result: CommitOutcome,
    reconcile_result: ReconcileOutcome,
}

impl StubConnector {
    fn new(commit_result: CommitOutcome, reconcile_result: ReconcileOutcome) -> Self {
        Self {
            commit_result,
            reconcile_result,
        }
    }
}

#[async_trait::async_trait]
impl Connector for StubConnector {
    fn connector_type(&self) -> &str {
        "stub"
    }
    fn connector_version(&self) -> &str {
        "0.0.1"
    }
    fn supports_compensation(&self) -> bool {
        false
    }
    async fn prepare(
        &self,
        _intent: serde_json::Value,
        effect_id: String,
        fence: u64,
    ) -> Result<PreparedEffect, ConnectorError> {
        Ok(PreparedEffect {
            effect_id,
            payload: json!({}),
            fence,
        })
    }
    async fn commit(&self, _prepared: PreparedEffect) -> Result<CommitOutcome, ConnectorError> {
        Ok(self.commit_result.clone())
    }
    async fn reconcile(&self, _effect_id: &str) -> Result<ReconcileOutcome, ConnectorError> {
        Ok(self.reconcile_result.clone())
    }
}

struct AlwaysRetryableCommitConnector;

#[async_trait::async_trait]
impl Connector for AlwaysRetryableCommitConnector {
    fn connector_type(&self) -> &str {
        "always-retryable"
    }
    fn connector_version(&self) -> &str {
        "0.0.1"
    }
    fn supports_compensation(&self) -> bool {
        false
    }
    async fn prepare(
        &self,
        _intent: serde_json::Value,
        effect_id: String,
        fence: u64,
    ) -> Result<PreparedEffect, ConnectorError> {
        Ok(PreparedEffect {
            effect_id,
            payload: json!({}),
            fence,
        })
    }
    async fn commit(&self, _prepared: PreparedEffect) -> Result<CommitOutcome, ConnectorError> {
        Err(ConnectorError::retryable("timeout"))
    }
    async fn reconcile(&self, _effect_id: &str) -> Result<ReconcileOutcome, ConnectorError> {
        Ok(ReconcileOutcome::StillAmbiguous)
    }
}

struct AlwaysTerminalCommitConnector;

#[async_trait::async_trait]
impl Connector for AlwaysTerminalCommitConnector {
    fn connector_type(&self) -> &str {
        "always-terminal"
    }
    fn connector_version(&self) -> &str {
        "0.0.1"
    }
    fn supports_compensation(&self) -> bool {
        false
    }
    async fn prepare(
        &self,
        _intent: serde_json::Value,
        effect_id: String,
        fence: u64,
    ) -> Result<PreparedEffect, ConnectorError> {
        Ok(PreparedEffect {
            effect_id,
            payload: json!({}),
            fence,
        })
    }
    async fn commit(&self, _prepared: PreparedEffect) -> Result<CommitOutcome, ConnectorError> {
        Err(ConnectorError::terminal("bad request"))
    }
    async fn reconcile(&self, _effect_id: &str) -> Result<ReconcileOutcome, ConnectorError> {
        Ok(ReconcileOutcome::NotCommitted)
    }
}

struct AlwaysPrepareFailConnector;

#[async_trait::async_trait]
impl Connector for AlwaysPrepareFailConnector {
    fn connector_type(&self) -> &str {
        "prepare-fail"
    }
    fn connector_version(&self) -> &str {
        "0.0.1"
    }
    fn supports_compensation(&self) -> bool {
        false
    }
    async fn prepare(
        &self,
        _intent: serde_json::Value,
        _effect_id: String,
        _fence: u64,
    ) -> Result<PreparedEffect, ConnectorError> {
        Err(ConnectorError::retryable("conn refused"))
    }
    async fn commit(&self, _prepared: PreparedEffect) -> Result<CommitOutcome, ConnectorError> {
        Err(ConnectorError::terminal("unreachable"))
    }
    async fn reconcile(&self, _effect_id: &str) -> Result<ReconcileOutcome, ConnectorError> {
        Ok(ReconcileOutcome::NotCommitted)
    }
}

fn make_task(effect_id: &str, fence: u64, connector_type: &str) -> ManagedEffectTask {
    ManagedEffectTask::new(
        effect_id.to_string(),
        fence,
        connector_type.to_string(),
        json!({"test": true}),
    )
}

fn make_registry() -> ConnectorRegistry {
    ConnectorRegistry::new()
}

#[tokio::test]
async fn managed_effect_routes_to_dedicated_path_and_committed() {
    let mut registry = make_registry();
    registry.register(
        "stub".to_string(),
        Box::new(StubConnector::new(
            CommitOutcome::Committed {
                receipt: "r-1".to_string(),
            },
            ReconcileOutcome::NotCommitted,
        )),
    );
    let executor = DefaultManagedEffectExecutor::new(registry);
    let task = make_task("fx-1", 1, "stub");
    let result = executor.execute(task).await;
    assert_eq!(
        result,
        Ok(ExecutionOutcome::Committed {
            receipt: "r-1".to_string()
        })
    );
}

#[tokio::test]
async fn managed_effect_rolled_back_on_connector_failure() {
    let mut registry = make_registry();
    registry.register(
        "stub".to_string(),
        Box::new(StubConnector::new(
            CommitOutcome::Failed,
            ReconcileOutcome::NotCommitted,
        )),
    );
    let executor = DefaultManagedEffectExecutor::new(registry);
    let task = make_task("fx-2", 1, "stub");
    let result = executor.execute(task).await;
    assert_eq!(
        result,
        Ok(ExecutionOutcome::RolledBack {
            reason: "connector reported failure".to_string(),
        })
    );
}

#[tokio::test]
async fn managed_effect_reconciles_on_ambiguous_commit() {
    let mut registry = make_registry();
    registry.register(
        "stub".to_string(),
        Box::new(StubConnector::new(
            CommitOutcome::Ambiguous,
            ReconcileOutcome::Committed {
                receipt: "r-rec".to_string(),
            },
        )),
    );
    let executor = DefaultManagedEffectExecutor::new(registry);
    let task = make_task("fx-3", 1, "stub");
    let result = executor.execute(task).await;
    assert_eq!(
        result,
        Ok(ExecutionOutcome::Committed {
            receipt: "r-rec".to_string()
        })
    );
}

#[tokio::test]
async fn managed_effect_rolled_back_after_reconciliation_confirms_not_committed() {
    let mut registry = make_registry();
    registry.register(
        "stub".to_string(),
        Box::new(StubConnector::new(
            CommitOutcome::Ambiguous,
            ReconcileOutcome::NotCommitted,
        )),
    );
    let executor = DefaultManagedEffectExecutor::new(registry);
    let task = make_task("fx-4", 1, "stub");
    let result = executor.execute(task).await;
    assert_eq!(
        result,
        Ok(ExecutionOutcome::RolledBack {
            reason: "reconciliation confirmed not committed".to_string(),
        })
    );
}

#[tokio::test]
async fn managed_effect_ambiguous_when_reconciliation_still_ambiguous() {
    let mut registry = make_registry();
    registry.register(
        "stub".to_string(),
        Box::new(StubConnector::new(
            CommitOutcome::Ambiguous,
            ReconcileOutcome::StillAmbiguous,
        )),
    );
    let executor = DefaultManagedEffectExecutor::new(registry);
    let task = make_task("fx-5", 1, "stub");
    let result = executor.execute(task).await;
    assert_eq!(
        result,
        Ok(ExecutionOutcome::Ambiguous {
            connector_type: "stub".to_string(),
        })
    );
}

#[tokio::test]
async fn managed_effect_error_when_connector_not_found() {
    let registry = make_registry();
    let executor = DefaultManagedEffectExecutor::new(registry);
    let task = make_task("fx-6", 1, "nonexistent");
    let result = executor.execute(task).await;
    assert!(matches!(
        result,
        Err(ManagedEffectError::ConnectorNotFound(_))
    ));
}

#[tokio::test]
async fn managed_effect_error_on_retryable_commit_failure() {
    let mut registry = make_registry();
    registry.register(
        "always-retryable".to_string(),
        Box::new(AlwaysRetryableCommitConnector),
    );
    let executor = DefaultManagedEffectExecutor::new(registry);
    let task = make_task("fx-7", 1, "always-retryable");
    let result = executor.execute(task).await;
    assert!(matches!(result, Err(ManagedEffectError::CommitFailed(_))));
    assert!(result.unwrap_err().is_retryable());
}

#[tokio::test]
async fn managed_effect_rolled_back_on_terminal_commit_error() {
    let mut registry = make_registry();
    registry.register(
        "always-terminal".to_string(),
        Box::new(AlwaysTerminalCommitConnector),
    );
    let executor = DefaultManagedEffectExecutor::new(registry);
    let task = make_task("fx-8", 1, "always-terminal");
    let result = executor.execute(task).await;
    assert!(matches!(result, Ok(ExecutionOutcome::RolledBack { .. })));
}

#[tokio::test]
async fn managed_effect_error_on_prepare_failure() {
    let mut registry = make_registry();
    registry.register(
        "prepare-fail".to_string(),
        Box::new(AlwaysPrepareFailConnector),
    );
    let executor = DefaultManagedEffectExecutor::new(registry);
    let task = make_task("fx-9", 1, "prepare-fail");
    let result = executor.execute(task).await;
    assert!(matches!(result, Err(ManagedEffectError::PrepareFailed(_))));
}

#[tokio::test]
async fn managed_effect_ambiguous_outcome_is_not_terminal() {
    let mut registry = make_registry();
    registry.register(
        "stub".to_string(),
        Box::new(StubConnector::new(
            CommitOutcome::Ambiguous,
            ReconcileOutcome::StillAmbiguous,
        )),
    );
    let executor = DefaultManagedEffectExecutor::new(registry);
    let task = make_task("fx-10", 1, "stub");
    let result = executor.execute(task).await;
    assert!(result.is_ok());
    assert!(!result.unwrap().is_terminal());
}

#[test]
fn managed_effect_error_is_retryable_for_commit_and_reconciliation() {
    let commit_err = ManagedEffectError::CommitFailed("timeout".to_string());
    assert!(commit_err.is_retryable());

    let reconcile_err = ManagedEffectError::ReconciliationFailed("timeout".to_string());
    assert!(reconcile_err.is_retryable());

    let prepare_err = ManagedEffectError::PrepareFailed("bad input".to_string());
    assert!(!prepare_err.is_retryable());

    let not_found_err = ManagedEffectError::ConnectorNotFound("missing".to_string());
    assert!(!not_found_err.is_retryable());
}

#[test]
fn execution_outcome_is_terminal_for_committed_and_rolled_back() {
    let committed = ExecutionOutcome::Committed {
        receipt: "r".to_string(),
    };
    assert!(committed.is_terminal());

    let rolled_back = ExecutionOutcome::RolledBack {
        reason: "err".to_string(),
    };
    assert!(rolled_back.is_terminal());

    let ambiguous = ExecutionOutcome::Ambiguous {
        connector_type: "http".to_string(),
    };
    assert!(!ambiguous.is_terminal());
}

#[test]
fn managed_effect_task_accessors() {
    let task = ManagedEffectTask::new(
        "fx-acc".to_string(),
        42,
        "http".to_string(),
        json!({"method": "POST"}),
    );
    assert_eq!(task.effect_id(), "fx-acc");
    assert_eq!(task.fence(), 42);
    assert_eq!(task.connector_type(), "http");
    assert_eq!(task.intent(), &json!({"method": "POST"}));
}
