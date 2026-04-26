//! ControlActorMessage clone tests.

use crate::{
    control_msgs::ControlActorMessage,
    signal_messages::{SignalPayload, WaitKey},
    InstanceId,
};

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
