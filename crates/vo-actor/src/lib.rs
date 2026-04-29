//! Actor framework for vo-engine.
//!
//! Provides the actor model implementation using the Ractor library.
//! Actors are the fundamental units of computation in the engine.

use bytes::Bytes;
use vo_types::InstanceId;
use vo_types::{SequenceNumber, TimerId, TimestampMs, WorkflowName};

/// Namespace identifier for workflow isolation.
pub type NamespaceId = String;

pub mod heartbeat {
    pub fn run_heartbeat_watcher() {}
}

pub mod actor_messages;
pub mod async_message_router;
pub mod db_writer;
pub mod fairness;
pub mod instance;
pub mod instance_registry;
pub mod lifecycle;
pub mod master;
pub mod message_router;
pub mod port;
pub mod probe;
pub mod reanimator;
pub mod semaphore;
pub mod signal_buffer;
pub mod signal_messages;
pub mod signals;
pub mod control_actor;
pub mod spawn_supervisor;

#[cfg(test)]
pub mod signal_buffer_tests;

#[cfg(test)]
pub mod instance_registry_tests;

#[cfg(test)]
pub mod vo_actor_comprehensive_tests;

// #[cfg(test)]
// pub mod replay_attack_tests;  // module file missing
pub mod timer_lifecycle;
pub mod timer_supervisor;
pub mod timer_supervisor_tests;
pub mod timers;

pub use master::{MasterOrchestrator, OrchestratorConfig};

#[derive(Debug, thiserror::Error)]
pub enum TerminateError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("failed: {0}")]
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowParadigm {
    Fsm,
    Dag,
    Procedural,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstancePhaseView {
    Replay,
    Live,
    Terminated,
}

/// Messages sent to the orchestrator actor.
/// Messages sent to the orchestrator actor.
#[derive(Debug)]
pub enum OrchestratorMsg {
    /// Start a new workflow instance
    StartWorkflow {
        namespace: NamespaceId,
        instance_id: InstanceId,
        workflow_type: String,
        paradigm: WorkflowParadigm,
        input: Bytes,
        reply: ractor::port::RpcReplyPort<Result<(), StartError>>,
    },
    ReserveWorkflowStart {
        namespace: NamespaceId,
        instance_id: InstanceId,
        workflow_type: String,
        paradigm: WorkflowParadigm,
        input: Bytes,
        reply: ractor::port::RpcReplyPort<Result<(), StartError>>,
    },
    CommitWorkflowStart {
        namespace: NamespaceId,
        instance_id: InstanceId,
        workflow_type: String,
        paradigm: WorkflowParadigm,
        input: Bytes,
        reply: ractor::port::RpcReplyPort<Result<(), StartError>>,
    },
    AbortWorkflowStart {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reply: ractor::port::RpcReplyPort<()>,
    },
    /// Get status of a workflow instance
    GetStatus {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reply: ractor::port::RpcReplyPort<Option<InstanceSnapshot>>,
    },
    /// Terminate a workflow instance
    Terminate {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reason: String,
        reply: ractor::port::RpcReplyPort<Result<(), TerminateError>>,
    },
    ReserveTerminate {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reason: String,
        reply: ractor::port::RpcReplyPort<Result<(), TerminateError>>,
    },
    CommitTerminate {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reason: String,
        reply: ractor::port::RpcReplyPort<Result<(), TerminateError>>,
    },
    AbortWorkflowTransition {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reply: ractor::port::RpcReplyPort<()>,
    },
    /// List all active workflow instances
    ListActive {
        reply: ractor::port::RpcReplyPort<Vec<InstanceSnapshot>>,
    },
    /// Compensate a completed workflow
    Compensate {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reply: ractor::port::RpcReplyPort<Result<(), CompensateError>>,
    },
    ReserveCompensate {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reply: ractor::port::RpcReplyPort<Result<(), CompensateError>>,
    },
    CommitCompensate {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reply: ractor::port::RpcReplyPort<Result<(), CompensateError>>,
    },
    /// Send a signal to a workflow instance
    Signal {
        namespace: NamespaceId,
        instance_id: InstanceId,
        signal_name: String,
        payload: Bytes,
        reply: ractor::port::RpcReplyPort<Result<(), SignalError>>,
    },
    ReserveSignal {
        namespace: NamespaceId,
        instance_id: InstanceId,
        signal_name: String,
        reply: ractor::port::RpcReplyPort<Result<(), SignalError>>,
    },
    CommitSignal {
        namespace: NamespaceId,
        instance_id: InstanceId,
        signal_name: String,
        payload: Bytes,
        reply: ractor::port::RpcReplyPort<Result<(), SignalError>>,
    },
}

/// Error type for signal operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SignalError {
    #[error("instance not found: {0}")]
    NotFound(String),
    #[error("signal failed: {0}")]
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompensateError {
    #[error("instance not found: {0}")]
    NotFound(String),
    #[error("compensation failed: {0}")]
    Failed(String),
}

/// Instance snapshot for status queries.
#[derive(Debug, Clone)]
pub struct InstanceSnapshot {
    pub instance_id: InstanceId,
    pub namespace: NamespaceId,
    pub workflow_type: String,
    pub paradigm: WorkflowParadigm,
    pub phase: InstancePhaseView,
    pub events_applied: u64,
}

#[cfg(test)]
mod signal_error_tests {
    use super::*;

    #[test]
    fn signal_error_variants_can_be_constructed() {
        let err = SignalError::NotFound("inst-1".to_string());
        assert!(matches!(err, SignalError::NotFound(msg) if msg == "inst-1"));

        let err = SignalError::Failed("timeout".to_string());
        assert!(matches!(err, SignalError::Failed(msg) if msg == "timeout"));
    }

    #[test]
    fn orchestrator_msg_signal_variant_exists() {
        fn _check(_msg: OrchestratorMsg) {
            if let OrchestratorMsg::Signal {
                namespace: _,
                instance_id: _,
                signal_name: _,
                payload: _,
                reply: _,
            } = _msg
            {}
        }
    }
}

#[cfg(test)]
mod terminate_error_tests {
    use super::*;

    #[test]
    fn terminate_error_variants_can_be_constructed() {
        let err_not_found = TerminateError::NotFound("wf-123".to_string());
        assert!(matches!(err_not_found, TerminateError::NotFound(msg) if msg == "wf-123"));

        let err_failed = TerminateError::Failed("crashed".to_string());
        assert!(matches!(err_failed, TerminateError::Failed(msg) if msg == "crashed"));
    }
}

pub use signal_messages::mock_signal_storage;
pub use signal_messages::mock_signal_storage::{MockSignalStorage, MockSignalWorkQueue};
pub use signal_messages::{
    AcceptResumeError, AcceptResumeOutcome, BinaryHash, CancelError, CancelRequested,
    ContinueAsNewError, InstanceResumed, LifecycleState, NodeName, ResumeError, RolloverState,
    SecretId, SignalAccepted, SignalName, SignalPayload, SignalStorage, SignalStorageError,
    SignalWorkQueue, SignalWorkQueueError, StateLookup, TestStateLookup, WaitKey,
    WorkflowCancelled, WorkflowContinued,
};

pub use control_actor::ControlActor;

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
        signal_id: SignalName,
        payload: crate::SignalPayload,
    },
}

// =============================================================================
// Constructor Functions - InstanceActorMessage
// =============================================================================

impl InstanceActorMessage {
    /// Creates a new `StartWorkflow` message.
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

    /// Creates a new `StepCompleted` message.
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

    /// Creates a new `StepFailed` message.
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

// Note: ractor::Message is automatically implemented for types that are
// Send + Sync + 'static via a blanket impl. Since all our fields are
// Send + Sync newtypes, the trait is already implemented.

// =============================================================================
// Unit Tests - Constructor Tests (InstanceActorMessage - 6 variants)
// =============================================================================

#[cfg(test)]
mod constructor_tests_instance_actor_message {
    use super::*;

    #[test]
    fn start_workflow_constructs_correctly_when_given_valid_votypes() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let workflow_name = WorkflowName::parse("deploy-prod").unwrap();
        let node_name = NodeName::parse("build-step").unwrap();

        let message = InstanceActorMessage::new_start_workflow(
            instance_id.clone(),
            workflow_name.clone(),
            node_name.clone(),
        );

        match &message {
            InstanceActorMessage::StartWorkflow {
                instance_id: id,
                workflow_name: wn,
                node_name: nn,
            } => {
                assert_eq!(id.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
                assert_eq!(wn.as_str(), "deploy-prod");
                assert_eq!(nn.as_str(), "build-step");
            }
            _ => panic!("Expected StartWorkflow variant"),
        }
    }

    #[test]
    fn step_completed_constructs_correctly_when_given_valid_votypes() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let node_name = NodeName::parse("compile-step").unwrap();
        let sequence = SequenceNumber::new_unchecked(1);

        let message = InstanceActorMessage::new_step_completed(
            instance_id.clone(),
            node_name.clone(),
            sequence,
        );

        match &message {
            InstanceActorMessage::StepCompleted {
                instance_id: id,
                node_name: nn,
                sequence: seq,
            } => {
                assert_eq!(id.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
                assert_eq!(nn.as_str(), "compile-step");
                assert_eq!(seq.as_u64(), 1);
            }
            _ => panic!("Expected StepCompleted variant"),
        }
    }

    #[test]
    fn step_failed_constructs_correctly_when_given_valid_votypes_and_error_string() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let node_name = NodeName::parse("compile-step").unwrap();
        let sequence = SequenceNumber::new_unchecked(42);
        let error = "connection timeout".to_string();

        let message = InstanceActorMessage::new_step_failed(
            instance_id.clone(),
            node_name.clone(),
            sequence,
            error.clone(),
        );

        match &message {
            InstanceActorMessage::StepFailed {
                instance_id: id,
                node_name: nn,
                sequence: seq,
                error: err,
            } => {
                assert_eq!(id.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
                assert_eq!(nn.as_str(), "compile-step");
                assert_eq!(seq.as_u64(), 42);
                assert_eq!(err, "connection timeout");
            }
            _ => panic!("Expected StepFailed variant"),
        }
    }

    #[test]
    fn timer_fired_constructs_correctly_when_given_valid_votypes() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let timer_id = TimerId::parse("timer-abc-123").unwrap();

        let message = InstanceActorMessage::new_timer_fired(instance_id.clone(), timer_id.clone());

        match &message {
            InstanceActorMessage::TimerFired {
                instance_id: id,
                timer_id: tid,
            } => {
                assert_eq!(id.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
                assert_eq!(tid.as_str(), "timer-abc-123");
            }
            _ => panic!("Expected TimerFired variant"),
        }
    }

    #[test]
    fn cancel_requested_constructs_correctly_when_given_valid_instance_id() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let message = InstanceActorMessage::new_cancel_requested(instance_id.clone());

        match &message {
            InstanceActorMessage::CancelRequested { instance_id: id } => {
                assert_eq!(id.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
            }
            _ => panic!("Expected CancelRequested variant"),
        }
    }

    #[test]
    fn get_status_constructs_correctly_when_given_valid_instance_id() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let message = InstanceActorMessage::new_get_status(instance_id.clone());

        match &message {
            InstanceActorMessage::GetStatus { instance_id: id } => {
                assert_eq!(id.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
            }
            _ => panic!("Expected GetStatus variant"),
        }
    }
}

// =============================================================================
// Unit Tests - Constructor Tests (ControlActorMessage - 2 variants)
// =============================================================================

#[cfg(test)]
mod constructor_tests_control_actor_message {
    use super::*;

    #[test]
    fn cancel_constructs_correctly_when_given_valid_instance_id() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let message = ControlActorMessage::new_cancel(instance_id.clone());

        match &message {
            ControlActorMessage::Cancel { instance_id: id } => {
                assert_eq!(id.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
            }
            _ => panic!("Expected Cancel variant"),
        }
    }

    #[test]
    fn resume_constructs_correctly_when_given_valid_instance_id() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let message = ControlActorMessage::new_resume(instance_id.clone());

        match &message {
            ControlActorMessage::Resume { instance_id: id } => {
                assert_eq!(id.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
            }
            _ => panic!("Expected Resume variant"),
        }
    }

    #[test]
    fn accept_and_resume_constructs_correctly() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let wait_key = crate::WaitKey::parse("approval-v2").unwrap();
        let payload = crate::SignalPayload::empty();
        let signal_name = SignalName::parse("sig-1").unwrap();
        let message = ControlActorMessage::new_accept_and_resume(
            instance_id.clone(),
            wait_key.clone(),
            signal_name.clone(),
            payload.clone(),
        );

        match &message {
            ControlActorMessage::AcceptAndResume {
                instance_id: id,
                wait_key: wk,
                signal_id,
                payload: p,
            } => {
                assert_eq!(id.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
                assert_eq!(wk.as_str(), "approval-v2");
                assert_eq!(signal_id.as_str(), "sig-1");
                assert!(p.is_empty());
            }
            _ => panic!("Expected AcceptAndResume variant"),
        }
    }
}

// =============================================================================
// Unit Tests - Debug Format (InstanceActorMessage - 6 variants)
// =============================================================================

#[cfg(test)]
mod debug_format_instance_actor_message {
    use super::*;

    #[test]
    fn start_workflow_debug_format_is_exact_string() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let workflow_name = WorkflowName::parse("deploy-prod").unwrap();
        let node_name = NodeName::parse("build-step").unwrap();
        let message =
            InstanceActorMessage::new_start_workflow(instance_id, workflow_name, node_name);

        let debug_str = format!("{:?}", message);
        assert_eq!(
            debug_str,
            "StartWorkflow { instance_id: InstanceId(\"01H5JYV4XHGSR2F8KZ9BWNRFMA\"), workflow_name: WorkflowName(\"deploy-prod\"), node_name: NodeName(\"build-step\") }"
        );
    }

    #[test]
    fn step_completed_debug_format_is_exact_string() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let node_name = NodeName::parse("compile-step").unwrap();
        let sequence = SequenceNumber::new_unchecked(1);
        let message = InstanceActorMessage::new_step_completed(instance_id, node_name, sequence);

        let debug_str = format!("{:?}", message);
        assert_eq!(
            debug_str,
            "StepCompleted { instance_id: InstanceId(\"01H5JYV4XHGSR2F8KZ9BWNRFMA\"), node_name: NodeName(\"compile-step\"), sequence: SequenceNumber(1) }"
        );
    }

    #[test]
    fn step_failed_debug_format_is_exact_string() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let node_name = NodeName::parse("compile-step").unwrap();
        let sequence = SequenceNumber::new_unchecked(42);
        let error = "connection timeout".to_string();
        let message =
            InstanceActorMessage::new_step_failed(instance_id, node_name, sequence, error);

        let debug_str = format!("{:?}", message);
        assert_eq!(
            debug_str,
            "StepFailed { instance_id: InstanceId(\"01H5JYV4XHGSR2F8KZ9BWNRFMA\"), node_name: NodeName(\"compile-step\"), sequence: SequenceNumber(42), error: \"connection timeout\" }"
        );
    }

    #[test]
    fn timer_fired_debug_format_is_exact_string() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let timer_id = TimerId::parse("timer-abc-123").unwrap();
        let message = InstanceActorMessage::new_timer_fired(instance_id, timer_id);

        let debug_str = format!("{:?}", message);
        assert_eq!(
            debug_str,
            "TimerFired { instance_id: InstanceId(\"01H5JYV4XHGSR2F8KZ9BWNRFMA\"), timer_id: TimerId(\"timer-abc-123\") }"
        );
    }

    #[test]
    fn cancel_requested_debug_format_is_exact_string() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let message = InstanceActorMessage::new_cancel_requested(instance_id);

        let debug_str = format!("{:?}", message);
        assert_eq!(
            debug_str,
            "CancelRequested { instance_id: InstanceId(\"01H5JYV4XHGSR2F8KZ9BWNRFMA\") }"
        );
    }

    #[test]
    fn get_status_debug_format_is_exact_string() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let message = InstanceActorMessage::new_get_status(instance_id);

        let debug_str = format!("{:?}", message);
        assert_eq!(
            debug_str,
            "GetStatus { instance_id: InstanceId(\"01H5JYV4XHGSR2F8KZ9BWNRFMA\") }"
        );
    }
}

// =============================================================================
// Unit Tests - Debug Format (ControlActorMessage - 2 variants)
// =============================================================================

#[cfg(test)]
mod debug_format_control_actor_message {
    use super::*;

    #[test]
    fn cancel_debug_format_is_exact_string() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let message = ControlActorMessage::new_cancel(instance_id);

        let debug_str = format!("{:?}", message);
        assert_eq!(
            debug_str,
            "Cancel { instance_id: InstanceId(\"01H5JYV4XHGSR2F8KZ9BWNRFMA\") }"
        );
    }

    #[test]
    fn resume_debug_format_is_exact_string() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let message = ControlActorMessage::new_resume(instance_id);

        let debug_str = format!("{:?}", message);
        assert_eq!(
            debug_str,
            "Resume { instance_id: InstanceId(\"01H5JYV4XHGSR2F8KZ9BWNRFMA\") }"
        );
    }

    #[test]
    fn accept_and_resume_debug_format_is_exact_string() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let wait_key = crate::WaitKey::parse("approval-v2").unwrap();
        let payload = crate::SignalPayload::empty();
        let signal_name = SignalName::parse("sig-1").unwrap();
        let message =
            ControlActorMessage::new_accept_and_resume(instance_id, wait_key, signal_name, payload);

        let debug_str = format!("{:?}", message);
        assert!(debug_str.contains("AcceptAndResume"));
        assert!(debug_str.contains("approval-v2"));
    }
}

// =============================================================================
// Unit Tests - Clone with Field-Level Verification (InstanceActorMessage)
// =============================================================================

#[cfg(test)]
mod clone_instance_actor_message {
    use super::*;

    #[test]
    fn start_workflow_clone_produces_bitwise_identical_copy() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let workflow_name = WorkflowName::parse("deploy-prod").unwrap();
        let node_name = NodeName::parse("build-step").unwrap();
        let message = InstanceActorMessage::new_start_workflow(
            instance_id.clone(),
            workflow_name.clone(),
            node_name.clone(),
        );

        let clone = message.clone();

        match (&message, &clone) {
            (
                InstanceActorMessage::StartWorkflow {
                    instance_id: id1,
                    workflow_name: wn1,
                    node_name: nn1,
                },
                InstanceActorMessage::StartWorkflow {
                    instance_id: id2,
                    workflow_name: wn2,
                    node_name: nn2,
                },
            ) => {
                assert_eq!(id1.as_str(), id2.as_str());
                assert_eq!(wn1.as_str(), wn2.as_str());
                assert_eq!(nn1.as_str(), nn2.as_str());
            }
            _ => panic!("Variants don't match"),
        }
        assert_eq!(clone, message);
    }

    #[test]
    fn step_completed_clone_produces_bitwise_identical_copy() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let node_name = NodeName::parse("compile-step").unwrap();
        let sequence = SequenceNumber::new_unchecked(1);
        let message = InstanceActorMessage::new_step_completed(
            instance_id.clone(),
            node_name.clone(),
            sequence,
        );

        let clone = message.clone();

        match (&message, &clone) {
            (
                InstanceActorMessage::StepCompleted {
                    instance_id: id1,
                    node_name: nn1,
                    sequence: seq1,
                },
                InstanceActorMessage::StepCompleted {
                    instance_id: id2,
                    node_name: nn2,
                    sequence: seq2,
                },
            ) => {
                assert_eq!(id1.as_str(), id2.as_str());
                assert_eq!(nn1.as_str(), nn2.as_str());
                assert_eq!(seq1.as_u64(), seq2.as_u64());
            }
            _ => panic!("Variants don't match"),
        }
        assert_eq!(clone, message);
    }

    #[test]
    fn step_failed_clone_produces_bitwise_identical_copy() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let node_name = NodeName::parse("compile-step").unwrap();
        let sequence = SequenceNumber::new_unchecked(42);
        let error = "connection timeout".to_string();
        let message = InstanceActorMessage::new_step_failed(
            instance_id.clone(),
            node_name.clone(),
            sequence,
            error.clone(),
        );

        let clone = message.clone();

        match (&message, &clone) {
            (
                InstanceActorMessage::StepFailed {
                    instance_id: id1,
                    node_name: nn1,
                    sequence: seq1,
                    error: e1,
                },
                InstanceActorMessage::StepFailed {
                    instance_id: id2,
                    node_name: nn2,
                    sequence: seq2,
                    error: e2,
                },
            ) => {
                assert_eq!(id1.as_str(), id2.as_str());
                assert_eq!(nn1.as_str(), nn2.as_str());
                assert_eq!(seq1.as_u64(), seq2.as_u64());
                assert_eq!(e1, e2);
            }
            _ => panic!("Variants don't match"),
        }
        assert_eq!(clone, message);
    }

    #[test]
    fn timer_fired_clone_produces_bitwise_identical_copy() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let timer_id = TimerId::parse("timer-abc-123").unwrap();
        let message = InstanceActorMessage::new_timer_fired(instance_id.clone(), timer_id.clone());

        let clone = message.clone();

        match (&message, &clone) {
            (
                InstanceActorMessage::TimerFired {
                    instance_id: id1,
                    timer_id: tid1,
                },
                InstanceActorMessage::TimerFired {
                    instance_id: id2,
                    timer_id: tid2,
                },
            ) => {
                assert_eq!(id1.as_str(), id2.as_str());
                assert_eq!(tid1.as_str(), tid2.as_str());
            }
            _ => panic!("Variants don't match"),
        }
        assert_eq!(clone, message);
    }

    #[test]
    fn cancel_requested_clone_produces_bitwise_identical_copy() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let message = InstanceActorMessage::new_cancel_requested(instance_id.clone());

        let clone = message.clone();

        match (&message, &clone) {
            (
                InstanceActorMessage::CancelRequested { instance_id: id1 },
                InstanceActorMessage::CancelRequested { instance_id: id2 },
            ) => {
                assert_eq!(id1.as_str(), id2.as_str());
            }
            _ => panic!("Variants don't match"),
        }
        assert_eq!(clone, message);
    }

    #[test]
    fn get_status_clone_produces_bitwise_identical_copy() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let message = InstanceActorMessage::new_get_status(instance_id.clone());

        let clone = message.clone();

        match (&message, &clone) {
            (
                InstanceActorMessage::GetStatus { instance_id: id1 },
                InstanceActorMessage::GetStatus { instance_id: id2 },
            ) => {
                assert_eq!(id1.as_str(), id2.as_str());
            }
            _ => panic!("Variants don't match"),
        }
        assert_eq!(clone, message);
    }
}

// =============================================================================
// Unit Tests - Clone with Field-Level Verification (ControlActorMessage)
// =============================================================================

#[cfg(test)]
mod clone_control_actor_message {
    use super::*;

    #[test]
    fn cancel_clone_produces_bitwise_identical_copy() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let message = ControlActorMessage::new_cancel(instance_id.clone());

        let clone = message.clone();

        match (&message, &clone) {
            (
                ControlActorMessage::Cancel { instance_id: id1 },
                ControlActorMessage::Cancel { instance_id: id2 },
            ) => {
                assert_eq!(id1.as_str(), id2.as_str());
            }
            _ => panic!("Variants don't match"),
        }
        assert_eq!(clone, message);
    }

    #[test]
    fn resume_clone_produces_bitwise_identical_copy() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let message = ControlActorMessage::new_resume(instance_id.clone());

        let clone = message.clone();

        match (&message, &clone) {
            (
                ControlActorMessage::Resume { instance_id: id1 },
                ControlActorMessage::Resume { instance_id: id2 },
            ) => {
                assert_eq!(id1.as_str(), id2.as_str());
            }
            _ => panic!("Variants don't match"),
        }
        assert_eq!(clone, message);
    }
}

// =============================================================================
// Unit Tests - PartialEq (InstanceActorMessage)
// =============================================================================

#[cfg(test)]
mod partial_eq_instance_actor_message {
    use super::*;

    #[test]
    fn partial_eq_returns_true_for_identical_values() {
        let instance_id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let workflow_name1 = WorkflowName::parse("deploy-prod").unwrap();
        let node_name1 = NodeName::parse("build-step").unwrap();

        let instance_id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let workflow_name2 = WorkflowName::parse("deploy-prod").unwrap();
        let node_name2 = NodeName::parse("build-step").unwrap();

        let msg1 =
            InstanceActorMessage::new_start_workflow(instance_id1, workflow_name1, node_name1);
        let msg2 =
            InstanceActorMessage::new_start_workflow(instance_id2, workflow_name2, node_name2);

        assert!(msg1 == msg2);
    }

    #[test]
    fn partial_eq_returns_false_for_different_variants() {
        let instance_id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let workflow_name1 = WorkflowName::parse("deploy-prod").unwrap();
        let node_name1 = NodeName::parse("build-step").unwrap();

        let instance_id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let node_name2 = NodeName::parse("compile-step").unwrap();
        let sequence2 = SequenceNumber::new_unchecked(1);

        let msg1 =
            InstanceActorMessage::new_start_workflow(instance_id1, workflow_name1, node_name1);
        let msg2 = InstanceActorMessage::new_step_completed(instance_id2, node_name2, sequence2);

        assert!(msg1 != msg2);
    }

    #[test]
    fn partial_eq_returns_false_for_same_variant_different_fields() {
        let instance_id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let workflow_name1 = WorkflowName::parse("deploy-prod").unwrap();
        let node_name1 = NodeName::parse("build-step").unwrap();

        let instance_id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();
        let workflow_name2 = WorkflowName::parse("deploy-prod").unwrap();
        let node_name2 = NodeName::parse("build-step").unwrap();

        let msg1 =
            InstanceActorMessage::new_start_workflow(instance_id1, workflow_name1, node_name1);
        let msg2 =
            InstanceActorMessage::new_start_workflow(instance_id2, workflow_name2, node_name2);

        assert!(msg1 != msg2);
    }
}

// =============================================================================
// Unit Tests - PartialEq (ControlActorMessage)
// =============================================================================

#[cfg(test)]
mod partial_eq_control_actor_message {
    use super::*;

    #[test]
    fn partial_eq_returns_true_for_identical_values() {
        let instance_id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let instance_id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        let msg1 = ControlActorMessage::new_cancel(instance_id1);
        let msg2 = ControlActorMessage::new_cancel(instance_id2);

        assert!(msg1 == msg2);
    }

    #[test]
    fn partial_eq_returns_false_for_different_variants() {
        let instance_id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let instance_id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        let msg1 = ControlActorMessage::new_cancel(instance_id1);
        let msg2 = ControlActorMessage::new_resume(instance_id2);

        assert!(msg1 != msg2);
    }

    #[test]
    fn partial_eq_returns_false_for_same_variant_different_fields() {
        let instance_id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let instance_id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();

        let msg1 = ControlActorMessage::new_cancel(instance_id1);
        let msg2 = ControlActorMessage::new_cancel(instance_id2);

        assert!(msg1 != msg2);
    }
}

// =============================================================================
// Unit Tests - Eq Properties (InstanceActorMessage)
// =============================================================================

#[cfg(test)]
mod eq_properties_instance_actor_message {
    use super::*;

    #[test]
    fn eq_is_reflexive() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let workflow_name = WorkflowName::parse("deploy-prod").unwrap();
        let node_name = NodeName::parse("build-step").unwrap();
        let msg = InstanceActorMessage::new_start_workflow(instance_id, workflow_name, node_name);

        assert!(msg == msg);
    }

    #[test]
    fn eq_is_symmetric() {
        let instance_id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let workflow_name1 = WorkflowName::parse("deploy-prod").unwrap();
        let node_name1 = NodeName::parse("build-step").unwrap();

        let instance_id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let workflow_name2 = WorkflowName::parse("deploy-prod").unwrap();
        let node_name2 = NodeName::parse("build-step").unwrap();

        let msg1 =
            InstanceActorMessage::new_start_workflow(instance_id1, workflow_name1, node_name1);
        let msg2 =
            InstanceActorMessage::new_start_workflow(instance_id2, workflow_name2, node_name2);

        assert!(msg1 == msg2);
        assert!(msg2 == msg1);
    }

    #[test]
    fn eq_is_transitive() {
        let instance_id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let workflow_name1 = WorkflowName::parse("deploy-prod").unwrap();
        let node_name1 = NodeName::parse("build-step").unwrap();

        let instance_id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let workflow_name2 = WorkflowName::parse("deploy-prod").unwrap();
        let node_name2 = NodeName::parse("build-step").unwrap();

        let instance_id3 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let workflow_name3 = WorkflowName::parse("deploy-prod").unwrap();
        let node_name3 = NodeName::parse("build-step").unwrap();

        let msg1 =
            InstanceActorMessage::new_start_workflow(instance_id1, workflow_name1, node_name1);
        let msg2 =
            InstanceActorMessage::new_start_workflow(instance_id2, workflow_name2, node_name2);
        let msg3 =
            InstanceActorMessage::new_start_workflow(instance_id3, workflow_name3, node_name3);

        assert!(msg1 == msg2);
        assert!(msg2 == msg3);
        assert!(msg1 == msg3);
    }
}

// =============================================================================
// Unit Tests - Eq Properties (ControlActorMessage)
// =============================================================================

#[cfg(test)]
mod eq_properties_control_actor_message {
    use super::*;

    #[test]
    fn eq_is_reflexive() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let msg = ControlActorMessage::new_cancel(instance_id);

        assert!(msg == msg);
    }

    #[test]
    fn eq_is_symmetric() {
        let instance_id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let instance_id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        let msg1 = ControlActorMessage::new_cancel(instance_id1);
        let msg2 = ControlActorMessage::new_cancel(instance_id2);

        assert!(msg1 == msg2);
        assert!(msg2 == msg1);
    }

    #[test]
    fn eq_is_transitive() {
        let instance_id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let instance_id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let instance_id3 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        let msg1 = ControlActorMessage::new_cancel(instance_id1);
        let msg2 = ControlActorMessage::new_cancel(instance_id2);
        let msg3 = ControlActorMessage::new_cancel(instance_id3);

        assert!(msg1 == msg2);
        assert!(msg2 == msg3);
        assert!(msg1 == msg3);
    }
}

// =============================================================================
// Unit Tests - Send + Sync Bounds (compile-time verification)
// =============================================================================

#[cfg(test)]
mod send_sync_bounds {
    use super::*;

    #[test]
    fn instance_actor_message_implements_send_bound() {
        fn assert_send<T: Send>() {}
        assert_send::<InstanceActorMessage>();
    }

    #[test]
    fn instance_actor_message_implements_sync_bound() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<InstanceActorMessage>();
    }

    #[test]
    fn control_actor_message_implements_send_bound() {
        fn assert_send<T: Send>() {}
        assert_send::<ControlActorMessage>();
    }

    #[test]
    fn control_actor_message_implements_sync_bound() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ControlActorMessage>();
    }
}

// =============================================================================
// Unit Tests - ractor::Message Trait (compile-time verification)
// =============================================================================

#[cfg(test)]
mod ractor_message_trait {
    use super::*;

    #[test]
    fn instance_actor_message_implements_ractor_message_trait() {
        fn assert_message<T: ractor::Message>() {}
        assert_message::<InstanceActorMessage>();
    }

    #[test]
    fn control_actor_message_implements_ractor_message_trait() {
        fn assert_message<T: ractor::Message>() {}
        assert_message::<ControlActorMessage>();
    }
}
// =============================================================================
// Workload Classes and Reserved Permit Budget (ADR-033)
// =============================================================================

pub use fairness::WorkloadClass;

/// Errors from actor start operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StartError {
    #[error("Budget exhausted for {class:?}: requested {requested}, available {available}")]
    BudgetExhaustion {
        class: WorkloadClass,
        requested: u32,
        available: u32,
    },
    #[error("Invalid config: {0}")]
    InvalidConfig(String),
    #[error("At capacity: {running}/{max} instances running")]
    AtCapacity { running: u32, max: u32 },
    #[error("Instance {0} already exists")]
    AlreadyExists(String),
    #[error("Spawn failed: {0}")]
    SpawnFailed(String),
    #[error("Ghost instance rejected: {0} has been reaped by zombie detection")]
    GhostInstance(String),
}

/// Reserved permit budget tracking per workload class.
/// Ensures each class maintains its reserved capacity per ADR-033.
#[derive(Debug, Clone)]
pub struct ReservedPermitBudget {
    max_per_class: u32,
    class_counts: std::collections::HashMap<WorkloadClass, u32>,
}

impl ReservedPermitBudget {
    /// Creates a new budget with the specified maximum per class.
    ///
    /// # Panics
    /// Panics if `max_per_class` is zero.
    #[must_use]
    pub fn new(max_per_class: u32) -> Self {
        assert!(max_per_class > 0, "max_per_class must be > 0");
        Self {
            max_per_class,
            class_counts: std::collections::HashMap::new(),
        }
    }

    /// Attempts to acquire a permit for the given class.
    ///
    /// # Errors
    /// Returns `StartError::BudgetExhaustion` if no permits available.
    pub fn try_acquire(&mut self, class: WorkloadClass) -> Result<(), StartError> {
        let current = self.class_counts.get(&class).copied().unwrap_or(0);
        if current >= self.max_per_class {
            return Err(StartError::BudgetExhaustion {
                class,
                requested: 1,
                available: self.max_per_class - current,
            });
        }
        *self.class_counts.entry(class).or_insert(0) += 1;
        Ok(())
    }

    /// Releases a permit for the given class.
    /// If count is already zero, this is a no-op.
    pub fn release(&mut self, class: WorkloadClass) {
        let count = self.class_counts.get(&class).copied().unwrap_or(0);
        if count == 0 {
            return;
        }
        self.class_counts.insert(class, count - 1);
    }

    /// Resets all class counts to zero.
    pub fn reset(&mut self) {
        self.class_counts.clear();
    }

    /// Returns the number of available permits for the given class.
    #[must_use]
    pub fn available(&self, class: WorkloadClass) -> u32 {
        let used = self.class_counts.get(&class).copied().unwrap_or(0);
        self.max_per_class.saturating_sub(used)
    }

    /// Returns true if the given class has no available permits.
    #[must_use]
    pub fn is_exhausted(&self, class: WorkloadClass) -> bool {
        self.available(class) == 0
    }
}

#[cfg(test)]
mod reserved_permit_budget_tests {
    use super::*;

    // =============================================================================
    // WorkloadClass Tests
    // =============================================================================

    mod workload_class_tests {
        use super::*;

        #[test]
        fn workload_class_variants_exist() {
            assert!(matches!(WorkloadClass::Recovery, WorkloadClass::Recovery));
            assert!(matches!(
                WorkloadClass::NewInstance,
                WorkloadClass::NewInstance
            ));
            assert!(matches!(WorkloadClass::Internal, WorkloadClass::Internal));
        }

        #[test]
        fn workload_class_debug_format() {
            assert_eq!(format!("{:?}", WorkloadClass::Recovery), "Recovery");
            assert_eq!(format!("{:?}", WorkloadClass::NewInstance), "NewInstance");
            assert_eq!(format!("{:?}", WorkloadClass::Internal), "Internal");
        }

        #[test]
        fn workload_class_eq() {
            assert_eq!(WorkloadClass::Recovery, WorkloadClass::Recovery);
            assert_eq!(WorkloadClass::NewInstance, WorkloadClass::NewInstance);
            assert_eq!(WorkloadClass::Internal, WorkloadClass::Internal);
            assert_ne!(WorkloadClass::Recovery, WorkloadClass::NewInstance);
        }

        #[test]
        fn workload_class_clone() {
            let a = WorkloadClass::Recovery;
            let b = a;
            assert_eq!(a, b);
        }

        #[test]
        fn workload_class_copy() {
            let a = WorkloadClass::Recovery;
            let b = a;
            assert_eq!(a, b);
        }
    }

    // =============================================================================
    // StartError Tests
    // =============================================================================

    mod start_error_tests {
        use super::*;

        #[test]
        fn budget_exhaustion_contains_fields() {
            let err = StartError::BudgetExhaustion {
                class: WorkloadClass::Recovery,
                requested: 1,
                available: 0,
            };
            assert!(matches!(err, StartError::BudgetExhaustion { .. }));
        }

        #[test]
        fn budget_exhaustion_display() {
            let err = StartError::BudgetExhaustion {
                class: WorkloadClass::Recovery,
                requested: 1,
                available: 0,
            };
            let display = format!("{}", err);
            assert!(display.contains("Recovery"));
            assert!(display.contains("requested"));
            assert!(display.contains("available"));
        }

        #[test]
        fn invalid_config_display() {
            let err = StartError::InvalidConfig("test error".to_string());
            let display = format!("{}", err);
            assert!(display.contains("Invalid config"));
            assert!(display.contains("test error"));
        }

        #[test]
        fn budget_exhaustion_partial_eq() {
            let err1 = StartError::BudgetExhaustion {
                class: WorkloadClass::Recovery,
                requested: 1,
                available: 0,
            };
            let err2 = StartError::BudgetExhaustion {
                class: WorkloadClass::Recovery,
                requested: 1,
                available: 0,
            };
            assert_eq!(err1, err2);
        }

        #[test]
        fn budget_exhaustion_different_classes_not_equal() {
            let err1 = StartError::BudgetExhaustion {
                class: WorkloadClass::Recovery,
                requested: 1,
                available: 0,
            };
            let err2 = StartError::BudgetExhaustion {
                class: WorkloadClass::NewInstance,
                requested: 1,
                available: 0,
            };
            assert_ne!(err1, err2);
        }
    }

    // =============================================================================
    // ReservedPermitBudget Tests
    // =============================================================================

    mod reserved_permit_budget_tests {
        use super::*;

        #[test]
        fn budget_creation() {
            let budget = ReservedPermitBudget::new(5);
            assert_eq!(budget.available(WorkloadClass::Recovery), 5);
            assert_eq!(budget.available(WorkloadClass::NewInstance), 5);
            assert_eq!(budget.available(WorkloadClass::Internal), 5);
        }

        #[test]
        fn budget_acquire_decrements_available() {
            let mut budget = ReservedPermitBudget::new(5);
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            assert_eq!(budget.available(WorkloadClass::Recovery), 4);
        }

        #[test]
        fn budget_acquire_multiple() {
            let mut budget = ReservedPermitBudget::new(5);
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            assert_eq!(budget.available(WorkloadClass::Recovery), 3);
        }

        #[test]
        fn budget_acquire_returns_err_when_exhausted() {
            let mut budget = ReservedPermitBudget::new(2);
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            let result = budget.try_acquire(WorkloadClass::Recovery);
            assert!(matches!(
                result,
                Err(StartError::BudgetExhaustion {
                    class: WorkloadClass::Recovery,
                    requested: 1,
                    available: 0,
                })
            ));
        }

        #[test]
        fn budget_release_increments_available() {
            let mut budget = ReservedPermitBudget::new(5);
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.release(WorkloadClass::Recovery);
            assert_eq!(budget.available(WorkloadClass::Recovery), 4);
        }

        #[test]
        fn budget_release_on_zero_is_noop() {
            let mut budget = ReservedPermitBudget::new(5);
            budget.release(WorkloadClass::Recovery);
            assert_eq!(budget.available(WorkloadClass::Recovery), 5);
        }

        #[test]
        fn budget_reset_clears_counts() {
            let mut budget = ReservedPermitBudget::new(5);
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.try_acquire(WorkloadClass::NewInstance).unwrap();
            budget.reset();
            assert_eq!(budget.available(WorkloadClass::Recovery), 5);
            assert_eq!(budget.available(WorkloadClass::NewInstance), 5);
        }

        #[test]
        fn budget_is_exhausted_false_when_available() {
            let budget = ReservedPermitBudget::new(5);
            assert!(!budget.is_exhausted(WorkloadClass::Recovery));
        }

        #[test]
        fn budget_is_exhausted_true_when_empty() {
            let mut budget = ReservedPermitBudget::new(2);
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            assert!(budget.is_exhausted(WorkloadClass::Recovery));
        }

        #[test]
        fn budget_classes_are_independent() {
            let mut budget = ReservedPermitBudget::new(3);
            // Exhaust Recovery
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            // Internal should still have capacity
            assert!(budget.try_acquire(WorkloadClass::Internal).is_ok());
            assert_eq!(budget.available(WorkloadClass::Internal), 2);
        }

        #[test]
        fn budget_exhaustion_error_contains_class_and_available() {
            let mut budget = ReservedPermitBudget::new(1);
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            let result = budget.try_acquire(WorkloadClass::Recovery);
            match result {
                Err(StartError::BudgetExhaustion {
                    class,
                    requested: _,
                    available,
                }) => {
                    assert_eq!(class, WorkloadClass::Recovery);
                    assert_eq!(available, 0);
                }
                _ => panic!("Expected BudgetExhaustion error"),
            }
        }
    }
}
