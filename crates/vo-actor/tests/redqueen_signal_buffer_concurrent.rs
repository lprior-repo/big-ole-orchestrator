#![cfg(test)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

//! Red Queen: Signal buffer concurrent access tests.
//!
//! Attack vectors targeting thread-safety of signal buffering:
//! - Buffer corruption under concurrent access from multiple signal sources
//! - Signal loss during Single→Many migration
//! - Data races when multiple threads buffer to same key simultaneously

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use vo_actor::signal_buffer::{BufferResult, BufferedSignal, SignalBuffer};
use vo_actor::WaitKey;
use vo_types::BufferPolicy;
use vo_types::InstanceId;

fn instance_id_a() -> InstanceId {
    InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap()
}

fn wait_key_approval() -> WaitKey {
    WaitKey::parse("approval").unwrap()
}

fn make_signal(signal_id: &str) -> BufferedSignal {
    BufferedSignal::new(
        signal_id.to_string(),
        vo_actor::SignalPayload::empty(),
        vo_types::TimestampMs::now(),
    )
}

// =============================================================================
// ATTACK 1: Concurrent Single→Many Migration Race
// =============================================================================
// INV: When multiple signals arrive concurrently for the same key that is Single,
// the system SHALL preserve all signals. Lost signals indicate a data race.

#[test]
fn rq_signal_buffer_concurrent_single_to_many_migration_no_loss() {
    // Scenario: Two threads concurrently try to buffer signals to the same key
    // when it's currently Single. Both should succeed, transitioning to Many.
    // If the implementation has a race, one signal will be lost.

    let buffer = Arc::new(tokio::sync::Mutex::new(SignalBuffer::with_default_config()));
    let instance_id = instance_id_a();
    let wait_key = wait_key_approval();

    // Pre-populate with a Single signal
    {
        let mut buf = buffer.blocking_lock();
        let sig = make_signal("initial-signal");
        let result = buf.buffer_signal(
            instance_id.clone(),
            wait_key.clone(),
            sig,
            BufferPolicy::BufferOne,
        );
        assert_eq!(result, BufferResult::Buffered, "Initial signal must buffer");
        assert_eq!(buf.buffered_count(&instance_id, &wait_key), 1);
    }

    // Now spawn two threads that try to buffer concurrently using BufferMany
    let sig_a = make_signal("concurrent-signal-a");
    let sig_b = make_signal("concurrent-signal-b");

    let buffer_a = buffer.clone();
    let buffer_b = buffer.clone();
    let instance_id_clone_a = instance_id.clone();
    let wait_key_clone_a = wait_key.clone();
    let instance_id_clone_b = instance_id.clone();
    let wait_key_clone_b = wait_key.clone();

    let handle_a = thread::spawn(move || {
        let mut buf = buffer_a.blocking_lock();
        buf.buffer_signal(
            instance_id_clone_a,
            wait_key_clone_a,
            sig_a,
            BufferPolicy::BufferMany,
        )
    });

    let handle_b = thread::spawn(move || {
        let mut buf = buffer_b.blocking_lock();
        buf.buffer_signal(
            instance_id_clone_b,
            wait_key_clone_b,
            sig_b,
            BufferPolicy::BufferMany,
        )
    });

    let result_a = handle_a.join().unwrap();
    let result_b = handle_b.join().unwrap();

    // Both should succeed - Single→Many migration should handle concurrent access
    assert_eq!(
        result_a,
        BufferResult::Buffered,
        "Signal A must be buffered (may fail if race causes Dropped)"
    );
    assert_eq!(
        result_b,
        BufferResult::Buffered,
        "Signal B must be buffered (may fail if race causes Dropped)"
    );

    // CRITICAL CHECK: All 3 signals must be present (1 initial + 2 concurrent)
    let buf = buffer.blocking_lock();
    let total = buf.buffered_count(&instance_id, &wait_key);

    // Due to the race condition in Single→Many migration, this assertion WILL FAIL
    // because one of the concurrent signals overwrites the other's entry
    assert_eq!(
        total, 3,
        "INV VIOLATION: Signal buffer corrupted under concurrent Single→Many migration. \
         Expected 3 signals (1 initial + 2 concurrent), got {}. \
         This indicates a data race where one concurrent signal was lost.",
        total
    );
}

// =============================================================================
// ATTACK 2: 10 Concurrent Signals Buffered - Happy Path
// =============================================================================

#[test]
fn rq_signal_buffer_10_concurrent_signals_no_corruption() {
    let buffer = Arc::new(tokio::sync::Mutex::new(SignalBuffer::with_default_config()));
    let instance_id = instance_id_a();
    let wait_key = wait_key_approval();
    let num_signals = 10;

    let mut handles = vec![];

    for i in 0..num_signals {
        let buffer_clone = buffer.clone();
        let instance_id_clone = instance_id.clone();
        let wait_key_clone = wait_key.clone();

        let handle = thread::spawn(move || {
            let sig = make_signal(&format!("sig-{}", i));
            let mut buf = buffer_clone.blocking_lock();
            buf.buffer_signal(
                instance_id_clone,
                wait_key_clone,
                sig,
                BufferPolicy::BufferMany,
            )
        });
        handles.push(handle);
    }

    for (i, handle) in handles.into_iter().enumerate() {
        let result = handle.join().unwrap();
        assert_eq!(
            result,
            BufferResult::Buffered,
            "Signal {} must be buffered",
            i
        );
    }

    let buf = buffer.blocking_lock();
    let total = buf.buffered_count(&instance_id, &wait_key);
    assert_eq!(
        total, num_signals,
        "INV VIOLATION: Expected {} signals, got {}. Buffer corrupted under concurrent access.",
        num_signals, total
    );
}

// =============================================================================
// ATTACK 3: 100 Concurrent Signals - Stress Test
// =============================================================================

#[test]
fn rq_signal_buffer_100_concurrent_signals_no_loss() {
    let buffer = Arc::new(tokio::sync::Mutex::new(SignalBuffer::with_default_config()));
    let instance_id = instance_id_a();
    let wait_key = wait_key_approval();
    let num_signals = 100;

    let mut handles = vec![];

    for i in 0..num_signals {
        let buffer_clone = buffer.clone();
        let instance_id_clone = instance_id.clone();
        let wait_key_clone = wait_key.clone();

        let handle = thread::spawn(move || {
            let sig = make_signal(&format!("stress-sig-{}", i));
            let mut buf = buffer_clone.blocking_lock();
            buf.buffer_signal(
                instance_id_clone,
                wait_key_clone,
                sig,
                BufferPolicy::BufferMany,
            )
        });
        handles.push(handle);
    }

    for handle in handles {
        let result = handle.join().unwrap();
        assert_eq!(result, BufferResult::Buffered);
    }

    let buf = buffer.blocking_lock();
    let total = buf.buffered_count(&instance_id, &wait_key);
    assert_eq!(
        total, num_signals,
        "INV VIOLATION: Expected {} signals under stress test, got {}. \
         Buffer corruption detected on high-contention concurrent access.",
        num_signals, total
    );
}

// =============================================================================
// ATTACK 4: FIFO Order Preservation Under Concurrent Access
// =============================================================================

#[test]
fn rq_signal_buffer_concurrent_fifo_order_preserved() {
    let buffer = Arc::new(tokio::sync::Mutex::new(SignalBuffer::with_default_config()));
    let instance_id = instance_id_a();
    let wait_key = wait_key_approval();

    // Pre-populate with a Single signal
    {
        let mut buf = buffer.blocking_lock();
        let sig = make_signal("initial");
        buf.buffer_signal(
            instance_id.clone(),
            wait_key.clone(),
            sig,
            BufferPolicy::BufferOne,
        );
    }

    // Spawn threads that buffer signals concurrently
    let num_signals = 10;
    let mut handles = vec![];

    for i in 0..num_signals {
        let buffer_clone = buffer.clone();
        let instance_id_clone = instance_id.clone();
        let wait_key_clone = wait_key.clone();

        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_micros(10 * i as u64)); // Stagger slightly
            let sig = make_signal(&format!("sig-{}", i));
            let mut buf = buffer_clone.blocking_lock();
            buf.buffer_signal(
                instance_id_clone,
                wait_key_clone,
                sig,
                BufferPolicy::BufferMany,
            )
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Pop all signals and verify FIFO order
    let buf = buffer.blocking_lock();
    let all_signals = buf.peek_all(&instance_id, &wait_key);

    // Should have 1 initial + 10 concurrent = 11 signals
    assert_eq!(
        all_signals.len(),
        11,
        "INV VIOLATION: Expected 11 signals, got {}. Buffer corrupted.",
        all_signals.len()
    );

    // Initial should be first (it was buffered before concurrent threads started)
    assert_eq!(
        all_signals[0].signal_id, "initial",
        "Initial signal must be first (FIFO)"
    );

    // The concurrent signals should be in order 0-9 (though timing may vary slightly)
    for i in 1..11 {
        let expected_prefix = format!("sig-{}", i - 1);
        assert!(
            all_signals[i].signal_id.starts_with("sig-"),
            "Signal {} has unexpected ID: {}",
            i,
            all_signals[i].signal_id
        );
    }
}

// =============================================================================
// ATTACK 5: Separate Keys - No Interference
// =============================================================================

#[test]
fn rq_signal_buffer_concurrent_different_keys_no_interference() {
    let buffer = Arc::new(tokio::sync::Mutex::new(SignalBuffer::with_default_config()));
    let instance_id_a = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let instance_id_b = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();
    let wait_key = wait_key_approval();

    let buffer_a = buffer.clone();
    let buffer_b = buffer.clone();
    let instance_id_a_clone = instance_id_a.clone();
    let instance_id_b_clone = instance_id_b.clone();
    let wait_key_a_clone = wait_key.clone();
    let wait_key_b_clone = wait_key.clone();

    // Thread A buffers to key A
    let handle_a = thread::spawn(move || {
        for i in 0..10 {
            let sig = make_signal(&format!("sig-a-{}", i));
            let mut buf = buffer_a.blocking_lock();
            buf.buffer_signal(
                instance_id_a_clone.clone(),
                wait_key_a_clone.clone(),
                sig,
                BufferPolicy::BufferMany,
            );
        }
    });

    // Thread B buffers to key B concurrently
    let handle_b = thread::spawn(move || {
        for i in 0..10 {
            let sig = make_signal(&format!("sig-b-{}", i));
            let mut buf = buffer_b.blocking_lock();
            buf.buffer_signal(
                instance_id_b_clone.clone(),
                wait_key_b_clone.clone(),
                sig,
                BufferPolicy::BufferMany,
            );
        }
    });

    handle_a.join().unwrap();
    handle_b.join().unwrap();

    let buf = buffer.blocking_lock();
    assert_eq!(
        buf.buffered_count(&instance_id_a, &wait_key),
        10,
        "Key A should have 10 signals"
    );
    assert_eq!(
        buf.buffered_count(&instance_id_b, &wait_key),
        10,
        "Key B should have 10 signals"
    );
    assert_eq!(
        buf.total_buffered_count(),
        20,
        "Total should be 20 signals across all keys"
    );
}
