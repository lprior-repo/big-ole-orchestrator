//! Actor message constructors.
//!
//! Constructor functions for actor message types.

use vo_types::{InstanceId, NodeName, SequenceNumber, TimerId, WorkflowName};

use super::types::{ControlActorMessage, InstanceActorMessage};

// =============================================================================
// Constructor Functions - InstanceActorMessage
// =============================================================================

impl InstanceActorMessage {
    /// Creates a new `StartWorkflow` message.
    #[must_use]
    pub fn new_start_workflow(
        instance_id: InstanceId,
        workflow_name: WorkflowName,
        node_name: NodeName,
    ) -> Self {
        Self::StartWorkflow {
            instance_id,
            workflow_name,
            node_name,
        }
    }

    /// Creates a new `StepCompleted` message.
    #[must_use]
    pub fn new_step_completed(
        instance_id: InstanceId,
        node_name: NodeName,
        sequence: SequenceNumber,
    ) -> Self {
        Self::StepCompleted {
            instance_id,
            node_name,
            sequence,
        }
    }

    /// Creates a new `StepFailed` message.
    #[must_use]
    pub fn new_step_failed(
        instance_id: InstanceId,
        node_name: NodeName,
        sequence: SequenceNumber,
        error: String,
    ) -> Self {
        Self::StepFailed {
            instance_id,
            node_name,
            sequence,
            error,
        }
    }

    /// Creates a new `TimerFired` message.
    #[must_use]
    pub fn new_timer_fired(instance_id: InstanceId, timer_id: TimerId) -> Self {
        Self::TimerFired {
            instance_id,
            timer_id,
        }
    }

    /// Creates a new `CancelRequested` message.
    #[must_use]
    pub fn new_cancel_requested(instance_id: InstanceId) -> Self {
        Self::CancelRequested { instance_id }
    }

    /// Creates a new `GetStatus` message.
    #[must_use]
    pub fn new_get_status(instance_id: InstanceId) -> Self {
        Self::GetStatus { instance_id }
    }
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
        signal_id: String,
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

// Note: ractor::Message is automatically implemented for types that are
// Send + Sync + 'static via a blanket impl. Since all our fields are
// Send + Sync newtypes, the trait is already implemented.
