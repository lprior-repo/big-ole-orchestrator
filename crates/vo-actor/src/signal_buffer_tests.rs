#![cfg(test)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

use crate::signal_buffer::{
    apply_policy, can_buffer, BufferResult, BufferedSignal, SignalBuffer, SignalBufferConfig,
};
use crate::WaitKey;
use vo_types::InstanceId;
use vo_types::{BufferPolicy, SignalDelivery};

fn instance_id_a() -> InstanceId {
    InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap()
}
fn instance_id_b() -> InstanceId {
    InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap()
}
fn wait_key_approval() -> WaitKey {
    WaitKey::parse("approval").unwrap()
}
fn wait_key_notif() -> WaitKey {
    WaitKey::parse("notification").unwrap()
}
fn make_signal(signal_id: &str) -> BufferedSignal {
    BufferedSignal::new(
        signal_id.to_string(),
        crate::SignalPayload::empty(),
        vo_types::TimestampMs::now(),
    )
}
fn default_config() -> SignalBufferConfig {
    SignalBufferConfig::default()
}

mod signal_buffer_config_tests {
    use super::*;
    #[test]
    fn config_default_max_buffered_per_key_is_100() {
        assert_eq!(SignalBufferConfig::default().max_buffered_per_key, 100);
    }
    #[test]
    fn config_new_with_zero_yields_one() {
        assert_eq!(SignalBufferConfig::new(0).max_buffered_per_key, 1);
    }
    #[test]
    fn config_new_with_50_yields_50() {
        assert_eq!(SignalBufferConfig::new(50).max_buffered_per_key, 50);
    }
    #[test]
    fn config_equality() {
        assert_eq!(SignalBufferConfig::new(100), SignalBufferConfig::new(100));
        assert_ne!(SignalBufferConfig::new(100), SignalBufferConfig::new(50));
    }
}

mod buffered_signal_tests {
    use super::*;
    #[test]
    fn buffered_signal_constructs_with_all_fields() {
        let payload = crate::SignalPayload::from_bytes(vec![1, 2, 3]).unwrap();
        let ts = vo_types::TimestampMs::now();
        let signal = BufferedSignal::new("sig-42".to_string(), payload.clone(), ts);
        assert_eq!(signal.signal_id, "sig-42");
        assert_eq!(signal.payload.as_bytes(), &[1, 2, 3]);
        assert_eq!(signal.buffered_at, ts);
    }
    #[test]
    fn buffered_signal_clone_is_independent() {
        let signal = make_signal("sig-clone");
        assert_eq!(signal, signal.clone());
    }
}

mod buffer_result_tests {
    use super::*;
    #[test]
    fn buffer_result_variants() {
        assert!(format!("{:?}", BufferResult::Rejected).contains("Rejected"));
        assert!(format!("{:?}", BufferResult::Buffered).contains("Buffered"));
        assert!(format!("{:?}", BufferResult::Dropped).contains("Dropped"));
    }
}

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

mod signal_buffer_basic_tests {
    use super::*;
    #[test]
    fn buffer_starts_empty() {
        let buffer = SignalBuffer::with_default_config();
        assert_eq!(buffer.num_keys_with_signals(), 0);
        assert_eq!(buffer.total_buffered_count(), 0);
    }
    #[test]
    fn buffer_count_returns_zero_for_unknown_key() {
        let buffer = SignalBuffer::with_default_config();
        assert_eq!(
            buffer.buffered_count(&instance_id_a(), &wait_key_approval()),
            0
        );
    }
    #[test]
    fn buffer_has_buffered_signals_false_for_unknown_key() {
        let buffer = SignalBuffer::with_default_config();
        assert!(!buffer.has_buffered_signals(&instance_id_a(), &wait_key_approval()));
    }
    #[test]
    fn buffer_signal_reject_returns_rejected() {
        let mut buffer = SignalBuffer::with_default_config();
        let result = buffer.buffer_signal(
            instance_id_a(),
            wait_key_approval(),
            make_signal("sig-1"),
            BufferPolicy::Reject,
        );
        assert_eq!(result, BufferResult::Rejected);
        assert_eq!(buffer.total_buffered_count(), 0);
    }
}

mod signal_buffer_one_tests {
    use super::*;
    #[test]
    fn buffer_one_stores_signal() {
        let mut buffer = SignalBuffer::with_default_config();
        let result = buffer.buffer_signal(
            instance_id_a(),
            wait_key_approval(),
            make_signal("sig-1"),
            BufferPolicy::BufferOne,
        );
        assert_eq!(result, BufferResult::Buffered);
        assert_eq!(
            buffer.buffered_count(&instance_id_a(), &wait_key_approval()),
            1
        );
        assert!(buffer.has_buffered_signals(&instance_id_a(), &wait_key_approval()));
    }
    #[test]
    fn buffer_one_replaces_existing_signal() {
        let mut buffer = SignalBuffer::with_default_config();
        let first = buffer.buffer_signal(
            instance_id_a(),
            wait_key_approval(),
            make_signal("sig-old"),
            BufferPolicy::BufferOne,
        );
        assert_eq!(first, BufferResult::Buffered);
        let second = buffer.buffer_signal(
            instance_id_a(),
            wait_key_approval(),
            make_signal("sig-new"),
            BufferPolicy::BufferOne,
        );
        assert_eq!(second, BufferResult::Buffered);
        assert_eq!(
            buffer.buffered_count(&instance_id_a(), &wait_key_approval()),
            1
        );
        assert_eq!(
            buffer.peek_all(&instance_id_a(), &wait_key_approval())[0].signal_id,
            "sig-new"
        );
    }
    #[test]
    fn buffer_one_allows_different_keys() {
        let mut buffer = SignalBuffer::with_default_config();
        let r1 = buffer.buffer_signal(
            instance_id_a(),
            wait_key_approval(),
            make_signal("sig-approval"),
            BufferPolicy::BufferOne,
        );
        assert_eq!(r1, BufferResult::Buffered);
        let r2 = buffer.buffer_signal(
            instance_id_a(),
            wait_key_notif(),
            make_signal("sig-notif"),
            BufferPolicy::BufferOne,
        );
        assert_eq!(r2, BufferResult::Buffered);
        assert_eq!(buffer.total_buffered_count(), 2);
    }
    #[test]
    fn buffer_one_pop_returns_and_removes_signal() {
        let mut buffer = SignalBuffer::with_default_config();
        let _ = buffer.buffer_signal(
            instance_id_a(),
            wait_key_approval(),
            make_signal("sig-pop"),
            BufferPolicy::BufferOne,
        );
        let popped = buffer.pop_buffered(&instance_id_a(), &wait_key_approval());
        assert!(popped.is_some());
        assert_eq!(popped.unwrap().signal_id, "sig-pop");
        assert_eq!(
            buffer.buffered_count(&instance_id_a(), &wait_key_approval()),
            0
        );
    }
    #[test]
    fn buffer_one_pop_none_for_unknown_key() {
        let mut buffer = SignalBuffer::with_default_config();
        assert!(buffer
            .pop_buffered(&instance_id_a(), &wait_key_approval())
            .is_none());
    }
    #[test]
    fn buffer_one_clear_removes_signal() {
        let mut buffer = SignalBuffer::with_default_config();
        let _ = buffer.buffer_signal(
            instance_id_a(),
            wait_key_approval(),
            make_signal("sig-clear"),
            BufferPolicy::BufferOne,
        );
        buffer.clear(&instance_id_a(), &wait_key_approval());
        assert_eq!(
            buffer.buffered_count(&instance_id_a(), &wait_key_approval()),
            0
        );
    }
}

mod signal_buffer_many_tests {
    use super::*;
    #[test]
    fn buffer_many_stores_multiple_signals() {
        let mut buffer = SignalBuffer::with_default_config();
        let id = instance_id_a();
        let key = wait_key_approval();
        for i in 0..4 {
            let result = buffer.buffer_signal(
                id.clone(),
                key.clone(),
                make_signal(&format!("sig-{i}")),
                BufferPolicy::BufferMany,
            );
            assert_eq!(result, BufferResult::Buffered);
        }
        assert_eq!(buffer.buffered_count(&id, &key), 4);
    }
    #[test]
    fn buffer_many_respects_max_limit() {
        let mut buffer = SignalBuffer::new(SignalBufferConfig::new(3));
        let id = instance_id_a();
        let key = wait_key_approval();
        for i in 0..3 {
            let result = buffer.buffer_signal(
                id.clone(),
                key.clone(),
                make_signal(&format!("sig-{i}")),
                BufferPolicy::BufferMany,
            );
            assert_eq!(result, BufferResult::Buffered);
        }
        let result = buffer.buffer_signal(
            id.clone(),
            key.clone(),
            make_signal("sig-overflow"),
            BufferPolicy::BufferMany,
        );
        assert_eq!(result, BufferResult::Dropped);
        assert_eq!(buffer.buffered_count(&id, &key), 3);
    }
    #[test]
    fn buffer_many_fifo_order() {
        let mut buffer = SignalBuffer::with_default_config();
        let id = instance_id_a();
        let key = wait_key_approval();
        for i in 0..3 {
            let _ = buffer.buffer_signal(
                id.clone(),
                key.clone(),
                make_signal(&format!("sig-{i}")),
                BufferPolicy::BufferMany,
            );
        }
        assert_eq!(buffer.pop_buffered(&id, &key).unwrap().signal_id, "sig-0");
        assert_eq!(buffer.pop_buffered(&id, &key).unwrap().signal_id, "sig-1");
        assert_eq!(buffer.pop_buffered(&id, &key).unwrap().signal_id, "sig-2");
        assert!(buffer.pop_buffered(&id, &key).is_none());
    }
    #[test]
    fn buffer_many_peek_all_returns_all_without_removing() {
        let mut buffer = SignalBuffer::with_default_config();
        let id = instance_id_a();
        let key = wait_key_approval();
        for i in 0..3 {
            let _ = buffer.buffer_signal(
                id.clone(),
                key.clone(),
                make_signal(&format!("sig-{i}")),
                BufferPolicy::BufferMany,
            );
        }
        assert_eq!(buffer.peek_all(&id, &key).len(), 3);
        assert_eq!(buffer.peek_all(&id, &key).len(), 3);
    }
    #[test]
    fn buffer_many_separate_keys_independent() {
        let mut buffer = SignalBuffer::with_default_config();
        let id_a = instance_id_a();
        let id_b = instance_id_b();
        let key = wait_key_approval();
        let _ = buffer.buffer_signal(
            id_a.clone(),
            key.clone(),
            make_signal("sig-a1"),
            BufferPolicy::BufferMany,
        );
        let _ = buffer.buffer_signal(
            id_a.clone(),
            key.clone(),
            make_signal("sig-a2"),
            BufferPolicy::BufferMany,
        );
        let _ = buffer.buffer_signal(
            id_b.clone(),
            key.clone(),
            make_signal("sig-b1"),
            BufferPolicy::BufferMany,
        );
        assert_eq!(buffer.buffered_count(&id_a, &key), 2);
        assert_eq!(buffer.buffered_count(&id_b, &key), 1);
    }
    #[test]
    fn buffer_many_migrates_from_single() {
        let mut buffer = SignalBuffer::with_default_config();
        let id = instance_id_a();
        let key = wait_key_approval();
        let _ = buffer.buffer_signal(
            id.clone(),
            key.clone(),
            make_signal("sig-single"),
            BufferPolicy::BufferOne,
        );
        assert_eq!(buffer.buffered_count(&id, &key), 1);
        let _ = buffer.buffer_signal(
            id.clone(),
            key.clone(),
            make_signal("sig-many-1"),
            BufferPolicy::BufferMany,
        );
        assert_eq!(buffer.buffered_count(&id, &key), 2);
        assert_eq!(
            buffer.pop_buffered(&id, &key).unwrap().signal_id,
            "sig-single"
        );
        assert_eq!(
            buffer.pop_buffered(&id, &key).unwrap().signal_id,
            "sig-many-1"
        );
    }
}

mod signal_buffer_clear_tests {
    use super::*;
    #[test]
    fn clear_unknown_key_is_noop() {
        let mut buffer = SignalBuffer::with_default_config();
        buffer.clear(&instance_id_a(), &wait_key_approval());
        assert_eq!(buffer.total_buffered_count(), 0);
    }
    #[test]
    fn clear_removes_all_signals_for_key() {
        let mut buffer = SignalBuffer::with_default_config();
        let id = instance_id_a();
        let key = wait_key_approval();
        let _ = buffer.buffer_signal(
            id.clone(),
            key.clone(),
            make_signal("sig-1"),
            BufferPolicy::BufferMany,
        );
        let _ = buffer.buffer_signal(
            id.clone(),
            key.clone(),
            make_signal("sig-2"),
            BufferPolicy::BufferMany,
        );
        assert_eq!(buffer.buffered_count(&id, &key), 2);
        buffer.clear(&id, &key);
        assert_eq!(buffer.buffered_count(&id, &key), 0);
    }
    #[test]
    fn clear_preserves_other_keys() {
        let mut buffer = SignalBuffer::with_default_config();
        let id_a = instance_id_a();
        let id_b = instance_id_b();
        let key = wait_key_approval();
        let _ = buffer.buffer_signal(
            id_a.clone(),
            key.clone(),
            make_signal("sig-a"),
            BufferPolicy::BufferOne,
        );
        let _ = buffer.buffer_signal(
            id_b.clone(),
            key.clone(),
            make_signal("sig-b"),
            BufferPolicy::BufferOne,
        );
        buffer.clear(&id_a, &key);
        assert_eq!(buffer.buffered_count(&id_a, &key), 0);
        assert_eq!(buffer.buffered_count(&id_b, &key), 1);
    }
}

mod signal_buffer_config_access_tests {
    use super::*;
    #[test]
    fn config_returns_stored_config() {
        let config = SignalBufferConfig::new(42);
        let buffer = SignalBuffer::new(config);
        assert_eq!(buffer.config().max_buffered_per_key, 42);
    }
}
