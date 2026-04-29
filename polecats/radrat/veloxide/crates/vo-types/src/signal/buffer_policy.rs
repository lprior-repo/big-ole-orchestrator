//! BufferPolicy — Determines signal buffering behavior per ADR-042 Section 3

use serde::{Deserialize, Serialize};

/// Determines signal buffering behavior when no matching wait is active.
///
/// Per ADR-042 Section 3, the buffer policy controls whether signals are
/// buffered for later delivery or rejected outright.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum BufferPolicy {
    /// Return a structured mismatch error when no matching wait is active.
    #[default]
    Reject,
    /// Store exactly one pending signal for the matching key.
    BufferOne,
    /// Store a bounded queue of pending signals for the matching key.
    BufferMany,
}

impl BufferPolicy {
    /// Returns `true` if this policy buffers signals (BufferOne or BufferMany).
    #[must_use]
    pub const fn is_buffering(&self) -> bool {
        matches!(self, Self::BufferOne | Self::BufferMany)
    }
}
