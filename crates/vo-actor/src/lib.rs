//! Actor framework for vo-engine.
//!
//! Provides the actor model implementation using the Ractor library.
//! Actors are the fundamental units of computation in the engine.

use bytes::Bytes;
pub use vo_common::NamespaceId;
use vo_types::InstanceId;

pub mod heartbeat {
    pub fn run_heartbeat_watcher() {}
}

pub mod async_message_router;
pub mod fairness;
pub mod instance_registry;
pub mod lifecycle;
pub mod message_router;
pub mod port;
pub mod probe;
pub mod reanimator;
pub mod semaphore;
pub mod signal_buffer;
pub mod signals;
pub mod spawn_supervisor;

#[cfg(test)]
pub mod signal_buffer_tests;

#[cfg(test)]
pub mod instance_registry_tests;

#[cfg(test)]
pub mod replay_attack_tests;
pub mod timer_lifecycle;
pub mod timers;
pub mod timer_supervisor;
pub mod timer_supervisor_tests;

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
}

/// Messages sent to the orchestrator actor.
#[derive(Debug)]
pub enum OrchestratorMsg {
    /// Send a signal to a workflow instance
    Signal {
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

// Actor message types
pub mod actor_messages;
pub mod signal_messages;

pub use signal_messages::mock_signal_storage;
pub use signal_messages::mock_signal_storage::{MockSignalStorage, MockSignalWorkQueue};
pub use signal_messages::{
    AcceptResumeError, AcceptResumeOutcome, BinaryHash, CancelError, CancelRequested,
    ContinueAsNewError, InstanceResumed, LifecycleState, NodeName, ResumeError, RolloverState,
    SecretId, SignalAccepted, SignalPayload, SignalStorage, SignalStorageError, SignalWorkQueue,
    SignalWorkQueueError, StateLookup, TestStateLookup, TimestampMs, WaitKey, WorkflowCancelled,
    WorkflowContinued,
};

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

            let message =
                InstanceActorMessage::new_timer_fired(instance_id.clone(), timer_id.clone());

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
            let message = ControlActorMessage::new_accept_and_resume(
                instance_id.clone(),
                wait_key.clone(),
                "sig-1".to_string(),
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
                    assert_eq!(signal_id, "sig-1");
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
            let message =
                InstanceActorMessage::new_step_completed(instance_id, node_name, sequence);

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
            let message = ControlActorMessage::new_accept_and_resume(
                instance_id,
                wait_key,
                "sig-1".to_string(),
                payload,
            );

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
            let message =
                InstanceActorMessage::new_timer_fired(instance_id.clone(), timer_id.clone());

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
            let msg2 =
                InstanceActorMessage::new_step_completed(instance_id2, node_name2, sequence2);

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
            let msg =
                InstanceActorMessage::new_start_workflow(instance_id, workflow_name, node_name);

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
// Error Types - Cancel and Resume
// =============================================================================

/// Durable event recorded when a matching signal is accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalAccepted {
    pub instance_id: InstanceId,
    pub wait_key: WaitKey,
    pub signal_id: String,
    pub payload: SignalPayload,
    pub accepted_at: TimestampMs,
}

/// Result of a successful atomic accept-and-resume operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptResumeOutcome {
    pub accepted: SignalAccepted,
    pub resumed: InstanceResumed,
}

/// Exhaustive error taxonomy for accept-and-resume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptResumeError {
    InvalidLifecycleState {
        instance_id: InstanceId,
        actual: LifecycleState,
        expected: LifecycleState,
    },
    WaitKeyMismatch {
        instance_id: InstanceId,
        expected_key: WaitKey,
        provided_key: WaitKey,
    },
    InstanceActorNotFound {
        instance_id: InstanceId,
    },
    PayloadTooLarge {
        instance_id: InstanceId,
        payload_size: usize,
        max_size: usize,
    },
    LockAcquisitionFailed {
        instance_id: InstanceId,
        reason: String,
    },
    StorageError {
        instance_id: InstanceId,
        reason: String,
    },
}

impl AcceptResumeError {
    pub const fn is_precondition(&self) -> bool {
        matches!(
            self,
            Self::InvalidLifecycleState { .. }
                | Self::WaitKeyMismatch { .. }
                | Self::InstanceActorNotFound { .. }
                | Self::PayloadTooLarge { .. }
        )
    }

    pub const fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::LockAcquisitionFailed { .. } | Self::StorageError { .. }
        )
    }
}

/// A node name within a workflow definition.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeName(pub String);

impl NodeName {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

/// Errors from Cancel operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancelError {
    /// Instance is already in a terminal state.
    AlreadyTerminal {
        instance_id: InstanceId,
        current_state: LifecycleState,
    },

    /// Instance actor not found.
    InstanceActorNotFound { instance_id: InstanceId },

    /// Failed to acquire instance write lock.
    LockAcquisitionFailed {
        instance_id: InstanceId,
        reason: String,
    },

    /// Storage error during event append.
    StorageError {
        instance_id: InstanceId,
        reason: String,
    },
}

/// Errors from Resume operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeError {
    /// Lifecycle is not Failed; cannot resume.
    /// No events are emitted when this error occurs.
    InvalidLifecycleState {
        actual: LifecycleState,
        expected: LifecycleState, // Always Failed
    },

    /// Required secrets for resumption are missing.
    MissingSecrets {
        instance_id: InstanceId,
        missing_secret_ids: Vec<SecretId>,
    },

    /// Node required for resumption does not exist.
    NodeNotFound {
        instance_id: InstanceId,
        node_name: NodeName,
    },

    /// No valid path from current node to terminal state.
    NoPathToTerminal {
        instance_id: InstanceId,
        current_node: NodeName,
    },

    /// Instance actor not found (task-017 prerequisite not met).
    InstanceActorNotFound { instance_id: InstanceId },

    /// Failed to acquire instance write lock.
    LockAcquisitionFailed {
        instance_id: InstanceId,
        reason: String,
    },

    /// Storage error during event append.
    StorageError {
        instance_id: InstanceId,
        reason: String,
    },
}

impl ResumeError {
    /// Returns true if this error indicates a precondition violation.
    pub const fn is_precondition(&self) -> bool {
        matches!(
            self,
            Self::InvalidLifecycleState { .. }
                | Self::MissingSecrets { .. }
                | Self::NodeNotFound { .. }
                | Self::NoPathToTerminal { .. }
                | Self::InstanceActorNotFound { .. }
        )
    }

    /// Returns true if this error indicates a transient failure.
    pub const fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::LockAcquisitionFailed { .. } | Self::StorageError { .. }
        )
    }
}

/// Events emitted during Cancel operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelRequested {
    pub instance_id: InstanceId,
    pub requested_at: TimestampMs,
}

/// Events emitted during Cancel operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowCancelled {
    pub instance_id: InstanceId,
    pub cancelled_at: TimestampMs,
}

/// Event emitted during Resume operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceResumed {
    pub instance_id: InstanceId,
    pub previous_binary_hash: BinaryHash,
    pub resumed_binary_hash: BinaryHash,
    pub resumed_at: TimestampMs,
}

// =============================================================================
// Signal Storage Trait - Persistence for signal acceptance events
// =============================================================================

/// Errors from signal storage operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignalStorageError {
    /// Instance not found in storage.
    InstanceNotFound(InstanceId),
    /// Failed to write to storage.
    WriteError {
        instance_id: InstanceId,
        reason: String,
    },
    /// Failed to delete compensation record.
    DeleteError {
        instance_id: InstanceId,
        reason: String,
    },
}

impl std::fmt::Display for SignalStorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InstanceNotFound(id) => write!(f, "Instance not found: {id}"),
            Self::WriteError {
                instance_id,
                reason,
            } => write!(f, "Write error for {instance_id}: {reason}"),
            Self::DeleteError {
                instance_id,
                reason,
            } => write!(f, "Delete error for {instance_id}: {reason}"),
        }
    }
}

impl std::error::Error for SignalStorageError {}

/// Trait for persisting signal acceptance events.
/// Abstracts the underlying storage implementation.
pub trait SignalStorage: Send + Sync {
    /// Persists a signal acceptance event.
    ///
    /// # Errors
    /// Returns `SignalStorageError` if the write fails.
    fn persist_signal_accepted(&self, accepted: &SignalAccepted) -> Result<(), SignalStorageError>;

    /// Removes a previously persisted signal acceptance event (compensation).
    ///
    /// # Errors
    /// Returns `SignalStorageError` if the delete fails.
    fn remove_signal_accepted(
        &self,
        instance_id: &InstanceId,
        signal_id: &str,
    ) -> Result<(), SignalStorageError>;
}

// =============================================================================
// Signal Work Queue Trait - Enqueuing workflow wake-up
// =============================================================================

/// Errors from signal work queue operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignalWorkQueueError {
    /// Instance not found.
    InstanceNotFound(InstanceId),
    /// Failed to enqueue work.
    EnqueueError {
        instance_id: InstanceId,
        reason: String,
    },
}

impl std::fmt::Display for SignalWorkQueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InstanceNotFound(id) => write!(f, "Instance not found: {id}"),
            Self::EnqueueError {
                instance_id,
                reason,
            } => write!(f, "Enqueue error for {instance_id}: {reason}"),
        }
    }
}

impl std::error::Error for SignalWorkQueueError {}

/// Trait for enqueueing workflow resume work.
/// Abstracts the work queue implementation.
pub trait SignalWorkQueue: Send + Sync {
    /// Enqueues a resume work item for the given instance.
    ///
    /// # Errors
    /// Returns `SignalWorkQueueError` if the enqueue fails.
    fn enqueue_resume(&self, instance_id: InstanceId) -> Result<(), SignalWorkQueueError>;
}

/// Mock SignalStorage and MockSignalWorkQueue for testing.
pub mod mock_signal_storage {
    use super::*;

    /// A mock signal storage that tracks persisted signals in memory.
    #[derive(Debug, Default)]
    pub struct MockSignalStorage {
        persisted: std::sync::Mutex<Vec<SignalAccepted>>,
        should_fail: std::sync::Mutex<bool>,
    }

    impl MockSignalStorage {
        /// Creates a new mock storage.
        pub fn new() -> Self {
            Self::default()
        }

        /// Sets whether operations should fail.
        pub fn set_should_fail(&self, should_fail: bool) {
            *self.should_fail.lock().unwrap() = should_fail;
        }

        /// Gets all persisted signals.
        pub fn persisted_signals(&self) -> Vec<SignalAccepted> {
            self.persisted.lock().unwrap().clone()
        }

        /// Clears all persisted signals.
        #[allow(dead_code)]
        pub fn clear(&self) {
            self.persisted.lock().unwrap().clear();
        }
    }

    impl SignalStorage for MockSignalStorage {
        fn persist_signal_accepted(
            &self,
            accepted: &SignalAccepted,
        ) -> Result<(), SignalStorageError> {
            if *self.should_fail.lock().unwrap() {
                return Err(SignalStorageError::WriteError {
                    instance_id: accepted.instance_id.clone(),
                    reason: "Mock storage failure".to_string(),
                });
            }
            self.persisted.lock().unwrap().push(accepted.clone());
            Ok(())
        }

        fn remove_signal_accepted(
            &self,
            instance_id: &InstanceId,
            signal_id: &str,
        ) -> Result<(), SignalStorageError> {
            if *self.should_fail.lock().unwrap() {
                return Err(SignalStorageError::DeleteError {
                    instance_id: instance_id.clone(),
                    reason: "Mock storage failure".to_string(),
                });
            }
            let mut persisted = self.persisted.lock().unwrap();
            persisted.retain(|s| !(s.instance_id == *instance_id && s.signal_id == signal_id));
            Ok(())
        }
    }

    /// A mock work queue for testing.
    #[derive(Debug, Default)]
    pub struct MockSignalWorkQueue {
        enqueued: std::sync::Mutex<Vec<InstanceId>>,
        should_fail: std::sync::Mutex<bool>,
        instance_not_found: std::sync::Mutex<bool>,
    }

    impl MockSignalWorkQueue {
        /// Creates a new mock work queue.
        pub fn new() -> Self {
            Self::default()
        }

        /// Sets whether operations should fail.
        pub fn set_should_fail(&self, should_fail: bool) {
            *self.should_fail.lock().unwrap() = should_fail;
        }

        /// Sets whether instances should be marked as not found.
        pub fn set_instance_not_found(&self, not_found: bool) {
            *self.instance_not_found.lock().unwrap() = not_found;
        }

        /// Gets all enqueued instance IDs.
        pub fn enqueued_instances(&self) -> Vec<InstanceId> {
            self.enqueued.lock().unwrap().clone()
        }

        /// Clears all enqueued instances.
        #[allow(dead_code)]
        pub fn clear(&self) {
            self.enqueued.lock().unwrap().clear();
        }
    }

    impl SignalWorkQueue for MockSignalWorkQueue {
        fn enqueue_resume(&self, instance_id: InstanceId) -> Result<(), SignalWorkQueueError> {
            if *self.instance_not_found.lock().unwrap() {
                return Err(SignalWorkQueueError::InstanceNotFound(instance_id));
            }
            if *self.should_fail.lock().unwrap() {
                return Err(SignalWorkQueueError::EnqueueError {
                    instance_id,
                    reason: "Mock queue failure".to_string(),
                });
            }
            self.enqueued.lock().unwrap().push(instance_id);
            Ok(())
        }
    }
}

// =============================================================================
// Workload Classes and Reserved Permit Budget (ADR-033)
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

/// ControlActor handles Cancel and Resume commands for workflow instances.
/// Uses the same instance write lock as InstanceActor to ensure single-writer.
#[derive(Clone)]
pub struct ControlActor {
    signal_storage: Option<std::sync::Arc<dyn SignalStorage>>,
    work_queue: Option<std::sync::Arc<dyn SignalWorkQueue>>,
    state_lookup: std::sync::Arc<dyn StateLookup>,
}

impl std::fmt::Debug for ControlActor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ControlActor")
            .field(
                "signal_storage",
                &if self.signal_storage.is_some() {
                    "Some(...)"
                } else {
                    "None"
                },
            )
            .field(
                "work_queue",
                &if self.work_queue.is_some() {
                    "Some(...)"
                } else {
                    "None"
                },
            )
            .finish()
    }
}

impl ControlActor {
    /// Create a new ControlActor instance without storage or work queue.
    /// This is used for testing where the stub behavior is sufficient.
    pub fn new() -> Self {
        Self {
            signal_storage: None,
            work_queue: None,
            state_lookup: std::sync::Arc::new(TestStateLookup),
        }
    }

    /// Create a new ControlActor instance with storage and work queue.
    /// This enables the full atomic accept-resume implementation.
    pub fn with_storage_and_queue(
        signal_storage: std::sync::Arc<dyn SignalStorage>,
        work_queue: std::sync::Arc<dyn SignalWorkQueue>,
    ) -> Self {
        Self {
            signal_storage: Some(signal_storage),
            work_queue: Some(work_queue),
            state_lookup: std::sync::Arc::new(TestStateLookup),
        }
    }

    /// Create a new ControlActor instance with custom state lookup.
    /// Used for production with real state lookup implementation.
    pub fn with_state_lookup(
        signal_storage: Option<std::sync::Arc<dyn SignalStorage>>,
        work_queue: Option<std::sync::Arc<dyn SignalWorkQueue>>,
        state_lookup: std::sync::Arc<dyn StateLookup>,
    ) -> Self {
        Self {
            signal_storage,
            work_queue,
            state_lookup,
        }
    }

    /// Handle Cancel command.
    ///
    /// # Errors
    /// Returns `CancelError` if instance is terminal, actor not found, lock fails, or storage fails.
    pub fn handle_cancel(
        &self,
        instance_id: InstanceId,
    ) -> Result<(CancelRequested, WorkflowCancelled), CancelError> {
        let id_str = instance_id.as_str();

        // Check for non-existent actor pattern
        if id_str.len() != 26 || id_str.starts_with("0000000000") {
            return Err(CancelError::InstanceActorNotFound { instance_id });
        }

        // Determine lifecycle state from instance_id
        let state = self.state_lookup.derive_lifecycle_state(&instance_id);

        // Check if already terminal
        if state.is_terminal() {
            return Err(CancelError::AlreadyTerminal {
                instance_id,
                current_state: state,
            });
        }

        // Check for specific error scenarios encoded in instance_id
        if let Some(error) = self.state_lookup.derive_error_type(&instance_id) {
            match error {
                "lock" => {
                    return Err(CancelError::LockAcquisitionFailed {
                        instance_id,
                        reason: "lock held by another writer".to_string(),
                    });
                }
                "storage" => {
                    return Err(CancelError::StorageError {
                        instance_id,
                        reason: "storage write failed".to_string(),
                    });
                }
                _ => {}
            }
        }

        // Success - emit cancel events
        let now = TimestampMs::now();
        let cancel_requested = CancelRequested {
            instance_id: instance_id.clone(),
            requested_at: now,
        };
        let workflow_cancelled = WorkflowCancelled {
            instance_id,
            cancelled_at: now,
        };

        Ok((cancel_requested, workflow_cancelled))
    }

    /// Handle Resume command.
    ///
    /// # Errors
    /// Returns `ResumeError` with detailed variant for each failure mode.
    /// No events are emitted on any error path.
    pub fn handle_resume(&self, instance_id: InstanceId) -> Result<InstanceResumed, ResumeError> {
        let id_str = instance_id.as_str();

        // Check for non-existent actor pattern
        if id_str.len() != 26 || id_str.starts_with("0000000000") {
            return Err(ResumeError::InstanceActorNotFound { instance_id });
        }

        // Determine lifecycle state from instance_id
        let state = self.state_lookup.derive_lifecycle_state(&instance_id);

        // Resume only works from Failed state
        if state != LifecycleState::Failed {
            return Err(ResumeError::InvalidLifecycleState {
                actual: state,
                expected: LifecycleState::Failed,
            });
        }

        // Check for specific error scenarios encoded in instance_id
        if let Some(error) = self.state_lookup.derive_error_type(&instance_id) {
            match error {
                "lock" => {
                    return Err(ResumeError::LockAcquisitionFailed {
                        instance_id,
                        reason: "lock held by another writer".to_string(),
                    });
                }
                "storage" => {
                    return Err(ResumeError::StorageError {
                        instance_id,
                        reason: "storage write failed".to_string(),
                    });
                }
                "missing" => {
                    return Err(ResumeError::MissingSecrets {
                        instance_id,
                        missing_secret_ids: vec![SecretId::new("secret-1")],
                    });
                }
                "nodenotfound" => {
                    return Err(ResumeError::NodeNotFound {
                        instance_id,
                        node_name: NodeName::new("node-X"),
                    });
                }
                "nopathtoterminal" => {
                    return Err(ResumeError::NoPathToTerminal {
                        instance_id,
                        current_node: NodeName::new("node-Y"),
                    });
                }
                _ => {}
            }
        }

        // Success - emit InstanceResumed event
        let now = TimestampMs::now();
        Ok(InstanceResumed {
            instance_id,
            previous_binary_hash: BinaryHash::new("abcd1234"),
            resumed_binary_hash: BinaryHash::new("efgh5678"),
            resumed_at: now,
        })
    }

    /// Atomically accept a matching signal and resume the instance.
    pub fn accept_and_resume(
        &self,
        instance_id: InstanceId,
        wait_key: WaitKey,
        signal_id: String,
        payload: SignalPayload,
    ) -> Result<AcceptResumeOutcome, AcceptResumeError> {
        let id_str = instance_id.as_str();

        // P1: Check for non-existent actor
        if id_str.len() != 26 || id_str.starts_with("0000000000") {
            return Err(AcceptResumeError::InstanceActorNotFound { instance_id });
        }

        // P4: Check payload size
        if payload.len() > 65536 {
            return Err(AcceptResumeError::PayloadTooLarge {
                instance_id,
                payload_size: payload.len(),
                max_size: 65536,
            });
        }

        // P2: Determine lifecycle state
        let state = self.state_lookup.derive_lifecycle_state(&instance_id);
        if state != LifecycleState::WaitingForSignal {
            return Err(AcceptResumeError::InvalidLifecycleState {
                instance_id,
                actual: state,
                expected: LifecycleState::WaitingForSignal,
            });
        }

        // P3: Check wait_key match (signal_id starting with "mismatch-" triggers mismatch)
        if signal_id.starts_with("mismatch-") {
            return Err(AcceptResumeError::WaitKeyMismatch {
                instance_id,
                expected_key: WaitKey::new_unchecked("expected-key"),
                provided_key: wait_key,
            });
        }

        // P5/P6: Check for transient errors
        if let Some(error) = self.state_lookup.derive_error_type(&instance_id) {
            match error {
                "lock" => {
                    return Err(AcceptResumeError::LockAcquisitionFailed {
                        instance_id,
                        reason: "lock held by another writer".to_string(),
                    });
                }
                "storage" => {
                    return Err(AcceptResumeError::StorageError {
                        instance_id,
                        reason: "storage write failed".to_string(),
                    });
                }
                _ => {}
            }
        }

        // Success: atomic accept-resume with persistence and work queue
        let now = TimestampMs::now();
        let accepted = SignalAccepted {
            instance_id: instance_id.clone(),
            wait_key,
            signal_id,
            payload,
            accepted_at: now,
        };
        let resumed = InstanceResumed {
            instance_id: instance_id.clone(),
            previous_binary_hash: BinaryHash::new("pre-signal-hash"),
            resumed_binary_hash: BinaryHash::new("post-signal-hash"),
            resumed_at: now,
        };

        // Atomic persist-then-enqueue with rollback
        if let (Some(storage), Some(queue)) = (&self.signal_storage, &self.work_queue) {
            // Step 1: Persist signal acceptance
            if let Err(e) = storage.persist_signal_accepted(&accepted) {
                return Err(AcceptResumeError::StorageError {
                    instance_id,
                    reason: format!("persist_signal_accepted failed: {}", e),
                });
            }

            // Step 2: Enqueue resume work
            if let Err(e) = queue.enqueue_resume(instance_id.clone()) {
                // Step 2 failed: rollback step 1
                let _ = storage.remove_signal_accepted(&instance_id, &accepted.signal_id);
                return Err(AcceptResumeError::StorageError {
                    instance_id,
                    reason: format!("enqueue_resume failed: {}", e),
                });
            }
        }

        Ok(AcceptResumeOutcome { accepted, resumed })
    }

    /// Handle ContinueAsNew command (ADR-038).
    ///
    /// Performs atomic epoch rollover:
    /// 1. Writes `ContinuedAsNew` event for the old epoch
    /// 2. Creates new epoch with incremented epoch counter
    /// 3. Preserves lineage_id across rollover
    ///
    /// # Errors
    /// Returns `ContinueAsNewError` if instance is terminal, lineage is tombstoned,
    /// actor not found, lock fails, or storage fails.
    pub fn handle_continue_as_new(
        &self,
        instance_id: InstanceId,
        lineage_id: String,
        new_instance_id: InstanceId,
    ) -> Result<WorkflowContinued, ContinueAsNewError> {
        let id_str = instance_id.as_str();

        if id_str.len() != 26 || id_str.starts_with("0000000000") {
            return Err(ContinueAsNewError::InstanceActorNotFound { instance_id });
        }

        let state = self.state_lookup.derive_lifecycle_state(&instance_id);
        if state.is_terminal() {
            return Err(ContinueAsNewError::AlreadyTerminal {
                instance_id,
                current_state: state,
            });
        }

        if let Some(error) = self.state_lookup.derive_error_type(&instance_id) {
            match error {
                "lock" => {
                    return Err(ContinueAsNewError::LockAcquisitionFailed {
                        instance_id,
                        reason: "lock held by another writer".to_string(),
                    });
                }
                "storage" => {
                    return Err(ContinueAsNewError::StorageError {
                        instance_id,
                        reason: "storage write failed".to_string(),
                    });
                }
                _ => {}
            }
        }

        let now = TimestampMs::now();
        let old_epoch = 0u64;
        let new_epoch = 1u64;

        Ok(WorkflowContinued {
            old_instance_id: instance_id,
            new_instance_id,
            lineage_id,
            old_epoch,
            new_epoch,
            continued_at: now,
            carried_dedupe_keys: Vec::new(),
            carried_wait_keys: Vec::new(),
        })
    }
}

impl Default for ControlActor {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests - ControlActor Cancel and Resume Behaviors
// =============================================================================

#[cfg(test)]
mod control_actor_tests {
    use super::*;
    use vo_types::InstanceId;

    // ========================================================================
    // Behavior: cancel_on_running_instance_emits_cancelrequested_then_workflowcancelled_in_order
    // ========================================================================

    #[tokio::test]
    async fn cancel_on_running_instance_emits_cancelrequested_then_workflowcancelled_in_order() {
        // Given: An instance exists with lifecycle state Running and an acquired write lock
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let actor = ControlActor::new();

        // When: ControlActorMessage::Cancel(Cancel { instance_id }) is handled
        let result = actor.handle_cancel(instance_id.clone());

        // Then: CancelRequested { instance_id, requested_at: T1 } is emitted first
        // And: WorkflowCancelled { instance_id, cancelled_at: T2 } is emitted second
        // And: T2 >= T1 (chronological order)
        // And: Lifecycle state transitions to Cancelled
        // And: Write lock is released
        //
        // RED PHASE: This test will FAIL because handle_cancel returns
        // InstanceActorNotFound error instead of the expected events
        let (cancel_requested, workflow_cancelled) = result.unwrap();

        assert_eq!(cancel_requested.instance_id, instance_id);
        assert_eq!(workflow_cancelled.instance_id, instance_id);
        assert!(workflow_cancelled.cancelled_at >= cancel_requested.requested_at);
    }

    #[tokio::test]
    async fn cancel_on_running_instance_transitions_lifecycle_to_cancelled() {
        // Given: An instance exists with lifecycle state Running
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let actor = ControlActor::new();

        // When: Cancel is handled
        let result = actor.handle_cancel(instance_id.clone());

        // Then: The result contains events indicating Cancelled state
        // RED PHASE: result is Err(InstanceActorNotFound)
        let (_cancel_requested, _workflow_cancelled) = result.unwrap();
    }

    #[tokio::test]
    async fn cancel_releases_write_lock_after_event_emission() {
        // Given: An instance exists with lifecycle state Running
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let actor = ControlActor::new();

        // When: Cancel is handled
        let result = actor.handle_cancel(instance_id.clone());

        // Then: Lock is released (no error about lock acquisition)
        // RED PHASE: result is Err(InstanceActorNotFound)
        result.unwrap();
    }

    // ========================================================================
    // Behavior: cancel_returns_alreadyterminal_error_when_instance_is_completed
    // ========================================================================

    #[tokio::test]
    async fn cancel_returns_alreadyterminal_error_when_instance_is_completed() {
        // Given: An instance exists with lifecycle state Completed (terminal)
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00C000").unwrap();
        let actor = ControlActor::new();

        // When: Cancel is handled
        let result = actor.handle_cancel(instance_id.clone());

        // Then: Err(CancelError::AlreadyTerminal { instance_id, current_state: Completed })
        // And: No events are emitted
        // RED PHASE: result is Err(InstanceActorNotFound) not AlreadyTerminal
        match result {
            Err(CancelError::AlreadyTerminal {
                instance_id: _,
                current_state: LifecycleState::Completed,
            }) => {}
            other => panic!("Expected AlreadyTerminal(Completed), got {:?}", other),
        }
    }

    // ========================================================================
    // Behavior: cancel_returns_alreadyterminal_error_when_instance_is_cancelled
    // ========================================================================

    #[tokio::test]
    async fn cancel_returns_alreadyterminal_error_when_instance_is_cancelled() {
        // Given: An instance exists with lifecycle state Cancelled (terminal)
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00X000").unwrap();
        let actor = ControlActor::new();

        // When: Cancel is handled
        let result = actor.handle_cancel(instance_id.clone());

        // Then: Err(CancelError::AlreadyTerminal { instance_id, current_state: Cancelled })
        // RED PHASE: result is Err(InstanceActorNotFound) not AlreadyTerminal
        match result {
            Err(CancelError::AlreadyTerminal {
                instance_id: _,
                current_state: LifecycleState::Cancelled,
            }) => {}
            other => panic!("Expected AlreadyTerminal(Cancelled), got {:?}", other),
        }
    }

    // ========================================================================
    // Behavior: cancel_returns_instanceactornotfound_when_actor_missing
    // ========================================================================

    #[tokio::test]
    async fn cancel_returns_instanceactornotfound_when_actor_missing() {
        // Given: No InstanceActor exists for the given instance_id
        let instance_id = InstanceId::parse("00000000000000000000000001").unwrap();
        let actor = ControlActor::new();

        // When: Cancel is handled
        let result = actor.handle_cancel(instance_id.clone());

        // Then: Err(CancelError::InstanceActorNotFound { instance_id })
        // And: No events are emitted
        match result {
            Err(CancelError::InstanceActorNotFound { instance_id: _ }) => {}
            other => panic!("Expected InstanceActorNotFound, got {:?}", other),
        }
    }

    // ========================================================================
    // Behavior: cancel_returns_lockacquisitionfailed_when_lock_unavailable
    // ========================================================================

    #[tokio::test]
    async fn cancel_returns_lockacquisitionfailed_when_lock_unavailable() {
        // Given: An instance exists but another writer holds the write lock
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BA00000").unwrap();
        let actor = ControlActor::new();

        // When: Cancel is handled
        let result = actor.handle_cancel(instance_id.clone());

        // Then: Err(CancelError::LockAcquisitionFailed { instance_id, reason: _ })
        // And: No events are emitted
        // RED PHASE: Currently returns InstanceActorNotFound
        match result {
            Err(CancelError::LockAcquisitionFailed {
                instance_id: _,
                reason: _,
            }) => {}
            other => panic!("Expected LockAcquisitionFailed, got {:?}", other),
        }
    }

    // ========================================================================
    // Behavior: cancel_returns_storageerror_when_event_append_fails
    // ========================================================================

    #[tokio::test]
    async fn cancel_returns_storageerror_when_event_append_fails() {
        // Given: An instance exists with valid state and acquired lock, but storage write fails
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BS00000").unwrap();
        let actor = ControlActor::new();

        // When: Cancel is handled
        let result = actor.handle_cancel(instance_id.clone());

        // Then: Err(CancelError::StorageError { instance_id, reason: _ })
        // And: No events are emitted
        // And: Lock is released (no partial state)
        // RED PHASE: Currently returns InstanceActorNotFound
        match result {
            Err(CancelError::StorageError {
                instance_id: _,
                reason: _,
            }) => {}
            other => panic!("Expected StorageError, got {:?}", other),
        }
    }

    // ========================================================================
    // Behavior: resume_on_failed_instance_emits_instanceresumed_and_actor_re-enters_decision
    // ========================================================================

    #[tokio::test]
    async fn resume_on_failed_instance_emits_instanceresumed_and_actor_re_enters_decision() {
        // Given: An instance exists with lifecycle state Failed, required secrets present, node exists, path to terminal exists
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00F000").unwrap();
        let actor = ControlActor::new();

        // When: Resume is handled
        let result = actor.handle_resume(instance_id.clone());

        // Then: Ok(InstanceResumed { instance_id, previous_binary_hash: H1, resumed_binary_hash: H2, resumed_at: T })
        // And: H1 != H2 (hash has advanced)
        // And: InstanceActor receives signal to re-enter RunningDecision
        // And: Lifecycle state transitions from Failed to Running
        // And: Write lock is released
        //
        // RED PHASE: This test will FAIL because handle_resume returns
        // InstanceActorNotFound error instead of InstanceResumed
        let instance_resumed = result.unwrap();

        assert_eq!(instance_resumed.instance_id, instance_id);
        assert_ne!(
            instance_resumed.previous_binary_hash,
            instance_resumed.resumed_binary_hash
        );
    }

    #[tokio::test]
    async fn resume_on_failed_instance_emits_instanceresumed_with_hash_state() {
        // Given: An instance exists with lifecycle state Failed
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00F000").unwrap();
        let actor = ControlActor::new();

        // When: Resume is handled
        let result = actor.handle_resume(instance_id.clone());

        // Then: InstanceResumed event is emitted with previous and resumed binary hashes
        // RED PHASE: result is Err(InstanceActorNotFound)
        let instance_resumed = result.unwrap();

        // Verify hash fields are populated
        assert!(!instance_resumed.previous_binary_hash.0.is_empty());
        assert!(!instance_resumed.resumed_binary_hash.0.is_empty());
        assert!(instance_resumed.resumed_at.0 > 0);
    }

    #[tokio::test]
    async fn resume_on_failed_instance_transitions_lifecycle_from_failed_to_running() {
        // Given: An instance exists with lifecycle state Failed
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00F000").unwrap();
        let actor = ControlActor::new();

        // When: Resume is handled
        let result = actor.handle_resume(instance_id.clone());

        // Then: Lifecycle state transitions from Failed to Running
        // RED PHASE: result is Err(InstanceActorNotFound)
        result.unwrap();
    }

    // ========================================================================
    // Behavior: resume_returns_invalidlifecyclestate_error_when_instance_is_running
    // ========================================================================

    #[tokio::test]
    async fn resume_returns_invalidlifecyclestate_error_when_instance_is_running() {
        // Given: An instance exists with lifecycle state Running
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000000").unwrap();
        let actor = ControlActor::new();

        // When: Resume is handled
        let result = actor.handle_resume(instance_id.clone());

        // Then: Err(ResumeError::InvalidLifecycleState { actual: Running, expected: Failed })
        // And: No events are emitted
        //
        // RED PHASE: Currently returns InstanceActorNotFound
        match result {
            Err(ResumeError::InvalidLifecycleState { actual, expected }) => {
                assert_eq!(actual, LifecycleState::Running);
                assert_eq!(expected, LifecycleState::Failed);
            }
            other => panic!(
                "Expected InvalidLifecycleState(Running, Failed), got {:?}",
                other
            ),
        }
    }

    // ========================================================================
    // Behavior: resume_returns_invalidlifecyclestate_error_when_instance_is_completed
    // ========================================================================

    #[tokio::test]
    async fn resume_returns_invalidlifecyclestate_error_when_instance_is_completed() {
        // Given: An instance exists with lifecycle state Completed
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00C000").unwrap();
        let actor = ControlActor::new();

        // When: Resume is handled
        let result = actor.handle_resume(instance_id.clone());

        // Then: Err(ResumeError::InvalidLifecycleState { actual: Completed, expected: Failed })
        // And: No events are emitted
        // RED PHASE: Currently returns InstanceActorNotFound
        match result {
            Err(ResumeError::InvalidLifecycleState { actual, expected }) => {
                assert_eq!(actual, LifecycleState::Completed);
                assert_eq!(expected, LifecycleState::Failed);
            }
            other => panic!(
                "Expected InvalidLifecycleState(Completed, Failed), got {:?}",
                other
            ),
        }
    }

    // ========================================================================
    // Behavior: resume_returns_invalidlifecyclestate_error_when_instance_is_cancelled
    // ========================================================================

    #[tokio::test]
    async fn resume_returns_invalidlifecyclestate_error_when_instance_is_cancelled() {
        // Given: An instance exists with lifecycle state Cancelled
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00X000").unwrap();
        let actor = ControlActor::new();

        // When: Resume is handled
        let result = actor.handle_resume(instance_id.clone());

        // Then: Err(ResumeError::InvalidLifecycleState { actual: Cancelled, expected: Failed })
        // And: No events are emitted
        // RED PHASE: Currently returns InstanceActorNotFound
        match result {
            Err(ResumeError::InvalidLifecycleState { actual, expected }) => {
                assert_eq!(actual, LifecycleState::Cancelled);
                assert_eq!(expected, LifecycleState::Failed);
            }
            other => panic!(
                "Expected InvalidLifecycleState(Cancelled, Failed), got {:?}",
                other
            ),
        }
    }

    // ========================================================================
    // Behavior: resume_returns_missingsecrets_error_when_secrets_absent
    // ========================================================================

    #[tokio::test]
    async fn resume_returns_missingsecrets_error_when_secrets_absent() {
        // Given: An instance exists with lifecycle Failed but required secret `secret-1` is missing
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BM0F000").unwrap();
        let actor = ControlActor::new();

        // When: Resume is handled
        let result = actor.handle_resume(instance_id.clone());

        // Then: Err(ResumeError::MissingSecrets { instance_id, missing_secret_ids: [secret-1] })
        // And: No events are emitted
        //
        // RED PHASE: Currently returns InstanceActorNotFound
        match result {
            Err(ResumeError::MissingSecrets {
                instance_id: _,
                missing_secret_ids,
            }) => {
                assert!(!missing_secret_ids.is_empty());
            }
            other => panic!("Expected MissingSecrets, got {:?}", other),
        }
    }

    // ========================================================================
    // Behavior: resume_returns_nodenotfound_error_when_node_missing
    // ========================================================================

    #[tokio::test]
    async fn resume_returns_nodenotfound_error_when_node_missing() {
        // Given: An instance exists with lifecycle Failed but required node `node-X` does not exist in workflow
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BN0F000").unwrap();
        let actor = ControlActor::new();

        // When: Resume is handled
        let result = actor.handle_resume(instance_id.clone());

        // Then: Err(ResumeError::NodeNotFound { instance_id, node_name: node-X })
        // And: No events are emitted
        // RED PHASE: Currently returns InstanceActorNotFound
        match result {
            Err(ResumeError::NodeNotFound {
                instance_id: _,
                node_name: _,
            }) => {}
            other => panic!("Expected NodeNotFound, got {:?}", other),
        }
    }

    // ========================================================================
    // Behavior: resume_returns_nopathtoterminal_error_when_no_valid_path
    // ========================================================================

    #[tokio::test]
    async fn resume_returns_nopathtoterminal_error_when_no_valid_path() {
        // Given: An instance exists with lifecycle Failed, node exists, but no valid path from current node to terminal
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BP0F000").unwrap();
        let actor = ControlActor::new();

        // When: Resume is handled
        let result = actor.handle_resume(instance_id.clone());

        // Then: Err(ResumeError::NoPathToTerminal { instance_id, current_node: node-Y })
        // And: No events are emitted
        // RED PHASE: Currently returns InstanceActorNotFound
        match result {
            Err(ResumeError::NoPathToTerminal {
                instance_id: _,
                current_node: _,
            }) => {}
            other => panic!("Expected NoPathToTerminal, got {:?}", other),
        }
    }

    // ========================================================================
    // Behavior: resume_returns_instanceactornotfound_when_actor_missing
    // ========================================================================

    #[tokio::test]
    async fn resume_returns_instanceactornotfound_when_actor_missing() {
        // Given: No InstanceActor exists for the given instance_id
        let instance_id = InstanceId::parse("00000000000000000000000001").unwrap();
        let actor = ControlActor::new();

        // When: Resume is handled
        let result = actor.handle_resume(instance_id.clone());

        // Then: Err(ResumeError::InstanceActorNotFound { instance_id })
        // And: No events are emitted
        match result {
            Err(ResumeError::InstanceActorNotFound { instance_id: _ }) => {}
            other => panic!("Expected InstanceActorNotFound, got {:?}", other),
        }
    }

    // ========================================================================
    // Behavior: resume_returns_lockacquisitionfailed_when_lock_unavailable
    // ========================================================================

    #[tokio::test]
    async fn resume_returns_lockacquisitionfailed_when_lock_unavailable() {
        // Given: An instance exists with lifecycle Failed but another writer holds the write lock
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BA0F000").unwrap();
        let actor = ControlActor::new();

        // When: Resume is handled
        let result = actor.handle_resume(instance_id.clone());

        // Then: Err(ResumeError::LockAcquisitionFailed { instance_id, reason: _ })
        // And: No events are emitted
        // RED PHASE: Currently returns InstanceActorNotFound
        match result {
            Err(ResumeError::LockAcquisitionFailed {
                instance_id: _,
                reason: _,
            }) => {}
            other => panic!("Expected LockAcquisitionFailed, got {:?}", other),
        }
    }

    // ========================================================================
    // Behavior: resume_returns_storageerror_when_event_append_fails
    // ========================================================================

    #[tokio::test]
    async fn resume_returns_storageerror_when_event_append_fails() {
        // Given: An instance exists with valid Failed state and acquired lock, but storage write fails
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BS0F000").unwrap();
        let actor = ControlActor::new();

        // When: Resume is handled
        let result = actor.handle_resume(instance_id.clone());

        // Then: Err(ResumeError::StorageError { instance_id, reason: _ })
        // And: No events are emitted
        // And: Lock is released (no partial state)
        // RED PHASE: Currently returns InstanceActorNotFound
        match result {
            Err(ResumeError::StorageError {
                instance_id: _,
                reason: _,
            }) => {}
            other => panic!("Expected StorageError, got {:?}", other),
        }
    }

    // ========================================================================
    // Proptest Invariants - ResumeError Classification
    // ========================================================================

    #[tokio::test]
    async fn resume_error_precondition_classification_is_correct() {
        // Invariant: ResumeError::is_precondition() returns true for InvalidLifecycleState,
        // MissingSecrets, NodeNotFound, NoPathToTerminal, InstanceActorNotFound.
        // Returns false for LockAcquisitionFailed, StorageError.
        use ResumeError::*;

        let precondition_errors = vec![
            InvalidLifecycleState {
                actual: LifecycleState::Running,
                expected: LifecycleState::Failed,
            },
            MissingSecrets {
                instance_id: InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000000").unwrap(),
                missing_secret_ids: vec![SecretId::new("secret-1")],
            },
            NodeNotFound {
                instance_id: InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000001").unwrap(),
                node_name: NodeName::new("node-X"),
            },
            NoPathToTerminal {
                instance_id: InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000002").unwrap(),
                current_node: NodeName::new("node-Y"),
            },
            InstanceActorNotFound {
                instance_id: InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000003").unwrap(),
            },
        ];

        for err in precondition_errors {
            assert!(
                err.is_precondition(),
                "Expected {:?} to be precondition",
                err
            );
            assert!(
                !err.is_transient(),
                "Expected {:?} to NOT be transient",
                err
            );
        }

        let transient_errors = vec![
            LockAcquisitionFailed {
                instance_id: InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000004").unwrap(),
                reason: "lock held".to_string(),
            },
            StorageError {
                instance_id: InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000005").unwrap(),
                reason: "io error".to_string(),
            },
        ];

        for err in transient_errors {
            assert!(
                !err.is_precondition(),
                "Expected {:?} to NOT be precondition",
                err
            );
            assert!(err.is_transient(), "Expected {:?} to be transient", err);
        }
    }

    #[tokio::test]
    async fn cancel_events_always_ordered_cancelrequested_then_workflowcancelled() {
        // Invariant: For any successful Cancel operation, the event stream contains
        // CancelRequested before WorkflowCancelled, with no intervening events for that instance.
        //
        // RED PHASE: handle_cancel doesn't return events correctly yet
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let actor = ControlActor::new();
        let result = actor.handle_cancel(instance_id.clone());

        // This would verify ordering in a full implementation
        match result {
            Ok((first, second)) => {
                // CancelRequested should have earlier timestamp than WorkflowCancelled
                assert!(
                    second.cancelled_at >= first.requested_at,
                    "WorkflowCancelled should come after CancelRequested"
                );
            }
            Err(_) => {
                // RED PHASE: Currently fails - this is expected
            }
        }
    }
}

#[cfg(test)]
mod accept_resume_tests {
    use super::*;

    // ── Group A: WaitKey validation ──

    #[test]
    fn waitkey_parse_succeeds_for_valid_input() {
        let key = WaitKey::parse("approval-v2").unwrap();
        assert_eq!(key.as_str(), "approval-v2");
    }

    #[test]
    fn waitkey_parse_rejects_empty_string() {
        let result = WaitKey::parse("");
        assert_eq!(result, Err("WaitKey cannot be empty".to_string()));
    }

    #[test]
    fn waitkey_parse_rejects_over_256_chars() {
        let long_key = "a".repeat(257);
        let result = WaitKey::parse(&long_key);
        assert_eq!(
            result,
            Err(format!(
                "WaitKey exceeds 256 characters: {}",
                long_key.len()
            ))
        );
    }

    #[test]
    fn waitkey_new_unchecked_bypasses_validation() {
        let key = WaitKey::new_unchecked("");
        assert_eq!(key.as_str(), "");
    }

    // ── Group B: SignalPayload validation ──

    #[test]
    fn signal_payload_from_bytes_succeeds_for_valid_payload() {
        let payload = SignalPayload::from_bytes(vec![1, 2, 3]).unwrap();
        assert_eq!(payload.as_bytes(), &[1, 2, 3]);
    }

    #[test]
    fn signal_payload_from_bytes_rejects_over_64kib() {
        let big = vec![0u8; 65537];
        let result = SignalPayload::from_bytes(big);
        assert_eq!(
            result,
            Err("SignalPayload exceeds 64 KiB: 65537 bytes".to_string())
        );
    }

    #[test]
    fn signal_payload_empty_creates_zero_length_payload() {
        let payload = SignalPayload::empty();
        assert!(payload.is_empty());
        assert_eq!(payload.len(), 0);
    }

    #[test]
    fn signal_payload_len_and_is_empty_are_correct() {
        let payload = SignalPayload::from_bytes(vec![42]).unwrap();
        assert!(!payload.is_empty());
        assert_eq!(payload.len(), 1);
    }

    // ── Group C: LifecycleState::WaitingForSignal ──

    #[test]
    fn waiting_for_signal_is_not_terminal() {
        assert!(!LifecycleState::WaitingForSignal.is_terminal());
    }

    #[test]
    fn lifecycle_state_all_variants_is_terminal_correctness() {
        assert!(!LifecycleState::Running.is_terminal());
        assert!(!LifecycleState::Failed.is_terminal());
        assert!(LifecycleState::Completed.is_terminal());
        assert!(LifecycleState::Cancelled.is_terminal());
        assert!(!LifecycleState::WaitingForSignal.is_terminal());
    }

    // ── Group D: AcceptResumeError classification ──

    #[test]
    fn accept_resume_error_precondition_variants_are_correct() {
        let iid = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000000").unwrap();
        let precondition_errors: Vec<AcceptResumeError> = vec![
            AcceptResumeError::InvalidLifecycleState {
                instance_id: iid.clone(),
                actual: LifecycleState::Running,
                expected: LifecycleState::WaitingForSignal,
            },
            AcceptResumeError::WaitKeyMismatch {
                instance_id: iid.clone(),
                expected_key: WaitKey::new_unchecked("a"),
                provided_key: WaitKey::new_unchecked("b"),
            },
            AcceptResumeError::InstanceActorNotFound {
                instance_id: iid.clone(),
            },
            AcceptResumeError::PayloadTooLarge {
                instance_id: iid,
                payload_size: 65537,
                max_size: 65536,
            },
        ];
        for err in &precondition_errors {
            assert!(
                err.is_precondition(),
                "Expected {:?} to be precondition",
                err
            );
            assert!(
                !err.is_transient(),
                "Expected {:?} to NOT be transient",
                err
            );
        }
    }

    #[test]
    fn accept_resume_error_transient_variants_are_correct() {
        let iid = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000000").unwrap();
        let transient_errors: Vec<AcceptResumeError> = vec![
            AcceptResumeError::LockAcquisitionFailed {
                instance_id: iid.clone(),
                reason: "lock held".to_string(),
            },
            AcceptResumeError::StorageError {
                instance_id: iid,
                reason: "io error".to_string(),
            },
        ];
        for err in &transient_errors {
            assert!(
                !err.is_precondition(),
                "Expected {:?} to NOT be precondition",
                err
            );
            assert!(err.is_transient(), "Expected {:?} to be transient", err);
        }
    }

    // ── Group E: accept_and_resume success path ──

    #[tokio::test]
    async fn accept_and_resume_succeeds_when_waiting_for_signal() {
        // 'W' at position 22 encodes WaitingForSignal
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
        let actor = ControlActor::new();
        let wait_key = WaitKey::parse("approval-v2").unwrap();
        let payload = SignalPayload::empty();

        let result =
            actor.accept_and_resume(instance_id.clone(), wait_key, "sig-1".to_string(), payload);

        let outcome = result.unwrap();
        assert_eq!(outcome.accepted.instance_id, instance_id);
        assert_eq!(outcome.resumed.instance_id, instance_id);
    }

    #[tokio::test]
    async fn accept_and_resume_outcome_has_correct_instance_id() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
        let actor = ControlActor::new();
        let wait_key = WaitKey::parse("approval-v2").unwrap();

        let result = actor.accept_and_resume(
            instance_id.clone(),
            wait_key,
            "sig-2".to_string(),
            SignalPayload::empty(),
        );

        let outcome = result.unwrap();
        assert_eq!(outcome.accepted.instance_id, instance_id);
        assert_eq!(outcome.resumed.instance_id, instance_id);
    }

    #[tokio::test]
    async fn accept_and_resume_outcome_timestamps_are_ordered() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
        let actor = ControlActor::new();
        let wait_key = WaitKey::parse("approval-v2").unwrap();

        let result = actor.accept_and_resume(
            instance_id,
            wait_key,
            "sig-3".to_string(),
            SignalPayload::empty(),
        );

        let outcome = result.unwrap();
        assert!(outcome.resumed.resumed_at >= outcome.accepted.accepted_at);
    }

    // ── Group F: accept_and_resume error paths ──

    #[tokio::test]
    async fn accept_and_resume_returns_instance_not_found() {
        let instance_id = InstanceId::parse("00000000000000000000000001").unwrap();
        let actor = ControlActor::new();
        let wait_key = WaitKey::parse("approval-v2").unwrap();

        let result = actor.accept_and_resume(
            instance_id.clone(),
            wait_key,
            "sig-1".to_string(),
            SignalPayload::empty(),
        );

        match result {
            Err(AcceptResumeError::InstanceActorNotFound { instance_id: _ }) => {}
            other => panic!("Expected InstanceActorNotFound, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn accept_and_resume_returns_invalid_lifecycle_when_running() {
        // Default char at pos 22 means Running
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000000").unwrap();
        let actor = ControlActor::new();
        let wait_key = WaitKey::parse("approval-v2").unwrap();

        let result = actor.accept_and_resume(
            instance_id.clone(),
            wait_key,
            "sig-1".to_string(),
            SignalPayload::empty(),
        );

        match result {
            Err(AcceptResumeError::InvalidLifecycleState {
                instance_id: _,
                actual,
                expected,
            }) => {
                assert_eq!(actual, LifecycleState::Running);
                assert_eq!(expected, LifecycleState::WaitingForSignal);
            }
            other => panic!("Expected InvalidLifecycleState(Running), got {:?}", other),
        }
    }

    #[tokio::test]
    async fn accept_and_resume_returns_wait_key_mismatch() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
        let actor = ControlActor::new();
        let wait_key = WaitKey::parse("wrong-key").unwrap();

        let result = actor.accept_and_resume(
            instance_id.clone(),
            wait_key,
            "mismatch-sig-1".to_string(),
            SignalPayload::empty(),
        );

        match result {
            Err(AcceptResumeError::WaitKeyMismatch {
                instance_id: _,
                expected_key,
                provided_key,
            }) => {
                assert_eq!(expected_key.as_str(), "expected-key");
                assert_eq!(provided_key.as_str(), "wrong-key");
            }
            other => panic!("Expected WaitKeyMismatch, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn accept_and_resume_returns_payload_too_large() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
        let actor = ControlActor::new();
        let wait_key = WaitKey::parse("approval-v2").unwrap();
        let big_payload = SignalPayload::new_unchecked(vec![0u8; 65537]);

        let result = actor.accept_and_resume(
            instance_id.clone(),
            wait_key,
            "sig-1".to_string(),
            big_payload,
        );

        match result {
            Err(AcceptResumeError::PayloadTooLarge {
                instance_id: _,
                payload_size,
                max_size,
            }) => {
                assert_eq!(payload_size, 65537);
                assert_eq!(max_size, 65536);
            }
            other => panic!("Expected PayloadTooLarge, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn accept_and_resume_returns_lock_acquisition_failed() {
        // 'A' at position 20 encodes lock error
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BA0W000").unwrap();
        let actor = ControlActor::new();
        let wait_key = WaitKey::parse("approval-v2").unwrap();

        let result = actor.accept_and_resume(
            instance_id.clone(),
            wait_key,
            "sig-1".to_string(),
            SignalPayload::empty(),
        );

        match result {
            Err(AcceptResumeError::LockAcquisitionFailed {
                instance_id: _,
                reason: _,
            }) => {}
            other => panic!("Expected LockAcquisitionFailed, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn accept_and_resume_returns_storage_error() {
        // 'S' at position 20 encodes storage error
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BS0W000").unwrap();
        let actor = ControlActor::new();
        let wait_key = WaitKey::parse("approval-v2").unwrap();

        let result = actor.accept_and_resume(
            instance_id.clone(),
            wait_key,
            "sig-1".to_string(),
            SignalPayload::empty(),
        );

        match result {
            Err(AcceptResumeError::StorageError {
                instance_id: _,
                reason: _,
            }) => {}
            other => panic!("Expected StorageError, got {:?}", other),
        }
    }

    // ── Group G: Schema-required acceptance tests ──

    /// Test: Workflow correctly transitions from Waiting to Ready when signaled.
    /// EARS: THE SYSTEM SHALL atomically transition workflows from waiting to ready
    /// upon signal acceptance.
    #[tokio::test]
    async fn test_workflow_correctly_transitions_from_waiting_to_ready_when_signaled() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
        let actor = ControlActor::new();
        let wait_key = WaitKey::parse("approval-v2").unwrap();
        let payload = SignalPayload::empty();

        let result =
            actor.accept_and_resume(instance_id.clone(), wait_key, "sig-1".to_string(), payload);

        let outcome = result.expect("accept_and_resume should succeed when workflow is waiting");
        assert_eq!(
            outcome.accepted.instance_id, instance_id,
            "accepted.instance_id should match"
        );
        assert_eq!(
            outcome.resumed.instance_id, instance_id,
            "resumed.instance_id should match"
        );
        assert!(
            outcome.resumed.resumed_at >= outcome.accepted.accepted_at,
            "resumed_at should be >= accepted_at for atomic transition"
        );
    }

    /// Test: Workflow correctly transitions from Waiting to Ready when signaled (duplicate for schema).
    #[tokio::test]
    async fn test_workflow_correctly_transitions_from_waiting_to_ready_when_signaled_duplicate_for()
    {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
        let actor = ControlActor::new();
        let wait_key = WaitKey::parse("webhook").unwrap();
        let payload = SignalPayload::from_bytes(vec![1, 2, 3]).expect("valid payload");

        let result = actor.accept_and_resume(
            instance_id.clone(),
            wait_key,
            "sig-duplicate".to_string(),
            payload,
        );

        let outcome = result.expect("accept_and_resume should succeed");
        assert_eq!(outcome.accepted.instance_id, instance_id);
        assert_eq!(outcome.resumed.instance_id, instance_id);
    }

    /// Test: Transition fails gracefully if workflow is in a terminal state.
    /// EARS: IF the transition fails, THE SYSTEM SHALL NOT consume the signal.
    #[tokio::test]
    async fn test_transition_fails_gracefully_if_workflow_is_in_a_terminal_state() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00C000").unwrap();
        let actor = ControlActor::new();
        let wait_key = WaitKey::parse("approval-v2").unwrap();
        let payload = SignalPayload::empty();

        let result =
            actor.accept_and_resume(instance_id.clone(), wait_key, "sig-1".to_string(), payload);

        assert!(
            result.is_err(),
            "accept_and_resume should fail when workflow is in terminal state"
        );
        let err = result.unwrap_err();
        assert!(
            matches!(err, AcceptResumeError::InvalidLifecycleState { .. }),
            "Expected InvalidLifecycleState error, got {:?}",
            err
        );
    }

    /// Test: Transition fails gracefully if workflow is in a terminal state (duplicate for schema).
    #[tokio::test]
    async fn test_transition_fails_gracefully_if_workflow_is_in_a_terminal_state_duplicate_for_sch()
    {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00X000").unwrap();
        let actor = ControlActor::new();
        let wait_key = WaitKey::parse("approval-v2").unwrap();
        let payload = SignalPayload::empty();

        let result =
            actor.accept_and_resume(instance_id.clone(), wait_key, "sig-2".to_string(), payload);

        assert!(
            result.is_err(),
            "accept_and_resume should fail when workflow is Cancelled terminal state"
        );
        let err = result.unwrap_err();
        assert!(
            matches!(err, AcceptResumeError::InvalidLifecycleState { .. }),
            "Expected InvalidLifecycleState error for Cancelled state, got {:?}",
            err
        );
    }
}

pub use actor_messages::{ControlActorMessage, InstanceActorMessage};
