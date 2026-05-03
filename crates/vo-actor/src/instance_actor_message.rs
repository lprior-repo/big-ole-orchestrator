//! Instance actor messages for workflow instance lifecycle.

use vo_types::{InstanceId, SequenceNumber, TimerId, WorkflowName};

pub use crate::signals::NodeName;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceActorMessage {
    StartWorkflow {
        instance_id: InstanceId,
        workflow_name: WorkflowName,
        node_name: NodeName,
    },
    StepCompleted {
        instance_id: InstanceId,
        node_name: NodeName,
        sequence: SequenceNumber,
    },
    StepFailed {
        instance_id: InstanceId,
        node_name: NodeName,
        sequence: SequenceNumber,
        error: String,
    },
    TimerFired {
        instance_id: InstanceId,
        timer_id: TimerId,
    },
    CancelRequested { instance_id: InstanceId },
    GetStatus { instance_id: InstanceId },
}

impl InstanceActorMessage {
    #[must_use]
    pub fn new_start_workflow<N>(
        instance_id: InstanceId,
        workflow_name: WorkflowName,
        node_name: N,
    ) -> Self
    where
        N: Into<NodeName>,
    {
        Self::StartWorkflow {
            instance_id,
            workflow_name,
            node_name: node_name.into(),
        }
    }

    #[must_use]
    pub fn new_step_completed<N>(
        instance_id: InstanceId,
        node_name: N,
        sequence: SequenceNumber,
    ) -> Self
    where
        N: Into<NodeName>,
    {
        Self::StepCompleted {
            instance_id,
            node_name: node_name.into(),
            sequence,
        }
    }

    #[must_use]
    pub fn new_step_failed<N>(
        instance_id: InstanceId,
        node_name: N,
        sequence: SequenceNumber,
        error: String,
    ) -> Self
    where
        N: Into<NodeName>,
    {
        Self::StepFailed {
            instance_id,
            node_name: node_name.into(),
            sequence,
            error,
        }
    }

    #[must_use]
    pub fn new_timer_fired(instance_id: InstanceId, timer_id: TimerId) -> Self {
        Self::TimerFired {
            instance_id,
            timer_id,
        }
    }

    #[must_use]
    pub fn new_cancel_requested(instance_id: InstanceId) -> Self {
        Self::CancelRequested { instance_id }
    }

    #[must_use]
    pub fn new_get_status(instance_id: InstanceId) -> Self {
        Self::GetStatus { instance_id }
    }
}