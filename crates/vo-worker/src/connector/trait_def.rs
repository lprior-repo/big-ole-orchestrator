//! Core Connector trait (ADR-041 §1).

use crate::connector::{CommitOutcome, ConnectorError, PreparedEffect, ReconcileOutcome};
use async_trait::async_trait;

/// The uniform runtime contract for all managed connectors (ADR-041 §1).
#[async_trait]
pub trait Connector: Send + Sync + 'static {
    fn connector_type(&self) -> &str;
    fn connector_version(&self) -> &str;
    fn supports_compensation(&self) -> bool;

    async fn prepare(
        &self,
        effect_intent: serde_json::Value,
        effect_id: String,
        fence: u64,
    ) -> Result<PreparedEffect, ConnectorError>;

    async fn commit(&self, prepared: PreparedEffect) -> Result<CommitOutcome, ConnectorError>;

    async fn reconcile(&self, effect_id: &str) -> Result<ReconcileOutcome, ConnectorError>;

    async fn compensate(
        &self,
        _compensation_intent: serde_json::Value,
        _compensation_effect_id: String,
        _fence: u64,
    ) -> Result<CommitOutcome, ConnectorError> {
        Err(ConnectorError::compensation_not_supported(
            self.connector_type(),
        ))
    }
}
