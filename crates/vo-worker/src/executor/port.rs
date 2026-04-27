//! Managed-effect executor trait and default implementation (ADR-030, ADR-041).

use crate::connector::{
    CommitOutcome, Connector, ConnectorError, ConnectorRegistry, PreparedEffect, ReconcileOutcome,
};
use crate::executor::error::ManagedEffectError;
use crate::executor::task::{ExecutionOutcome, ManagedEffectTask};
use async_trait::async_trait;
use futures::FutureExt;
use std::time::Duration;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

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
    timeout: Duration,
}

impl DefaultManagedEffectExecutor {
    #[must_use]
    pub fn new(registry: ConnectorRegistry) -> Self {
        Self {
            registry,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    #[must_use]
    pub fn with_timeout(registry: ConnectorRegistry, timeout: Duration) -> Self {
        Self { registry, timeout }
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

        let prepared = {
            let result = timeout(
                self.timeout,
                connector.prepare(
                    task.intent().clone(),
                    task.effect_id().to_string(),
                    task.fence(),
                ),
            )
            .await;
            match result {
                Err(_) => return Err(ManagedEffectError::Timeout(self.timeout)),
                Ok(Err(e)) => return Err(ManagedEffectError::PrepareFailed(e.to_string())),
                Ok(Ok(prepared)) => prepared,
            }
        };

        let commit_result = timeout(self.timeout, connector.commit(prepared)).await;
        let commit_outcome = match commit_result {
            Err(_) => return Err(ManagedEffectError::Timeout(self.timeout)),
            Ok(Err(ConnectorError::Retryable(msg))) => {
                return Err(ManagedEffectError::CommitFailed(msg))
            }
            Ok(Err(ConnectorError::Terminal(msg))) => {
                return Ok(ExecutionOutcome::RolledBack {
                    reason: format!("terminal connector error: {msg}"),
                });
            }
            Ok(Err(ConnectorError::CompensationNotSupported(msg))) => {
                return Err(ManagedEffectError::CommitFailed(msg))
            }
            Ok(Ok(outcome)) => outcome,
        };

        match commit_outcome {
            CommitOutcome::Committed { receipt } => Ok(ExecutionOutcome::Committed { receipt }),
            CommitOutcome::Failed => Ok(ExecutionOutcome::RolledBack {
                reason: "connector reported failure".to_string(),
            }),
            CommitOutcome::Ambiguous => {
                let reconcile_result = timeout(self.timeout, connector.reconcile(task.effect_id())).await;
                match reconcile_result {
                    Err(_) => Err(ManagedEffectError::Timeout(self.timeout)),
                    Ok(Err(e)) => Err(ManagedEffectError::ReconciliationFailed(e.to_string())),
                    Ok(Ok(ReconcileOutcome::Committed { receipt })) => {
                        Ok(ExecutionOutcome::Committed { receipt })
                    }
                    Ok(Ok(ReconcileOutcome::NotCommitted)) => Ok(ExecutionOutcome::RolledBack {
                        reason: "reconciliation confirmed not committed".to_string(),
                    }),
                    Ok(Ok(ReconcileOutcome::StillAmbiguous)) => Ok(ExecutionOutcome::Ambiguous {
                        connector_type: task.connector_type().to_string(),
                    }),
                }
            }
        }
    }
}

#[allow(dead_code)]
pub async fn execute_with_panic_catch<F, T>(
    future: F,
) -> Result<T, ManagedEffectError>
where
    F: std::future::Future<Output = T> + Send + std::panic::UnwindSafe,
{
    match tokio::time::timeout(Duration::from_secs(300), AssertUnwindSafe(future).catch_unwind()).await {
        Ok(Ok(val)) => Ok(val),
        Ok(Err(panic_payload)) => {
            let msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic".to_string()
            };
            Err(ManagedEffectError::HandlerPanic(msg))
        }
        Err(_) => Err(ManagedEffectError::HandlerPanic("timeout".to_string())),
    }
}

use std::panic::AssertUnwindSafe;
