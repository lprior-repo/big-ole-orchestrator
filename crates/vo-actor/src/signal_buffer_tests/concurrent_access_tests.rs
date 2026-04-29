use super::helpers::*;
use std::sync::{Arc, Mutex};
use std::thread;

mod signal_buffer_concurrent_access_tests {
    use super::*;

    fn buffer_signal_via(
        buffer: &Arc<Mutex<SignalBuffer>>,
        instance_id: InstanceId,
        wait_key: WaitKey,
        signal_id: String,
        policy: BufferPolicy,
    ) -> BufferResult {
        let mut buf = buffer.lock().unwrap();
        buf.buffer_signal(instance_id, wait_key, make_signal(&signal_id), policy)
    }

    #[test]
    fn ten_concurrent_signals_buffered_safely() {
        let buffer = Arc::new(Mutex::new(SignalBuffer::new(SignalBufferConfig::new(20))));
        let id = instance_id_a();
        let key = wait_key_approval();
        let mut handles = vec![];

        for i in 0..10 {
            let buf = Arc::clone(&buffer);
            let id = id.clone();
            let key = key.clone();
            handles.push(thread::spawn(move || {
                buffer_signal_via(
                    &buf,
                    id,
                    key,
                    format!("sig-{i}"),
                    BufferPolicy::BufferMany,
                )
            }));
        }

        let results: Vec<BufferResult> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let buffered_count = results.iter().filter(|r| **r == BufferResult::Buffered).count();
        assert_eq!(buffered_count, 10);

        let buf = buffer.lock().unwrap();
        assert_eq!(buf.buffered_count(&id, &key), 10);

        let mut buf = buffer.lock().unwrap();
        let mut signal_ids: Vec<String> = (0..10)
            .filter_map(|_| buf.pop_buffered(&id, &key).map(|s| s.signal_id))
            .collect();
        signal_ids.sort();
        let expected: Vec<String> = (0..10).map(|i| format!("sig-{i}")).collect();
        assert_eq!(signal_ids, expected);
    }

    #[test]
    fn hundred_concurrent_signals_no_corruption() {
        let buffer = Arc::new(Mutex::new(SignalBuffer::new(SignalBufferConfig::new(200))));
        let id = instance_id_a();
        let key = wait_key_approval();
        let mut handles = vec![];

        for i in 0..100 {
            let buf = Arc::clone(&buffer);
            let id = id.clone();
            let key = key.clone();
            handles.push(thread::spawn(move || {
                buffer_signal_via(
                    &buf,
                    id,
                    key,
                    format!("sig-{i}"),
                    BufferPolicy::BufferMany,
                )
            }));
        }

        let results: Vec<BufferResult> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let buffered_count = results.iter().filter(|r| **r == BufferResult::Buffered).count();
        assert_eq!(buffered_count, 100);

        let mut buf = buffer.lock().unwrap();
        assert_eq!(buf.buffered_count(&id, &key), 100);

        let mut signal_ids: Vec<String> = (0..100)
            .filter_map(|_| buf.pop_buffered(&id, &key).map(|s| s.signal_id))
            .collect();
        signal_ids.sort();
        let expected: Vec<String> = (0..100).map(|i| format!("sig-{i}")).collect();
        assert_eq!(signal_ids, expected);
    }

    #[test]
    fn concurrent_pop_preserves_fifo_no_loss() {
        let buffer = Arc::new(Mutex::new(SignalBuffer::new(SignalBufferConfig::new(20))));
        let id = instance_id_a();
        let key = wait_key_approval();

        {
            let mut buf = buffer.lock().unwrap();
            for i in 0..10 {
                buf.buffer_signal(
                    id.clone(),
                    key.clone(),
                    make_signal(&format!("sig-{i}")),
                    BufferPolicy::BufferMany,
                );
            }
        }

        let mut handles = vec![];
        for _ in 0..10 {
            let buf = Arc::clone(&buffer);
            let id = id.clone();
            let key = key.clone();
            handles.push(thread::spawn(move || {
                let mut buf = buf.lock().unwrap();
                buf.pop_buffered(&id, &key)
            }));
        }

        let popped: Vec<String> = handles
            .into_iter()
            .filter_map(|h| h.join().unwrap().map(|s| s.signal_id))
            .collect();

        assert_eq!(popped.len(), 10);
        let buf = buffer.lock().unwrap();
        assert_eq!(buf.buffered_count(&id, &key), 0);
    }

    #[test]
    fn concurrent_buffer_different_keys_isolation() {
        let buffer = Arc::new(Mutex::new(SignalBuffer::new(SignalBufferConfig::new(20))));
        let id = instance_id_a();
        let key_a = wait_key_approval();
        let key_b = wait_key_notif();
        let mut handles = vec![];

        for i in 0..10 {
            let buf = Arc::clone(&buffer);
            let id = id.clone();
            let key = if i % 2 == 0 {
                key_a.clone()
            } else {
                key_b.clone()
            };
            handles.push(thread::spawn(move || {
                buffer_signal_via(
                    &buf,
                    id,
                    key,
                    format!("sig-{i}"),
                    BufferPolicy::BufferMany,
                )
            }));
        }

        for h in handles {
            let _ = h.join().unwrap();
        }

        let buf = buffer.lock().unwrap();
        assert_eq!(buf.buffered_count(&id, &key_a), 5);
        assert_eq!(buf.buffered_count(&id, &key_b), 5);
    }

    #[test]
    fn concurrent_interleaved_buffer_and_pop_no_loss() {
        let buffer = Arc::new(Mutex::new(SignalBuffer::new(SignalBufferConfig::new(50))));
        let id = instance_id_a();
        let key = wait_key_approval();
        let mut buffer_handles = vec![];
        let mut pop_handles = vec![];

        for i in 0..10 {
            let buf = Arc::clone(&buffer);
            let id = id.clone();
            let key = key.clone();
            buffer_handles.push(thread::spawn(move || {
                buffer_signal_via(
                    &buf,
                    id,
                    key,
                    format!("sig-{i}"),
                    BufferPolicy::BufferMany,
                )
            }));
        }

        for _ in 0..10 {
            let buf = Arc::clone(&buffer);
            let id = id.clone();
            let key = key.clone();
            pop_handles.push(thread::spawn(move || {
                let mut buf = buf.lock().unwrap();
                buf.pop_buffered(&id, &key)
            }));
        }

        for h in buffer_handles {
            let _ = h.join().unwrap();
        }
        for h in pop_handles {
            let _ = h.join().unwrap();
        }

        let buf = buffer.lock().unwrap();
        let remaining = buf.buffered_count(&id, &key);
        assert_eq!(remaining, 0);
    }

    #[test]
    fn concurrent_buffer_one_rejects_duplicates_safely() {
        let buffer = Arc::new(Mutex::new(SignalBuffer::new(SignalBufferConfig::new(20))));
        let id = instance_id_a();
        let key = wait_key_approval();
        let mut handles = vec![];

        for i in 0..10 {
            let buf = Arc::clone(&buffer);
            let id = id.clone();
            let key = key.clone();
            handles.push(thread::spawn(move || {
                buffer_signal_via(
                    &buf,
                    id,
                    key,
                    format!("sig-{i}"),
                    BufferPolicy::BufferOne,
                )
            }));
        }

        let results: Vec<BufferResult> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let buffered = results
            .iter()
            .filter(|r| **r == BufferResult::Buffered)
            .count();
        let rejected = results
            .iter()
            .filter(|r| **r == BufferResult::Rejected)
            .count();

        assert_eq!(buffered, 1);
        assert_eq!(rejected, 9);

        let buf = buffer.lock().unwrap();
        assert_eq!(buf.buffered_count(&id, &key), 1);
    }
}
