use super::helpers::*;

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
    fn buffer_one_rejects_subsequent_signals_until_first_is_consumed() {
        let mut buffer = SignalBuffer::with_default_config();
        let first = buffer.buffer_signal(
            instance_id_a(),
            wait_key_approval(),
            make_signal("sig-first"),
            BufferPolicy::BufferOne,
        );
        assert_eq!(first, BufferResult::Buffered);
        assert_eq!(
            buffer.buffered_count(&instance_id_a(), &wait_key_approval()),
            1
        );
        let second = buffer.buffer_signal(
            instance_id_a(),
            wait_key_approval(),
            make_signal("sig-second"),
            BufferPolicy::BufferOne,
        );
        assert_eq!(second, BufferResult::Rejected);
        assert_eq!(
            buffer.buffered_count(&instance_id_a(), &wait_key_approval()),
            1
        );
        assert_eq!(
            buffer.peek_all(&instance_id_a(), &wait_key_approval())[0]
                .signal_id
                .as_str(),
            "sig-first"
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
        assert_eq!(popped.unwrap().signal_id.as_str(), "sig-pop");
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

    #[test]
    fn buffer_one_accepts_new_signal_after_pop() {
        let mut buffer = SignalBuffer::with_default_config();
        let _ = buffer.buffer_signal(
            instance_id_a(),
            wait_key_approval(),
            make_signal("sig-original"),
            BufferPolicy::BufferOne,
        );
        let _ = buffer.pop_buffered(&instance_id_a(), &wait_key_approval());
        let result = buffer.buffer_signal(
            instance_id_a(),
            wait_key_approval(),
            make_signal("sig-after-pop"),
            BufferPolicy::BufferOne,
        );
        assert_eq!(result, BufferResult::Buffered);
        assert_eq!(
            buffer.buffered_count(&instance_id_a(), &wait_key_approval()),
            1
        );
    }

    #[test]
    fn buffer_one_rejects_subsequent_signals_until_first_is_consumed_duplicate_for_schema() {
        let mut buffer = SignalBuffer::with_default_config();
        let first = buffer.buffer_signal(
            instance_id_a(),
            wait_key_approval(),
            make_signal("sig-first-dup"),
            BufferPolicy::BufferOne,
        );
        assert_eq!(first, BufferResult::Buffered);
        let second = buffer.buffer_signal(
            instance_id_a(),
            wait_key_approval(),
            make_signal("sig-second-dup"),
            BufferPolicy::BufferOne,
        );
        assert_eq!(second, BufferResult::Rejected);
    }
}
