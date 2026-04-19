//! Actor message types for workflow instance actors.
//!
//! This module was moved from vo-actor/src/lib.rs as part of the
//! ADR-016/v2 module split refactoring.

use vo_types::{InstanceId, NodeName, SequenceNumber, TimerId, WorkflowName};

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
        signal_id: String,
        payload: crate::SignalPayload,
    },
    /// Request continue-as-new rollover to a new epoch (ADR-038).
    ContinueAsNew {
        instance_id: InstanceId,
        lineage_id: String,
        new_instance_id: InstanceId,
    },
}
