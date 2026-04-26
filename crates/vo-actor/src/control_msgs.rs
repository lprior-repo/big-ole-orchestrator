//! ControlActorMessage enum and constructor implementations.

use crate::signal_messages::{SignalName, SignalPayload, WaitKey};
use crate::InstanceId;

/// Control messages for lifecycle management.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlActorMessage {
    /// Request cancellation of an instance
    Cancel { instance_id: InstanceId },
    /// Request resumption of a paused instance
    Resume { instance_id: InstanceId },
    /// Atomically accept a signal and resume the waiting instance.
    AcceptAndResume {
        instance_id: InstanceId,
        wait_key: crate::WaitKey,
        signal_id: SignalName,
        payload: crate::SignalPayload,
    },
}

// =============================================================================
// Constructor Functions - ControlActorMessage
// =============================================================================

impl ControlActorMessage {
    /// Creates a new `Cancel` message.
    #[must_use]
    pub fn new_cancel(instance_id: InstanceId) -> Self {
        Self::Cancel { instance_id }
    }

    /// Creates a new `Resume` message.
    #[must_use]
    pub fn new_resume(instance_id: InstanceId) -> Self {
        Self::Resume { instance_id }
    }

    /// Creates a new `AcceptAndResume` message.
    #[must_use]
    pub fn new_accept_and_resume(
        instance_id: InstanceId,
        wait_key: crate::WaitKey,
        signal_id: SignalName,
        payload: crate::SignalPayload,
    ) -> Self {
        Self::AcceptAndResume {
            instance_id,
            wait_key,
            signal_id,
            payload,
        }
    }
}
