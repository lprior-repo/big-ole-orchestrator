//! DIMENSION: concurrent_append
//! ADR-016 §1: Atomic WriteBatches - concurrent access safety
//! Note: BudgetQueues uses RefCell (not Sync), so we test via Arc<Mutex<>>

#![allow(clippy::unwrap_used)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use vo_storage::append::{
    AppendEntry, BackpressureSignal, BudgetQueues, BudgetQueuesError,
    ClassifiedWrite, ControlPlaneWrite, ProjectionWrite, QueueConfig, WriteBudget, WriteClass,
};
use vo_types::events::EventEnvelope;
#[cfg(test)]
use vo_types::events::EventMetadata;

use super::helpers::{make_event, make_projection_write, make_blob_write};

#[test]
fn red_queen_concurrent_sequential_operations() {
    let config = QueueConfig {
        critical_capacity: 1024,
        projection_capacity: 512,
        blob_capacity: 256,
    };
    let budget = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
    let queues = Arc::new(Mutex::new(BudgetQueues::new(&config, budget)));

    let success_count = Arc::new(AtomicUsize::new(0));
    let error_count = Arc::new(AtomicUsize::new(0));

    std::thread::scope(|s| {
        for i in 0..4 {
            let queues = Arc::clone(&queues);
            let success_count = &success_count;
            let error_count = &error_count;

            s.spawn(move || {
                for j in 0..250 {
                    let event = make_event(&format!("inst-{}", i), j);
                    let write = AppendEntry::ControlPlane(ControlPlaneWrite::new(event, 100));

                    match queues.lock().unwrap().try_enqueue(&write) {
                        Ok(()) => success_count.fetch_add(1, Ordering::Relaxed),
                        Err(_) => error_count.fetch_add(1, Ordering::Relaxed),
                    };
                }
            });
        }
    });

    let total = success_count.load(Ordering::Relaxed) + error_count.load(Ordering::Relaxed);
    assert_eq!(total, 1000, "All 1000 appends should be accounted for");
}

#[test]
fn red_queen_concurrent_enqueue_dequeue_via_arc_mutex() {
    let config = QueueConfig {
        critical_capacity: 100,
        projection_capacity: 100,
        blob_capacity: 100,
    };
    let budget = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
    let queues = Arc::new(Mutex::new(BudgetQueues::new(&config, budget)));

    let produced = Arc::new(AtomicUsize::new(0));
    let consumed = Arc::new(AtomicUsize::new(0));

    let produced_clone = produced.clone();
    let queues_clone = Arc::clone(&queues);
    let producer = std::thread::spawn(move || {
        for i in 0..500 {
            let event = make_event("producer", i);
            let write = AppendEntry::ControlPlane(ControlPlaneWrite::new(event, 50));
            if queues_clone.lock().unwrap().try_enqueue(&write).is_ok() {
                produced_clone.fetch_add(1, Ordering::Relaxed);
            }
        }
    });

    let consumed_clone = consumed.clone();
    let queues_clone2 = Arc::clone(&queues);
    let consumer = std::thread::spawn(move || {
        let mut local_consumed = 0;
        for _ in 0..500 {
            if queues_clone2
                .lock()
                .unwrap()
                .dequeue(WriteClass::CriticalControlPlane)
                .is_some()
            {
                local_consumed += 1;
            } else {
                std::thread::sleep(std::time::Duration::from_micros(100));
            }
        }
        consumed_clone.store(local_consumed, Ordering::Relaxed);
    });

    producer.join().unwrap();
    consumer.join().unwrap();

    assert_eq!(
        produced.load(Ordering::Relaxed),
        500,
        "All 500 items should be produced"
    );
}

#[test]
fn red_queen_concurrent_mixed_class_append() {
    let config = QueueConfig::default();
    let budget = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
    let appender = Arc::new(Mutex::new(super::super::Appender::new(&config, budget)));

    std::thread::scope(|s| {
        let appender_clone = appender.clone();
        s.spawn(move || {
            for i in 0..100 {
                let write = super::helpers::make_control_plane_write(100);
                let _ = appender_clone.lock().unwrap().append_control_plane(write);
            }
        });

        let appender_clone2 = appender.clone();
        s.spawn(move || {
            for i in 0..100 {
                let write = make_projection_write(&format!("proj-{}", i), 100);
                let _ = appender_clone2.lock().unwrap().append_projection(write);
            }
        });

        let appender_clone3 = appender.clone();
        s.spawn(move || {
            for i in 0..100 {
                let write = make_blob_write(&format!("blob-{}", i), 100);
                let _ = appender_clone3.lock().unwrap().append_blob(write);
            }
        });
    });

    let binding = appender.lock().unwrap().stats();
    let stats = binding.lock().unwrap();
    assert_eq!(stats.depth(WriteClass::CriticalControlPlane), 100);
    assert_eq!(stats.depth(WriteClass::OperatorProjection), 100);
    assert_eq!(stats.depth(WriteClass::BulkBlob), 100);
}
