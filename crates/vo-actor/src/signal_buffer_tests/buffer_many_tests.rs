use super::helpers::*;

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
        assert_eq!(
            buffer.pop_buffered(&id, &key).unwrap().signal_id.as_str(),
            "sig-0"
        );
        assert_eq!(
            buffer.pop_buffered(&id, &key).unwrap().signal_id.as_str(),
            "sig-1"
        );
        assert_eq!(
            buffer.pop_buffered(&id, &key).unwrap().signal_id.as_str(),
            "sig-2"
        );
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
            buffer.pop_buffered(&id, &key).unwrap().signal_id.as_str(),
            "sig-single"
        );
        assert_eq!(
            buffer.pop_buffered(&id, &key).unwrap().signal_id.as_str(),
            "sig-many-1"
        );
    }
}
