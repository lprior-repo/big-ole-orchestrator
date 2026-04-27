//! Send + Sync and ractor::Message trait compile-time verification.

use crate::{control_msgs::ControlActorMessage, instance_msgs::InstanceActorMessage};

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
