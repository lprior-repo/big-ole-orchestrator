//! Managed-effect task and outcome types (ADR-030).

use serde::{Deserialize, Serialize};

/// A task queued for execution on the managed-effect path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedEffectTask {
    effect_id: String,
    fence: u64,
    connector_type: String,
    intent: serde_json::Value,
}

/// Outcome of executing a managed effect through the dedicated path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionOutcome {
    Committed { receipt: String },
    RolledBack { reason: String },
    Ambiguous { connector_type: String },
}

impl ManagedEffectTask {
    #[must_use]
    pub fn new(
        effect_id: String,
        fence: u64,
        connector_type: String,
        intent: serde_json::Value,
    ) -> Self {
        Self {
            effect_id,
            fence,
            connector_type,
            intent,
        }
    }

    #[must_use]
    pub fn effect_id(&self) -> &str {
        &self.effect_id
    }

    #[must_use]
    pub fn fence(&self) -> u64 {
        self.fence
    }

    #[must_use]
    pub fn connector_type(&self) -> &str {
        &self.connector_type
    }

    #[must_use]
    pub fn intent(&self) -> &serde_json::Value {
        &self.intent
    }
}

impl ExecutionOutcome {
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Committed { .. } | Self::RolledBack { .. })
    }
}
