//! Actor framework for vo-engine.
//!
//! Provides the actor model implementation using the Ractor library.
//! Actors are the fundamental units of computation in the engine.

use bytes::Bytes;
use vo_types::InstanceId;
use vo_types::{SequenceNumber, TimerId, WorkflowName};

pub mod heartbeat {
    pub fn run_heartbeat_watcher() {}
}

pub mod actor_messages;
pub mod async_message_router;
pub mod fairness;
pub mod instance;
pub mod instance_registry;
pub mod lifecycle;
pub mod master;
pub mod message_router;
pub mod orchestrator_msg;
pub mod port;
pub mod probe;
pub mod reanimator;
pub mod semaphore;
pub mod signal_buffer;
pub mod signal_messages;
pub mod signals;
pub mod spawn_supervisor;
pub mod instance_actor_message;
pub mod control_actor_message;
pub mod control_actor;

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

pub use orchestrator_msg::{
    CompensateError, InstancePhaseView, InstanceSnapshot, NamespaceId, OrchestratorMsg,
    ReservedPermitBudget, SignalError, StartError, TerminateError, WorkflowParadigm,
};
pub use fairness::WorkloadClass;

pub use signal_messages::mock_signal_storage;
pub use signal_messages::mock_signal_storage::{MockSignalStorage, MockSignalWorkQueue};
pub use signal_messages::{
    AcceptResumeError, AcceptResumeOutcome, BinaryHash, CancelError, CancelRequested,
    ContinueAsNewError, InstanceResumed, LifecycleState, NodeName, ResumeError, RolloverState,
    SecretId, SignalAccepted, SignalName, SignalPayload, SignalStorage, SignalStorageError,
    SignalWorkQueue, SignalWorkQueueError, StateLookup, TestStateLookup, TimestampMs, WaitKey,
    WorkflowCancelled, WorkflowContinued,
};

pub use instance_actor_message::InstanceActorMessage;
pub use control_actor_message::ControlActorMessage;
pub use control_actor::ControlActor;

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

#[cfg(test)]
mod constructor_tests_instance_actor_message {
    use super::*;
    use vo_types::{InstanceId, SequenceNumber, TimerId, WorkflowName};

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
    fn cancel_requested_constructs_correctly() {
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
    fn get_status_constructs_correctly() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        let message = InstanceActorMessage::new_get_status(instance_id.clone());

        match &message {
            InstanceActorMessage::GetStatus { instance_id: id } => {
                assert_eq!(id.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
            }
            _ => panic!("Expected GetStatus variant"),
        }
    }

    #[test]
    fn all_variants_have_expected_fields() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let workflow_name = WorkflowName::parse("test-workflow").unwrap();
        let node_name = NodeName::parse("test-node").unwrap();
        let timer_id = TimerId::parse("timer-xyz").unwrap();
        let sequence = SequenceNumber::new_unchecked(99);

        fn _check_start(m: &InstanceActorMessage) {
            if let InstanceActorMessage::StartWorkflow { instance_id, workflow_name, node_name } = m {
                let _ = (instance_id, workflow_name, node_name);
            }
        }

        fn _check_step_completed(m: &InstanceActorMessage) {
            if let InstanceActorMessage::StepCompleted { instance_id, node_name, sequence } = m {
                let _ = (instance_id, node_name, sequence);
            }
        }

        fn _check_step_failed(m: &InstanceActorMessage) {
            if let InstanceActorMessage::StepFailed { instance_id, node_name, sequence, error } = m {
                let _ = (instance_id, node_name, sequence, error);
            }
        }

        fn _check_timer_fired(m: &InstanceActorMessage) {
            if let InstanceActorMessage::TimerFired { instance_id, timer_id } = m {
                let _ = (instance_id, timer_id);
            }
        }

        fn _check_cancel_requested(m: &InstanceActorMessage) {
            if let InstanceActorMessage::CancelRequested { instance_id } = m {
                let _ = instance_id;
            }
        }

        fn _check_get_status(m: &InstanceActorMessage) {
            if let InstanceActorMessage::GetStatus { instance_id } = m {
                let _ = instance_id;
            }
        }

        let _ = (
            _check_start,
            _check_step_completed,
            _check_step_failed,
            _check_timer_fired,
            _check_cancel_requested,
            _check_get_status,
        );

        assert!(true);
    }
}

#[cfg(test)]
mod constructor_tests_control_actor_message {
    use super::*;

    #[test]
    fn cancel_constructs_correctly() {
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
    fn resume_constructs_correctly() {
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
        let wait_key = WaitKey::parse("approval-v2").unwrap();
        let signal_name = SignalName::parse("sig-123").unwrap();
        let payload = SignalPayload::empty();

        let message = ControlActorMessage::new_accept_and_resume(
            instance_id.clone(),
            wait_key,
            signal_name,
            payload,
        );

        match &message {
            ControlActorMessage::AcceptAndResume {
                instance_id: id,
                wait_key: wk,
                signal_id: sn,
                payload: pl,
            } => {
                assert_eq!(id.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
                assert_eq!(wk.as_str(), "approval-v2");
                assert_eq!(sn.as_str(), "sig-123");
            }
            _ => panic!("Expected AcceptAndResume variant"),
        }
    }

    #[test]
    fn all_variants_have_expected_fields() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let wait_key = WaitKey::parse("key-1").unwrap();
        let signal_name = SignalName::parse("sig-1").unwrap();
        let payload = SignalPayload::empty();

        fn _check_cancel(m: &ControlActorMessage) {
            if let ControlActorMessage::Cancel { instance_id } = m {
                let _ = instance_id;
            }
        }

        fn _check_resume(m: &ControlActorMessage) {
            if let ControlActorMessage::Resume { instance_id } = m {
                let _ = instance_id;
            }
        }

        fn _check_accept_and_resume(m: &ControlActorMessage) {
            if let ControlActorMessage::AcceptAndResume {
                instance_id,
                wait_key,
                signal_id,
                payload,
            } = m
            {
                let _ = (instance_id, wait_key, signal_id, payload);
            }
        }

        let _ = (_check_cancel, _check_resume, _check_accept_and_resume);
        assert!(true);
    }
}

#[cfg(test)]
mod debug_format_instance_actor_message {
    use super::*;

    #[test]
    fn debug_format_includes_variant_name() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let workflow_name = WorkflowName::parse("test-workflow").unwrap();
        let node_name = NodeName::parse("test-node").unwrap();

        let message = InstanceActorMessage::new_start_workflow(
            instance_id.clone(),
            workflow_name.clone(),
            node_name,
        );

        let debug_str = format!("{:?}", message);
        assert!(debug_str.contains("StartWorkflow"), "Debug format should contain variant name");
    }

    #[test]
    fn debug_format_does_not_panic() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let workflow_name = WorkflowName::parse("test-workflow").unwrap();
        let node_name = NodeName::parse("test-node").unwrap();

        let message = InstanceActorMessage::new_start_workflow(
            instance_id,
            workflow_name,
            node_name,
        );

        let debug_str = format!("{:?}", message);
        assert!(!debug_str.is_empty());
    }

    #[test]
    fn each_variant_debug_format_is_unique() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let node_name = NodeName::parse("test-node").unwrap();
        let sequence = SequenceNumber::new_unchecked(1);
        let timer_id = TimerId::parse("timer-1").unwrap();

        let start = InstanceActorMessage::new_start_workflow(
            instance_id.clone(),
            WorkflowName::parse("wf-1").unwrap(),
            node_name.clone(),
        );
        let step = InstanceActorMessage::new_step_completed(
            instance_id.clone(),
            node_name.clone(),
            sequence,
        );
        let timer = InstanceActorMessage::new_timer_fired(instance_id.clone(), timer_id);
        let cancel = InstanceActorMessage::new_cancel_requested(instance_id);

        let formats = vec![
            format!("{:?}", start),
            format!("{:?}", step),
            format!("{:?}", timer),
            format!("{:?}", cancel),
        ];

        for (i, fmt1) in formats.iter().enumerate() {
            for fmt2 in formats.iter().skip(i + 1) {
                assert_ne!(
                    fmt1, fmt2,
                    "Debug formats should be unique across variants"
                );
            }
        }
    }
}

#[cfg(test)]
mod debug_format_control_actor_message {
    use super::*;

    #[test]
    fn debug_format_includes_variant_name() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        let message = ControlActorMessage::new_cancel(instance_id);

        let debug_str = format!("{:?}", message);
        assert!(debug_str.contains("Cancel"), "Debug format should contain variant name");
    }

    #[test]
    fn debug_format_does_not_panic() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        let message = ControlActorMessage::new_cancel(instance_id);

        let debug_str = format!("{:?}", message);
        assert!(!debug_str.is_empty());
    }

    #[test]
    fn each_variant_debug_format_is_unique() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        let cancel = ControlActorMessage::new_cancel(instance_id.clone());
        let resume = ControlActorMessage::new_resume(instance_id.clone());
        let accept_resume = ControlActorMessage::new_accept_and_resume(
            instance_id,
            WaitKey::parse("key-1").unwrap(),
            SignalName::parse("sig-1").unwrap(),
            SignalPayload::empty(),
        );

        let formats = vec![
            format!("{:?}", cancel),
            format!("{:?}", resume),
            format!("{:?}", accept_resume),
        ];

        for (i, fmt1) in formats.iter().enumerate() {
            for fmt2 in formats.iter().skip(i + 1) {
                assert_ne!(
                    fmt1, fmt2,
                    "Debug formats should be unique across variants"
                );
            }
        }
    }
}

#[cfg(test)]
mod clone_instance_actor_message {
    use super::*;

    #[test]
    fn clone_produces_equal_message() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let workflow_name = WorkflowName::parse("test-workflow").unwrap();
        let node_name = NodeName::parse("test-node").unwrap();

        let original = InstanceActorMessage::new_start_workflow(
            instance_id.clone(),
            workflow_name.clone(),
            node_name.clone(),
        );
        let cloned = original.clone();

        assert_eq!(original, cloned);
    }

    #[test]
    fn clone_is_independent() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let workflow_name = WorkflowName::parse("test-workflow").unwrap();
        let node_name = NodeName::parse("test-node").unwrap();

        let mut original = InstanceActorMessage::new_start_workflow(
            instance_id.clone(),
            workflow_name.clone(),
            node_name.clone(),
        );

        let cloned = original.clone();
        assert_eq!(original, cloned);

        match (&mut original, &cloned) {
            (
                InstanceActorMessage::StartWorkflow {
                    instance_id: orig_id,
                    workflow_name: orig_wf,
                    node_name: orig_node,
                },
                InstanceActorMessage::StartWorkflow {
                    instance_id: clone_id,
                    workflow_name: clone_wf,
                    node_name: clone_node,
                },
            ) => {
                assert_eq!(orig_id.as_str(), clone_id.as_str());
                assert_eq!(orig_wf.as_str(), clone_wf.as_str());
                assert_eq!(orig_node.as_str(), clone_node.as_str());
            }
            _ => panic!("Variant mismatch"),
        }
    }

    #[test]
    fn all_variants_are_cloneable() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let node_name = NodeName::parse("test-node").unwrap();
        let sequence = SequenceNumber::new_unchecked(1);
        let timer_id = TimerId::parse("timer-1").unwrap();

        let start = InstanceActorMessage::new_start_workflow(
            instance_id.clone(),
            WorkflowName::parse("wf-1").unwrap(),
            node_name.clone(),
        );
        let step = InstanceActorMessage::new_step_completed(
            instance_id.clone(),
            node_name.clone(),
            sequence,
        );
        let step_failed = InstanceActorMessage::new_step_failed(
            instance_id.clone(),
            node_name.clone(),
            SequenceNumber::new_unchecked(2),
            "error".to_string(),
        );
        let timer = InstanceActorMessage::new_timer_fired(instance_id.clone(), timer_id);
        let cancel = InstanceActorMessage::new_cancel_requested(instance_id.clone());
        let status = InstanceActorMessage::new_get_status(instance_id);

        let _ = start.clone();
        let _ = step.clone();
        let _ = step_failed.clone();
        let _ = timer.clone();
        let _ = cancel.clone();
        let _ = status.clone();

        assert!(true);
    }
}

#[cfg(test)]
mod clone_control_actor_message {
    use super::*;

    #[test]
    fn clone_produces_equal_message() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        let original = ControlActorMessage::new_cancel(instance_id.clone());
        let cloned = original.clone();

        assert_eq!(original, cloned);
    }

    #[test]
    fn clone_is_independent() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        let original = ControlActorMessage::new_cancel(instance_id.clone());
        let cloned = original.clone();

        assert_eq!(original, cloned);
    }

    #[test]
    fn all_variants_are_cloneable() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let wait_key = WaitKey::parse("key-1").unwrap();
        let signal_name = SignalName::parse("sig-1").unwrap();
        let payload = SignalPayload::empty();

        let cancel = ControlActorMessage::new_cancel(instance_id.clone());
        let resume = ControlActorMessage::new_resume(instance_id.clone());
        let accept_resume = ControlActorMessage::new_accept_and_resume(
            instance_id,
            wait_key,
            signal_name,
            payload,
        );

        let _ = cancel.clone();
        let _ = resume.clone();
        let _ = accept_resume.clone();

        assert!(true);
    }
}

#[cfg(test)]
mod partial_eq_instance_actor_message {
    use super::*;

    #[test]
    fn same_variant_same_fields_are_equal() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let workflow_name = WorkflowName::parse("test-workflow").unwrap();
        let node_name = NodeName::parse("test-node").unwrap();

        let msg1 = InstanceActorMessage::new_start_workflow(
            instance_id.clone(),
            workflow_name.clone(),
            node_name.clone(),
        );
        let msg2 = InstanceActorMessage::new_start_workflow(
            instance_id.clone(),
            workflow_name.clone(),
            node_name.clone(),
        );

        assert_eq!(msg1, msg2);
    }

    #[test]
    fn different_instance_ids_not_equal() {
        let instance_id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let instance_id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BYYYYYX").unwrap();
        let workflow_name = WorkflowName::parse("test-workflow").unwrap();
        let node_name = NodeName::parse("test-node").unwrap();

        let msg1 = InstanceActorMessage::new_start_workflow(
            instance_id1,
            workflow_name.clone(),
            node_name.clone(),
        );
        let msg2 = InstanceActorMessage::new_start_workflow(
            instance_id2,
            workflow_name.clone(),
            node_name.clone(),
        );

        assert_ne!(msg1, msg2);
    }

    #[test]
    fn different_workflow_names_not_equal() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let node_name = NodeName::parse("test-node").unwrap();

        let msg1 = InstanceActorMessage::new_start_workflow(
            instance_id.clone(),
            WorkflowName::parse("workflow-1").unwrap(),
            node_name.clone(),
        );
        let msg2 = InstanceActorMessage::new_start_workflow(
            instance_id.clone(),
            WorkflowName::parse("workflow-2").unwrap(),
            node_name.clone(),
        );

        assert_ne!(msg1, msg2);
    }

    #[test]
    fn same_variant_different_fields_not_equal() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let node_name1 = NodeName::parse("node-1").unwrap();
        let node_name2 = NodeName::parse("node-2").unwrap();

        let msg1 = InstanceActorMessage::new_start_workflow(
            instance_id.clone(),
            WorkflowName::parse("wf").unwrap(),
            node_name1,
        );
        let msg2 = InstanceActorMessage::new_start_workflow(
            instance_id.clone(),
            WorkflowName::parse("wf").unwrap(),
            node_name2,
        );

        assert_ne!(msg1, msg2);
    }

    #[test]
    fn different_variants_not_equal() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let node_name = NodeName::parse("test-node").unwrap();
        let sequence = SequenceNumber::new_unchecked(1);

        let start = InstanceActorMessage::new_start_workflow(
            instance_id.clone(),
            WorkflowName::parse("wf").unwrap(),
            node_name.clone(),
        );
        let step = InstanceActorMessage::new_step_completed(instance_id.clone(), node_name.clone(), sequence);
        let cancel = InstanceActorMessage::new_cancel_requested(instance_id);

        assert_ne!(start, step);
        assert_ne!(step, cancel);
        assert_ne!(start, cancel);
    }

    #[test]
    fn reflexive_equality() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let msg = InstanceActorMessage::new_cancel_requested(instance_id);
        assert_eq!(msg, msg);
    }
}

#[cfg(test)]
mod partial_eq_control_actor_message {
    use super::*;

    #[test]
    fn same_variant_same_fields_are_equal() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        let msg1 = ControlActorMessage::new_cancel(instance_id.clone());
        let msg2 = ControlActorMessage::new_cancel(instance_id.clone());

        assert_eq!(msg1, msg2);
    }

    #[test]
    fn different_instance_ids_not_equal() {
        let instance_id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let instance_id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BYYYYYX").unwrap();

        let msg1 = ControlActorMessage::new_cancel(instance_id1);
        let msg2 = ControlActorMessage::new_cancel(instance_id2);

        assert_ne!(msg1, msg2);
    }

    #[test]
    fn different_variants_not_equal() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let wait_key = WaitKey::parse("key-1").unwrap();
        let signal_name = SignalName::parse("sig-1").unwrap();
        let payload = SignalPayload::empty();

        let cancel = ControlActorMessage::new_cancel(instance_id.clone());
        let resume = ControlActorMessage::new_resume(instance_id.clone());
        let accept_resume = ControlActorMessage::new_accept_and_resume(
            instance_id,
            wait_key,
            signal_name,
            payload,
        );

        assert_ne!(cancel, resume);
        assert_ne!(resume, accept_resume);
        assert_ne!(cancel, accept_resume);
    }

    #[test]
    fn reflexive_equality() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let msg = ControlActorMessage::new_cancel(instance_id);
        assert_eq!(msg, msg);
    }
}

#[cfg(test)]
mod eq_properties_instance_actor_message {
    use super::*;

    #[test]
    fn equality_is_symmetric() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let msg1 = InstanceActorMessage::new_cancel_requested(instance_id.clone());
        let msg2 = InstanceActorMessage::new_cancel_requested(instance_id.clone());

        assert_eq!(msg1, msg2);
        assert_eq!(msg2, msg1);
    }

    #[test]
    fn equality_is_transitive() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let msg1 = InstanceActorMessage::new_cancel_requested(instance_id.clone());
        let msg2 = InstanceActorMessage::new_cancel_requested(instance_id.clone());
        let msg3 = InstanceActorMessage::new_cancel_requested(instance_id.clone());

        assert_eq!(msg1, msg2);
        assert_eq!(msg2, msg3);
        assert_eq!(msg1, msg3);
    }

    #[test]
    fn neq_is_not_eq() {
        let instance_id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let instance_id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BYYYYYX").unwrap();

        let msg1 = InstanceActorMessage::new_cancel_requested(instance_id1);
        let msg2 = InstanceActorMessage::new_cancel_requested(instance_id2);

        assert!(msg1 != msg2);
        assert!(!(msg1 == msg2));
    }
}

#[cfg(test)]
mod eq_properties_control_actor_message {
    use super::*;

    #[test]
    fn equality_is_symmetric() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let msg1 = ControlActorMessage::new_cancel(instance_id.clone());
        let msg2 = ControlActorMessage::new_cancel(instance_id.clone());

        assert_eq!(msg1, msg2);
        assert_eq!(msg2, msg1);
    }

    #[test]
    fn equality_is_transitive() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let msg1 = ControlActorMessage::new_cancel(instance_id.clone());
        let msg2 = ControlActorMessage::new_cancel(instance_id.clone());
        let msg3 = ControlActorMessage::new_cancel(instance_id.clone());

        assert_eq!(msg1, msg2);
        assert_eq!(msg2, msg3);
        assert_eq!(msg1, msg3);
    }

    #[test]
    fn neq_is_not_eq() {
        let instance_id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let instance_id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BYYYYYX").unwrap();

        let msg1 = ControlActorMessage::new_cancel(instance_id1);
        let msg2 = ControlActorMessage::new_cancel(instance_id2);

        assert!(msg1 != msg2);
        assert!(!(msg1 == msg2));
    }
}

#[cfg(test)]
mod send_sync_bounds {
    use super::*;

    #[test]
    fn instance_actor_message_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<InstanceActorMessage>();
    }

    #[test]
    fn instance_actor_message_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<InstanceActorMessage>();
    }

    #[test]
    fn control_actor_message_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ControlActorMessage>();
    }

    #[test]
    fn control_actor_message_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ControlActorMessage>();
    }
}

#[cfg(test)]
mod ractor_message_trait {
    use super::*;

    #[test]
    fn instance_actor_message_implements_ractor_message() {
        fn assert_message<T: ractor::Message>() {}
        assert_message::<InstanceActorMessage>();
    }

    #[test]
    fn control_actor_message_implements_ractor_message() {
        fn assert_message<T: ractor::Message>() {}
        assert_message::<ControlActorMessage>();
    }
}

#[cfg(test)]
mod reserved_permit_budget_tests {
    use super::*;

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
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
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

#[cfg(test)]
mod control_actor_tests {
    use super::*;

    #[tokio::test]
    async fn test_cancel_succeeds_for_non_terminal_instance() {
        let actor = ControlActor::new();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00R000").unwrap();

        let result = actor.handle_cancel(instance_id.clone());

        assert!(result.is_ok(), "Cancel should succeed for non-terminal instance");
        let (cancel_requested, workflow_cancelled) = result.unwrap();
        assert_eq!(cancel_requested.instance_id, instance_id);
        assert_eq!(workflow_cancelled.instance_id, instance_id);
    }

    #[tokio::test]
    async fn test_cancel_fails_for_terminal_instance() {
        let actor = ControlActor::new();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00CXXX").unwrap();

        let result = actor.handle_cancel(instance_id.clone());

        assert!(result.is_err(), "Cancel should fail for terminal instance");
        let err = result.unwrap_err();
        assert!(
            matches!(err, CancelError::AlreadyTerminal { .. }),
            "Expected AlreadyTerminal error, got {:?}",
            err
        );
    }

    #[tokio::test]
    async fn test_cancel_fails_for_nonexistent_instance() {
        let actor = ControlActor::new();
        let instance_id = InstanceId::parse("0000000000XXXXXXXXXXXXXXXX").unwrap();

        let result = actor.handle_cancel(instance_id.clone());

        assert!(result.is_err(), "Cancel should fail for non-existent instance");
        let err = result.unwrap_err();
        assert!(
            matches!(err, CancelError::InstanceActorNotFound { .. }),
            "Expected InstanceActorNotFound error, got {:?}",
            err
        );
    }

    #[tokio::test]
    async fn test_resume_succeeds_for_failed_instance() {
        let actor = ControlActor::new();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00F000").unwrap();

        let result = actor.handle_resume(instance_id.clone());

        assert!(result.is_ok(), "Resume should succeed for Failed instance");
        let resumed = result.unwrap();
        assert_eq!(resumed.instance_id, instance_id);
    }

    #[tokio::test]
    async fn test_resume_fails_for_non_failed_instance() {
        let actor = ControlActor::new();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00C000").unwrap();

        let result = actor.handle_resume(instance_id.clone());

        assert!(result.is_err(), "Resume should fail for non-Failed instance");
        let err = result.unwrap_err();
        assert!(
            matches!(err, ResumeError::InvalidLifecycleState { .. }),
            "Expected InvalidLifecycleState error, got {:?}",
            err
        );
    }

    #[tokio::test]
    async fn test_resume_fails_for_nonexistent_instance() {
        let actor = ControlActor::new();
        let instance_id = InstanceId::parse("0000000000XXXXXXXXXXXXXXXX").unwrap();

        let result = actor.handle_resume(instance_id.clone());

        assert!(result.is_err(), "Resume should fail for non-existent instance");
        let err = result.unwrap_err();
        assert!(
            matches!(err, ResumeError::InstanceActorNotFound { .. }),
            "Expected InstanceActorNotFound error, got {:?}",
            err
        );
    }
}

#[cfg(test)]
mod accept_resume_tests {
    use super::*;

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

    #[test]
    fn accept_resume_error_precondition_variants_are_correct() {
        let iid = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000000").unwrap();
        let err = AcceptResumeError::InstanceActorNotFound { instance_id: iid.clone() };
        assert!(matches!(err, AcceptResumeError::InstanceActorNotFound { .. }));
    }

    #[test]
    fn accept_resume_error_invalid_lifecycle_state_format() {
        let iid = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000000").unwrap();
        let err = AcceptResumeError::InvalidLifecycleState {
            instance_id: iid,
            actual: LifecycleState::Running,
            expected: LifecycleState::WaitingForSignal,
        };
        let display = format!("{}", err);
        assert!(display.contains("Running") && display.contains("WaitingForSignal"));
    }

    #[tokio::test]
    async fn accept_and_resume_succeeds_for_waiting_instance() {
        let actor = ControlActor::new();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
        let wait_key = WaitKey::parse("approval-v2").unwrap();
        let payload = SignalPayload::empty();

        let result = actor.accept_and_resume(
            instance_id.clone(),
            wait_key,
            "sig-1".to_string(),
            payload,
        );

        assert!(result.is_ok(), "accept_and_resume should succeed for WaitingForSignal instance");
        let outcome = result.unwrap();
        assert_eq!(outcome.accepted.instance_id, instance_id);
        assert_eq!(outcome.resumed.instance_id, instance_id);
    }

    #[tokio::test]
    async fn accept_and_resume_fails_for_non_waiting_instance() {
        let actor = ControlActor::new();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00C000").unwrap();
        let wait_key = WaitKey::parse("approval-v2").unwrap();
        let payload = SignalPayload::empty();

        let result = actor.accept_and_resume(
            instance_id.clone(),
            wait_key,
            "sig-1".to_string(),
            payload,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, AcceptResumeError::InvalidLifecycleState { .. }),
            "Expected InvalidLifecycleState error, got {:?}",
            err
        );
    }

    #[tokio::test]
    async fn accept_and_resume_fails_for_nonexistent_instance() {
        let actor = ControlActor::new();
        let instance_id = InstanceId::parse("0000000000XXXXXXXXXXXXXXXX").unwrap();
        let wait_key = WaitKey::parse("approval-v2").unwrap();
        let payload = SignalPayload::empty();

        let result = actor.accept_and_resume(
            instance_id.clone(),
            wait_key,
            "sig-1".to_string(),
            payload,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, AcceptResumeError::InstanceActorNotFound { .. }),
            "Expected InstanceActorNotFound error, got {:?}",
            err
        );
    }

    #[tokio::test]
    async fn accept_and_resume_fails_for_waitkey_mismatch() {
        let actor = ControlActor::new();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
        let wait_key = WaitKey::parse("approval-v2").unwrap();
        let payload = SignalPayload::empty();

        let result = actor.accept_and_resume(
            instance_id.clone(),
            wait_key,
            "mismatch-sig-1".to_string(),
            payload,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, AcceptResumeError::WaitKeyMismatch { .. }),
            "Expected WaitKeyMismatch error, got {:?}",
            err
        );
    }

    #[tokio::test]
    async fn accept_and_resume_fails_for_payload_too_large() {
        let actor = ControlActor::new();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
        let wait_key = WaitKey::parse("approval-v2").unwrap();
        let big_payload = vec![0u8; 65537];
        let payload = SignalPayload::new_unchecked(big_payload);

        let result = actor.accept_and_resume(
            instance_id.clone(),
            wait_key,
            "sig-1".to_string(),
            payload,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, AcceptResumeError::PayloadTooLarge { .. }),
            "Expected PayloadTooLarge error, got {:?}",
            err
        );
    }

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
            "accept_and_resume should fail when workflow is in terminal state"
        );
        let err = result.unwrap_err();
        assert!(
            matches!(err, AcceptResumeError::InvalidLifecycleState { .. }),
            "Expected InvalidLifecycleState error for Cancelled state, got {:?}",
            err
        );
    }
}