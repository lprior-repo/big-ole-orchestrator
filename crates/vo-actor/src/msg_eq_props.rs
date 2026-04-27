//! Eq property tests (reflexive, symmetric, transitive) for InstanceActorMessage and ControlActorMessage.

use crate::{
    control_msgs::ControlActorMessage, instance_msgs::InstanceActorMessage,
    signal_messages::NodeName, InstanceId, WorkflowName,
};

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
