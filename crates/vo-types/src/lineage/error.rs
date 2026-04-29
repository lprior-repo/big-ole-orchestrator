//! Lineage construction errors (ADR-038).
//!
//! Enumerates failures that can occur when building lineage objects.

use serde::{Deserialize, Serialize};

use super::epoch::Epoch;

/// Error constructing a lineage object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineageError {
    /// Attempted to create a lineage with an empty ID.
    EmptyLineageId,
    /// Attempted to create a lineage with an invalid ID format.
    InvalidLineageId,
    /// Attempted to continue a lineage that has been tombstoned.
    LineageTombstoned,
    /// Invalid epoch transition (e.g., epoch decrement).
    InvalidEpochTransition { parent_epoch: Epoch, epoch: Epoch },
    /// Epoch overflow: cannot advance beyond u64::MAX.
    EpochOverflow,
}

impl std::fmt::Display for LineageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyLineageId => write!(f, "empty lineage ID"),
            Self::InvalidLineageId => write!(f, "invalid lineage ID format"),
            Self::LineageTombstoned => write!(f, "lineage has been tombstoned"),
            Self::InvalidEpochTransition {
                parent_epoch,
                epoch,
            } => {
                write!(
                    f,
                    "parent_epoch ({parent_epoch}) must be less than epoch ({epoch})"
                )
            }
            Self::EpochOverflow => write!(f, "epoch overflow: cannot advance beyond u64::MAX"),
        }
    }
}

impl std::error::Error for LineageError {}
