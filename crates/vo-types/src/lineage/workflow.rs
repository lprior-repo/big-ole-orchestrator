//! Workflow lineage binding (ADR-038).
//!
//! Binds a stable lineage_id to an epoch with optional parent reference.
//! Enables continue-as-new while preserving workflow identity across epochs.

use serde::{Deserialize, Serialize};

use super::epoch::Epoch;
use super::error::LineageError;

/// WorkflowLineage tracks workflow identity across epoch rollover boundaries.
///
/// When a workflow executes continue-as-new, it creates a new epoch within
/// the same lineage, preserving lineage_id while incrementing the epoch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowLineage {
    lineage_id: String,
    epoch: Epoch,
    parent_epoch: Option<Epoch>,
}

impl WorkflowLineage {
    /// Create a new lineage with the given ID at epoch ZERO.
    pub fn new(lineage_id: String) -> Result<Self, LineageError> {
        if lineage_id.is_empty() {
            return Err(LineageError::InvalidLineageId);
        }
        Ok(Self {
            lineage_id,
            epoch: Epoch::ZERO,
            parent_epoch: None,
        })
    }

    /// Create a new epoch within this lineage (continue-as-new).
    pub fn continue_as_new(&self) -> Result<Self, LineageError> {
        if self.epoch == Epoch::ZERO && self.parent_epoch.is_some() {
            return Err(LineageError::InvalidEpochTransition);
        }
        Ok(Self {
            lineage_id: self.lineage_id.clone(),
            epoch: Epoch::new(self.epoch.value() + 1),
            parent_epoch: Some(self.epoch),
        })
    }

    /// Returns the stable lineage identifier.
    #[must_use]
    pub fn lineage_id(&self) -> &str {
        &self.lineage_id
    }

    /// Returns the current epoch within this lineage.
    #[must_use]
    pub fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// Returns the parent epoch if this is a continue-as-new epoch.
    #[must_use]
    pub fn parent_epoch(&self) -> Option<Epoch> {
        self.parent_epoch
    }
}
