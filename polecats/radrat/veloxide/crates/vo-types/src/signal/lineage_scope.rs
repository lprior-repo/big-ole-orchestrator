//! LineageScope — Delivery scope for a signal per ADR-042 Section 2

use serde::{Deserialize, Serialize};

/// Delivery scope for a signal — determines whether the signal targets a specific
/// epoch or the currently active epoch within a lineage.
///
/// Per ADR-042 Section 2:
/// - `EpochLocal`: Signal targets a specific, immutable epoch
/// - `LineageWide`: Signal routes to the currently active epoch within the lineage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LineageScope {
    /// Signal targets a specific epoch (immutable once set).
    EpochLocal,
    /// Signal routes to the currently active epoch within the lineage.
    LineageWide,
}

impl LineageScope {
    /// Returns `true` if this scope is epoch-local.
    #[must_use]
    pub const fn is_epoch_local(&self) -> bool {
        matches!(self, Self::EpochLocal)
    }

    /// Returns `true` if this scope is lineage-wide.
    #[must_use]
    pub const fn is_lineage_wide(&self) -> bool {
        matches!(self, Self::LineageWide)
    }
}
