//! Property test: SPSC queue concurrent correctness under stress.
//!
//! bead_id: ve-e2nsq

#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use vo_ipc::spsc::{SpscError, SpscQueue};

#[test]
fn spsc_concurrent_fifo_with_wraparound() {
    let queue = Arc::new(SpscQueue::<usize>::new(16));
    let item_count = 10_000;

    let q_producer = queue.clone();
    let producer = std::thread::spawn(move || {
        for i in 0..item_count {
            while q_producer.send(i).is_err() {
                std::hint::spin_loop();
            }
        }
    });

    let q_consumer = queue.clone();
    let consumer = std::thread::spawn(move || {
        let mut received = Vec::with_capacity(item_count);
        for _ in 0..item_count {
            loop {
                match q_consumer.recv() {
                    Ok(item) => {
                        received.push(item);
                        break;
                    }
                    Err(SpscError::Empty) => std::hint::spin_loop(),
                    Err(e) => panic!("Unexpected SPSC error: {:?}", e),
                }
            }
        }
        received
    });

    producer.join().expect("producer panicked");
    let received = consumer.join().expect("consumer panicked");

    assert_eq!(received.len(), item_count);
    for (i, &item) in received.iter().enumerate() {
        assert_eq!(item, i, "FIFO violation at index {}", i);
    }
}

#[test]
fn spsc_capacity_always_power_of_two() {
    for cap in [1, 2, 3, 4, 5, 7, 8, 15, 16, 31, 32, 100, 1024] {
        let q = SpscQueue::<()>::new(cap);
        assert!(
            q.capacity().is_power_of_two(),
            "Capacity {} should be power of two, got {}",
            cap,
            q.capacity()
        );
        assert!(
            q.capacity() >= cap,
            "Capacity {} should be >= requested {}",
            q.capacity(),
            cap
        );
    }
}
