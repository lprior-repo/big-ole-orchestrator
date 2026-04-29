//! SignalDelivery — Outcome of attempting to deliver a signal

use serde::{Deserialize, Serialize};

/// Outcome of attempting to deliver a signal.
///
/// SignalDelivery captures the result of a signal delivery attempt:
/// whether it was accepted, rejected, or buffered for later delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SignalDelivery {
    /// Signal matched a wait and was consumed.
    Accepted,
    /// Signal did not match (Reject policy) or was a duplicate.
    Rejected,
    /// Signal was buffered for later delivery (BufferOne/BufferMany policy).
    Buffered,
}

impl SignalDelivery {
    /// Returns `true` if signal processing is complete (no further action needed).
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Accepted | Self::Rejected)
    }

    /// Returns `true` if the signal is pending (buffered for future delivery).
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        matches!(self, Self::Buffered)
    }
}
