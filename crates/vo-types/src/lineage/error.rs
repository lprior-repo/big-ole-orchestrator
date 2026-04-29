//! Lineage construction errors (ADR-038).
//!
//! Enumerates failures that can occur when building lineage objects.

use serde::{Deserialize, Serialize};

/// Error constructing a lineage object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineageError {
    /// Attempted to create a lineage with an invalid ID format.
    InvalidLineageId,
    /// Attempted to continue a lineage that has been tombstoned.
    LineageTombstoned,
    /// Invalid epoch transition (e.g., epoch decrement).
    InvalidEpochTransition,
}

impl std::fmt::Display for LineageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidLineageId => write!(f, "invalid lineage ID format"),
            Self::LineageTombstoned => write!(f, "lineage has been tombstoned"),
            Self::InvalidEpochTransition => write!(f, "invalid epoch transition"),
        }
    }
}

impl std::error::Error for LineageError {}
