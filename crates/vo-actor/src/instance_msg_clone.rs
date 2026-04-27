//! InstanceActorMessage clone tests.

use crate::{
    instance_msgs::InstanceActorMessage, signal_messages::NodeName, InstanceId, SequenceNumber,
    TimerId, WorkflowName,
};

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
