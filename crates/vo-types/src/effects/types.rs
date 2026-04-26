//! Core data type definitions for managed effects (ADR-030).

use serde::{Deserialize, Serialize};

/// Lifecycle state of a managed effect (ADR-030).
///
/// Transitions are strictly one-directional: Prepared → Committed | RolledBack.
/// Committed and RolledBack are terminal states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EffectIntent {
    /// Effect has been prepared but not yet committed.
    Prepared,
    /// Effect has been successfully committed (terminal).
    Committed,
    /// Effect has been rolled back (terminal).
    RolledBack,
}

/// Category of managed side-effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EffectKind {
    /// HTTP API call (Stripe, external REST, etc.)
    HttpCall,
    /// SQL database query/write.
    SqlQuery,
    /// Blob storage write (S3, GCS, etc.)
    BlobWrite,
}

/// Durable execution receipt for a committed managed connector effect (ADR-041 §4).
///
/// Write-once immutable record produced when a connector commit succeeds.
/// Used for operator audit, replay, and exact-once deduplication.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExternalReceipt {
    connector_id: String,
    connector_version: String,
    sink_kind: EffectKind,
    receipt_payload: serde_json::Value,
}

impl ExternalReceipt {
    #[must_use]
    pub fn new(
        connector_id: String,
        connector_version: String,
        sink_kind: EffectKind,
        receipt_payload: serde_json::Value,
    ) -> Option<Self> {
        if connector_id.is_empty() {
            return None;
        }
        Some(Self {
            connector_id,
            connector_version,
            sink_kind,
            receipt_payload,
        })
    }

    #[must_use]
    pub fn connector_id(&self) -> &str {
        &self.connector_id
    }

    #[must_use]
    pub fn connector_version(&self) -> &str {
        &self.connector_version
    }

    #[must_use]
    pub fn sink_kind(&self) -> EffectKind {
        self.sink_kind
    }

    #[must_use]
    pub fn receipt_payload(&self) -> &serde_json::Value {
        &self.receipt_payload
    }
}

/// Compensation policy for an effect (ADR-030 §5, ADR-034).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CompensationPolicy {
    /// No compensation needed or available.
    None,
    /// Manual compensation — requires human intervention.
    Manual,
    /// Automatic compensation — engine drives rollback.
    Automatic,
}

/// Event that triggers an effect state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectTransitionEvent {
    /// Commit the effect — transition from Prepared to Committed.
    Commit,
    /// Roll back the effect — transition from Prepared to RolledBack.
    Rollback,
}

/// Error returned when an effect state transition is invalid.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EffectTransitionError {
    #[error("Cannot transition from terminal effect state")]
    TerminalStateTransition,
    #[error("Invalid effect state transition")]
    InvalidTransition,
}

/// Persisted record of a managed effect.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EffectRecord {
    intent_id: String,
    kind: EffectKind,
    params_json: serde_json::Value,
    status: EffectIntent,
    committed_at: Option<crate::types::TimestampMs>,
}

impl EffectRecord {
    /// Construct a new EffectRecord.
    ///
    /// Returns `None` if `intent_id` is empty (INV-EFF-003).
    #[must_use]
    pub fn new(
        intent_id: String,
        kind: EffectKind,
        params_json: serde_json::Value,
        status: EffectIntent,
        committed_at: Option<crate::types::TimestampMs>,
    ) -> Option<Self> {
        if intent_id.is_empty() {
            return None;
        }
        Some(Self {
            intent_id,
            kind,
            params_json,
            status,
            committed_at,
        })
    }

    #[must_use]
    pub fn intent_id(&self) -> &str {
        &self.intent_id
    }

    #[must_use]
    pub fn kind(&self) -> EffectKind {
        self.kind
    }

    #[must_use]
    pub fn params_json(&self) -> &serde_json::Value {
        &self.params_json
    }

    #[must_use]
    pub fn status(&self) -> EffectIntent {
        self.status
    }

    #[must_use]
    pub fn committed_at(&self) -> Option<&crate::types::TimestampMs> {
        self.committed_at.as_ref()
    }
}
