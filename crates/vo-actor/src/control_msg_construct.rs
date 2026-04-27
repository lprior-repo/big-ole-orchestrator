//! ControlActorMessage constructor and debug format tests.

use crate::{
    control_msgs::ControlActorMessage,
    signal_messages::{SignalName, SignalPayload, WaitKey},
    InstanceId,
};

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
        let wait_key = WaitKey::parse("approval-v2").unwrap();
        let payload = SignalPayload::empty();
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
        let wait_key = WaitKey::parse("approval-v2").unwrap();
        let payload = SignalPayload::empty();
        let signal_name = SignalName::parse("sig-1").unwrap();
        let message =
            ControlActorMessage::new_accept_and_resume(instance_id, wait_key, signal_name, payload);
        let debug_str = format!("{:?}", message);
        assert!(debug_str.contains("AcceptAndResume"));
        assert!(debug_str.contains("approval-v2"));
    }
}
