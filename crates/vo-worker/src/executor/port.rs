//! Managed-effect executor trait and default implementation (ADR-030, ADR-041).

use crate::connector::{
    CommitOutcome, Connector, ConnectorError, ConnectorRegistry, PreparedEffect, ReconcileOutcome,
};
use crate::executor::error::ManagedEffectError;
use crate::executor::task::{ExecutionOutcome, ManagedEffectTask};
use async_trait::async_trait;

/// Trait for executing managed effects through the dedicated path.
///
/// Implementations MUST isolate managed-effect execution from unsafe
/// activity execution. Panics in the general activity pool MUST NOT
/// crash the managed-effect executor (ADR-030 §1, invariant).
#[async_trait]
pub trait ManagedEffectExecutor: Send + Sync + 'static {
    async fn execute(
        &self,
        task: ManagedEffectTask,
    ) -> Result<ExecutionOutcome, ManagedEffectError>;
}

/// Default implementation that routes through the Connector prepare→commit
/// lifecycle with automatic reconciliation for ambiguous outcomes (ADR-041).
pub struct DefaultManagedEffectExecutor {
    registry: ConnectorRegistry,
}

impl DefaultManagedEffectExecutor {
    #[must_use]
    pub fn new(registry: ConnectorRegistry) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl ManagedEffectExecutor for DefaultManagedEffectExecutor {
    async fn execute(
        &self,
        task: ManagedEffectTask,
    ) -> Result<ExecutionOutcome, ManagedEffectError> {
        let connector = self.registry.get(task.connector_type()).ok_or_else(|| {
            ManagedEffectError::ConnectorNotFound(task.connector_type().to_string())
        })?;

        let prepared = connector
            .prepare(
                task.intent().clone(),
                task.effect_id().to_string(),
                task.fence(),
            )
            .await
            .map_err(|e| ManagedEffectError::PrepareFailed(e.to_string()))?;

        match connector.commit(prepared).await {
            Ok(CommitOutcome::Committed { receipt }) => Ok(ExecutionOutcome::Committed { receipt }),
            Ok(CommitOutcome::Failed) => Ok(ExecutionOutcome::RolledBack {
                reason: "connector reported failure".to_string(),
            }),
            Ok(CommitOutcome::Ambiguous) => {
                let reconcile_result = connector.reconcile(task.effect_id()).await;
                match reconcile_result {
                    Ok(ReconcileOutcome::Committed { receipt }) => {
                        Ok(ExecutionOutcome::Committed { receipt })
                    }
                    Ok(ReconcileOutcome::NotCommitted) => Ok(ExecutionOutcome::RolledBack {
                        reason: "reconciliation confirmed not committed".to_string(),
                    }),
                    Ok(ReconcileOutcome::StillAmbiguous) => Ok(ExecutionOutcome::Ambiguous {
                        connector_type: task.connector_type().to_string(),
                    }),
                    Err(e) => Err(ManagedEffectError::ReconciliationFailed(e.to_string())),
                }
            }
            Err(ConnectorError::Retryable(msg)) => Err(ManagedEffectError::CommitFailed(msg)),
            Err(ConnectorError::Terminal(msg)) => Ok(ExecutionOutcome::RolledBack {
                reason: format!("terminal connector error: {msg}"),
            }),
            Err(ConnectorError::CompensationNotSupported(msg)) => {
                Err(ManagedEffectError::CommitFailed(msg))
            }
        }
    }
}
