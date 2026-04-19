use super::helpers::*;

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
