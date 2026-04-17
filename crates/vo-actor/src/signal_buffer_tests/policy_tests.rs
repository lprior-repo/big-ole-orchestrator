use super::helpers::*;

mod apply_policy_tests {
    use super::*;

    #[test]
    fn apply_policy_accepts_when_matching_wait_exists() {
        for policy in &[
            BufferPolicy::Reject,
            BufferPolicy::BufferOne,
            BufferPolicy::BufferMany,
        ] {
            let (delivery, _) = apply_policy(*policy, true, false);
            assert_eq!(delivery, SignalDelivery::Accepted);
        }
    }

    #[test]
    fn apply_policy_rejects_when_no_wait_and_reject_policy() {
        let (delivery, result) = apply_policy(BufferPolicy::Reject, false, false);
        assert_eq!(delivery, SignalDelivery::Rejected);
        assert_eq!(result, Some(BufferResult::Rejected));
    }

    #[test]
    fn apply_policy_buffers_bufferone_when_no_wait() {
        let (delivery, result) = apply_policy(BufferPolicy::BufferOne, false, false);
        assert_eq!(delivery, SignalDelivery::Buffered);
        assert_eq!(result, Some(BufferResult::Buffered));
    }

    #[test]
    fn apply_policy_buffers_many_when_no_wait() {
        let (delivery, result) = apply_policy(BufferPolicy::BufferMany, false, false);
        assert_eq!(delivery, SignalDelivery::Buffered);
        assert_eq!(result, Some(BufferResult::Buffered));
    }

    #[test]
    fn apply_policy_ignores_existing_buffer_for_accept() {
        let (delivery, result) = apply_policy(BufferPolicy::BufferOne, true, true);
        assert_eq!(delivery, SignalDelivery::Accepted);
        assert_eq!(result, None);
    }
}

mod can_buffer_tests {
    use super::*;

    fn config_5() -> SignalBufferConfig {
        SignalBufferConfig::new(5)
    }

    #[test]
    fn can_buffer_reject_always_false() {
        assert!(!can_buffer(
            BufferPolicy::Reject,
            false,
            0,
            &default_config()
        ));
        assert!(!can_buffer(
            BufferPolicy::Reject,
            true,
            0,
            &default_config()
        ));
    }

    #[test]
    fn can_buffer_bufferone_always_true() {
        assert!(can_buffer(
            BufferPolicy::BufferOne,
            false,
            100,
            &default_config()
        ));
        assert!(can_buffer(
            BufferPolicy::BufferOne,
            true,
            100,
            &default_config()
        ));
    }

    #[test]
    fn can_buffer_many_true_when_under_limit() {
        let config = config_5();
        assert!(can_buffer(BufferPolicy::BufferMany, false, 0, &config));
        assert!(can_buffer(BufferPolicy::BufferMany, false, 4, &config));
    }

    #[test]
    fn can_buffer_many_false_when_at_limit() {
        let config = config_5();
        assert!(!can_buffer(BufferPolicy::BufferMany, false, 5, &config));
        assert!(!can_buffer(BufferPolicy::BufferMany, false, 10, &config));
    }
}
