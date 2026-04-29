//! Epoch newtype for workflow lineage (ADR-038).
//!
//! An `Epoch` identifies one execution epoch within a lineage. When a workflow
//! executes continue-as-new, it rolls to a new epoch while preserving lineage identity.

use serde::{Deserialize, Serialize};

/// Epoch counter for continue-as-new rollover (ADR-038).
///
/// Each epoch represents a distinct execution generation within a lineage.
/// Epochs increment monotonically; epoch 0 is the initial execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Epoch(u64);

impl Epoch {
    /// The initial epoch for a new lineage.
    pub const ZERO: Self = Self(0);

    /// Create a new epoch from a raw u64 value.
    #[must_use]
    pub const fn new(epoch: u64) -> Self {
        Self(epoch)
    }

    /// Returns the raw u64 value of this epoch.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    /// Returns the raw u64 value of this epoch (alias for `value`).
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl Default for Epoch {
    fn default() -> Self {
        Self::ZERO
    }
}

impl std::fmt::Display for Epoch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
