//! InstanceActorMessage enum and constructor implementations.

use crate::signal_messages::NodeName;
use vo_types::{InstanceId, SequenceNumber, TimerId, WorkflowName};

/// Messages sent to/from workflow instance actors.
///
/// These are commands that drive the workflow instance lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceActorMessage {
    /// Start a new workflow instance
    StartWorkflow {
        instance_id: InstanceId,
        workflow_name: WorkflowName,
        node_name: NodeName,
    },
    /// A step in the workflow completed
    StepCompleted {
        instance_id: InstanceId,
        node_name: NodeName,
        sequence: SequenceNumber,
    },
    /// A step in the workflow failed
    StepFailed {
        instance_id: InstanceId,
        node_name: NodeName,
        sequence: SequenceNumber,
        error: String,
    },
    /// A timer fired
    TimerFired {
        instance_id: InstanceId,
        timer_id: TimerId,
    },
    /// Cancellation was requested
    CancelRequested { instance_id: InstanceId },
    /// Get current status query
    GetStatus { instance_id: InstanceId },
}

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

// Note: ractor::Message is automatically implemented for types that are
// Send + Sync + 'static via a blanket impl. Since all our fields are
// Send + Sync newtypes, the trait is already implemented.
