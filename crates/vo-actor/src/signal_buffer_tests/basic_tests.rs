use super::helpers::*;

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
            buffer.buffered_count(&instance_id_a(), wait_key_approval()),
            0
        );
    }

    #[test]
    fn buffer_has_buffered_signals_false_for_unknown_key() {
        let buffer = SignalBuffer::with_default_config();
        assert!(!buffer.has_buffered_signals(&instance_id_a(), wait_key_approval()));
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
