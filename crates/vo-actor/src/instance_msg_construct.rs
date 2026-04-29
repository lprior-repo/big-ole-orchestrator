//! InstanceActorMessage constructor and debug format tests.

use crate::{instance_msgs::InstanceActorMessage, signal_messages::NodeName};
use vo_types::{InstanceId, SequenceNumber, TimerId, WorkflowName};

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
        assert_eq!(debug_str, "StartWorkflow { instance_id: InstanceId(\"01H5JYV4XHGSR2F8KZ9BWNRFMA\"), workflow_name: WorkflowName(\"deploy-prod\"), node_name: NodeName(\"build-step\") }");
    }

    #[test]
    fn step_completed_debug_format_is_exact_string() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let node_name = NodeName::parse("compile-step").unwrap();
        let sequence = SequenceNumber::new_unchecked(1);
        let message = InstanceActorMessage::new_step_completed(instance_id, node_name, sequence);
        let debug_str = format!("{:?}", message);
        assert_eq!(debug_str, "StepCompleted { instance_id: InstanceId(\"01H5JYV4XHGSR2F8KZ9BWNRFMA\"), node_name: NodeName(\"compile-step\"), sequence: SequenceNumber(1) }");
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
        assert_eq!(debug_str, "StepFailed { instance_id: InstanceId(\"01H5JYV4XHGSR2F8KZ9BWNRFMA\"), node_name: NodeName(\"compile-step\"), sequence: SequenceNumber(42), error: \"connection timeout\" }");
    }

    #[test]
    fn timer_fired_debug_format_is_exact_string() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let timer_id = TimerId::parse("timer-abc-123").unwrap();
        let message = InstanceActorMessage::new_timer_fired(instance_id, timer_id);
        let debug_str = format!("{:?}", message);
        assert_eq!(debug_str, "TimerFired { instance_id: InstanceId(\"01H5JYV4XHGSR2F8KZ9BWNRFMA\"), timer_id: TimerId(\"timer-abc-123\") }");
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
