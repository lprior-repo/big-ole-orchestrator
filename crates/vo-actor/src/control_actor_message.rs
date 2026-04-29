//! Control actor messages for lifecycle management.

use vo_types::InstanceId;

pub use crate::signal_messages::{SignalName, SignalPayload, WaitKey};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlActorMessage {
    Cancel { instance_id: InstanceId },
    Resume { instance_id: InstanceId },
    AcceptAndResume {
        instance_id: InstanceId,
        wait_key: WaitKey,
        signal_id: SignalName,
        payload: SignalPayload,
    },
}

impl ControlActorMessage {
    #[must_use]
    pub fn new_cancel(instance_id: InstanceId) -> Self {
        Self::Cancel { instance_id }
    }

    #[must_use]
    pub fn new_resume(instance_id: InstanceId) -> Self {
        Self::Resume { instance_id }
    }

    #[must_use]
    pub fn new_accept_and_resume(
        instance_id: InstanceId,
        wait_key: WaitKey,
        signal_id: SignalName,
        payload: SignalPayload,
    ) -> Self {
        Self::AcceptAndResume {
            instance_id,
            wait_key,
            signal_id,
            payload,
        }
    }
}