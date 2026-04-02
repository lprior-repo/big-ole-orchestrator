// Stub file - keeping original stubs for compatibility
pub mod heartbeat {
    pub fn run_heartbeat_watcher() {}
}

pub mod master {
    pub struct MasterOrchestrator;
    pub struct OrchestratorConfig;
}

#[derive(Debug)]
pub enum TerminateError {
    NotFound(String),
    Failed(String),
}

#[derive(Debug)]
pub enum WorkflowParadigm {
    Default,
}

#[derive(Debug, PartialEq, Eq)]
pub enum InstancePhaseView {
    Replay,
    Live,
}

#[derive(Debug)]
pub struct OrchestratorMsg;

#[derive(Debug)]
pub struct StartError;

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
pub mod actor_messages {
    // Import types directly from vo-types
    use vo_types::{InstanceId, NodeName, SequenceNumber, TimerId, WorkflowName};

    // =============================================================================
    // Type Definitions
    // =============================================================================

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
}

pub use actor_messages::{ControlActorMessage, InstanceActorMessage};
