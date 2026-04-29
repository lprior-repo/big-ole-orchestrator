//! PartialEq tests for InstanceActorMessage and ControlActorMessage.

use crate::{
    control_msgs::ControlActorMessage, instance_msgs::InstanceActorMessage,
    signal_messages::NodeName,
};
use vo_types::{InstanceId, SequenceNumber, WorkflowName};

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
