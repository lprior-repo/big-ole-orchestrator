//! Core connector types (ADR-041).

use serde::{Deserialize, Serialize};

/// A prepared effect ready for commit (ADR-041 §2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedEffect {
    pub effect_id: String,
    pub payload: serde_json::Value,
    pub fence: u64,
}

/// Outcome of a commit or compensate operation (ADR-041 §1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitOutcome {
    Committed { receipt: String },
    Failed,
    Ambiguous,
}

/// Outcome of a reconciliation operation (ADR-041 §3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconcileOutcome {
    Committed { receipt: String },
    NotCommitted,
    StillAmbiguous,
}

impl From<ReconcileOutcome> for vo_types::ReconcileAction {
    fn from(outcome: ReconcileOutcome) -> Self {
        match outcome {
            ReconcileOutcome::Committed { .. } => vo_types::ReconcileAction::Commit,
            ReconcileOutcome::NotCommitted => vo_types::ReconcileAction::Rollback,
            ReconcileOutcome::StillAmbiguous => vo_types::ReconcileAction::Retry,
        }
    }
}
