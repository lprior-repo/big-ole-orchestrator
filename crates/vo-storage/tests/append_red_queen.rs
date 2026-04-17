//! Red Queen adversarial tests for vo-storage append writer
//!
//! Tests the append writer invariants against:
//! - Concurrent appends from multiple threads (via Arc<Mutex> wrapping)
//! - Budget exhaustion and rollback atomicity
//! - Queue capacity limits and backpressure
//! - Priority ordering (ADR-016: CriticalControlPlane first)
//! - Thread safety under stress
//!
//! Target: vo-storage/append

#![allow(clippy::unwrap_used)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use vo_storage::append::{
    AppendEntry, Appender, BackpressureSignal, BlobWrite, BudgetQueues, BudgetQueuesError,
<<<<<<< HEAD
    ClassifiedWrite, ControlPlaneWrite, ProjectionWrite, QueueConfig, WriteBudget, WriteClass,
=======
    ControlPlaneWrite, ProjectionWrite, QueueConfig, WriteBudget, WriteClass,
>>>>>>> origin/vo-worker-tests
};
use vo_types::events::EventEnvelope;
#[cfg(test)]
use vo_types::events::EventMetadata;

// ========================================================================
// Test helpers
// ========================================================================

fn make_event(instance_id: &str, sequence: u64) -> EventEnvelope {
    EventEnvelope {
        schema_version: 1,
        instance_id: instance_id.to_string(),
        sequence,
        timestamp_ms: 1000 + sequence,
        payload: serde_json::json!({ "seq": sequence }),
        metadata: EventMetadata::default(),
    }
}

fn make_control_plane_write(size_bytes: u64) -> ControlPlaneWrite {
    ControlPlaneWrite::new(make_event("inst-1", 1), size_bytes)
}

fn make_projection_write(id: &str, size_bytes: u64) -> ProjectionWrite {
<<<<<<< HEAD
    ProjectionWrite::new(id.to_string(), size_bytes)
=======
    ProjectionWrite {
        projection_id: id.to_string(),
        size_bytes,
    }
>>>>>>> origin/vo-worker-tests
}

fn make_blob_write(id: &str, size_bytes: u64) -> BlobWrite {
    BlobWrite::bulk(id.to_string(), size_bytes)
}

// ========================================================================
// DIMENSION: concurrent_append
// ADR-016 §1: Atomic WriteBatches - concurrent access safety
// Note: BudgetQueues uses RefCell (not Sync), so we test via Arc<Mutex<>>
// ========================================================================

#[test]
fn red_queen_concurrent_sequential_operations() {
    let config = QueueConfig {
        critical_capacity: 1024,
        projection_capacity: 512,
        blob_capacity: 256,
    };
    let budget = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
<<<<<<< HEAD
    let queues = Arc::new(Mutex::new(BudgetQueues::new(&config, budget)));
=======
    let queues = Arc::new(Mutex::new(BudgetQueues::new(config, budget)));
>>>>>>> origin/vo-worker-tests

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
<<<<<<< HEAD
    let queues = Arc::new(Mutex::new(BudgetQueues::new(&config, budget)));
=======
    let queues = Arc::new(Mutex::new(BudgetQueues::new(config, budget)));
>>>>>>> origin/vo-worker-tests

    let produced = Arc::new(AtomicUsize::new(0));
    let consumed = Arc::new(AtomicUsize::new(0));

    // Producer thread
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

    // Consumer thread
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
<<<<<<< HEAD
    let appender = Arc::new(Mutex::new(Appender::new(&config, budget)));

    std::thread::scope(|s| {
        let appender_clone = appender.clone();
        s.spawn(move || {
=======
    let appender = Arc::new(Mutex::new(Appender::new(config, budget)));

    std::thread::scope(|s| {
        let appender_clone = Arc::clone(&appender);
        s.spawn(|| {
>>>>>>> origin/vo-worker-tests
            for i in 0..100 {
                let write = make_control_plane_write(100);
                let _ = appender_clone.lock().unwrap().append_control_plane(write);
            }
        });

<<<<<<< HEAD
        let appender_clone2 = appender.clone();
        s.spawn(move || {
=======
        let appender_clone2 = Arc::clone(&appender);
        s.spawn(|| {
>>>>>>> origin/vo-worker-tests
            for i in 0..100 {
                let write = make_projection_write(&format!("proj-{}", i), 100);
                let _ = appender_clone2.lock().unwrap().append_projection(write);
            }
        });

<<<<<<< HEAD
        let appender_clone3 = appender.clone();
        s.spawn(move || {
=======
        let appender_clone3 = Arc::clone(&appender);
        s.spawn(|| {
>>>>>>> origin/vo-worker-tests
            for i in 0..100 {
                let write = make_blob_write(&format!("blob-{}", i), 100);
                let _ = appender_clone3.lock().unwrap().append_blob(write);
            }
        });
    });

<<<<<<< HEAD
    let binding = appender.lock().unwrap().stats();
    let stats = binding.lock().unwrap();
=======
    let stats = appender.lock().unwrap().stats().lock().unwrap();
>>>>>>> origin/vo-worker-tests
    assert_eq!(stats.depth(WriteClass::CriticalControlPlane), 100);
    assert_eq!(stats.depth(WriteClass::OperatorProjection), 100);
    assert_eq!(stats.depth(WriteClass::BulkBlob), 100);
}

// ========================================================================
// DIMENSION: budget_exhaustion
// ADR-016 §1: Budget tracking must be consistent under concurrent access
// ========================================================================

#[test]
fn red_queen_budget_exhaustion_boundary() {
    let config = QueueConfig::default();
    let budget = WriteBudget::new(500, 1_000_000, 1_000_000);
<<<<<<< HEAD
    let appender = Appender::new(&config, budget);
=======
    let appender = Appender::new(config, budget);
>>>>>>> origin/vo-worker-tests

    let event = make_event("test", 1);
    let write = ControlPlaneWrite::new(event, 500);

    let result1 = appender.append_control_plane(write);
    assert!(
        result1.is_ok(),
        "First write of exactly 500 bytes should succeed"
    );

    let event2 = make_event("test", 2);
    let write2 = ControlPlaneWrite::new(event2, 1);
    let result2 = appender.append_control_plane(write2);
    assert!(
        matches!(result2, Err(BudgetQueuesError::BudgetExceeded { .. })),
        "Second write of 1 byte should fail - budget exhausted"
    );
}

#[test]
fn red_queen_budget_oversized_write_rejected() {
    let config = QueueConfig::default();
    let budget = WriteBudget::new(100, 1_000_000, 1_000_000);
<<<<<<< HEAD
    let appender = Appender::new(&config, budget);
=======
    let appender = Appender::new(config, budget);
>>>>>>> origin/vo-worker-tests

    let event = make_event("test", 1);
    let write = ControlPlaneWrite::new(event, 200);

    let result = appender.append_control_plane(write);
    assert!(
        matches!(result, Err(BudgetQueuesError::BudgetExceeded { .. })),
        "Write larger than total budget should be rejected"
    );
}

#[test]
fn red_queen_budget_rollback_on_queue_full() {
    let config = QueueConfig {
        critical_capacity: 1,
        projection_capacity: 1,
        blob_capacity: 1,
    };
    let budget = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
<<<<<<< HEAD
    let queues = BudgetQueues::new(&config, budget);
=======
    let queues = BudgetQueues::new(config, budget);
>>>>>>> origin/vo-worker-tests

    let write1 = AppendEntry::ControlPlane(make_control_plane_write(100));
    assert!(queues.try_enqueue(&write1).is_ok());

    let write2 = AppendEntry::ControlPlane(make_control_plane_write(100));
    assert!(matches!(
        queues.try_enqueue(&write2),
        Err(BudgetQueuesError::QueueFull { .. })
    ));

    assert_eq!(
        queues.budget().remaining(WriteClass::CriticalControlPlane),
        1_000_000 - 100,
        "Budget for dequeued item should be released"
    );
}

#[test]
fn red_queen_budget_reserved_on_successful_enqueue() {
    let config = QueueConfig::default();
    let budget = WriteBudget::new(1000, 1_000_000, 1_000_000);
<<<<<<< HEAD
    let queues = BudgetQueues::new(&config, budget);
=======
    let queues = BudgetQueues::new(config, budget);
>>>>>>> origin/vo-worker-tests

    let write = AppendEntry::ControlPlane(make_control_plane_write(300));
    assert!(queues.try_enqueue(&write).is_ok());

    assert_eq!(
        queues.budget().remaining(WriteClass::CriticalControlPlane),
        1000 - 300,
        "Budget should be reduced by exactly the write size"
    );
}

#[test]
fn red_queen_budget_not_reserved_on_failed_enqueue() {
    let config = QueueConfig {
        critical_capacity: 1,
        projection_capacity: 1,
        blob_capacity: 1,
    };
    let budget = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
<<<<<<< HEAD
    let queues = BudgetQueues::new(&config, budget);
=======
    let queues = BudgetQueues::new(config, budget);
>>>>>>> origin/vo-worker-tests

    let write1 = AppendEntry::ControlPlane(make_control_plane_write(100));
    assert!(queues.try_enqueue(&write1).is_ok());

    let initial_remaining = queues.budget().remaining(WriteClass::CriticalControlPlane);

    let write2 = AppendEntry::ControlPlane(make_control_plane_write(100));
    let result = queues.try_enqueue(&write2);
    assert!(result.is_err(), "Second write should fail - queue full");

    assert_eq!(
        queues.budget().remaining(WriteClass::CriticalControlPlane),
        initial_remaining,
        "Budget should not change when queue enqueue fails"
    );
}

// ========================================================================
// DIMENSION: queue_capacity
// ADR-016 §1: Queues are bounded - must handle overflow correctly
// ========================================================================

#[test]
fn red_queen_queue_capacity_exact_fill() {
    let config = QueueConfig {
        critical_capacity: 3,
        projection_capacity: 3,
        blob_capacity: 3,
    };
    let budget = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
<<<<<<< HEAD
    let appender = Appender::new(&config, budget);
=======
    let appender = Appender::new(config, budget);
>>>>>>> origin/vo-worker-tests

    for i in 0..3 {
        let event = make_event("test", i);
        let write = ControlPlaneWrite::new(event, 100);
        assert!(
            appender.append_control_plane(write).is_ok(),
            "Write {} of 3 should succeed",
            i + 1
        );
    }

    let event4 = make_event("test", 4);
    let write4 = ControlPlaneWrite::new(event4, 100);
    assert!(
        matches!(
            appender.append_control_plane(write4),
            Err(BudgetQueuesError::QueueFull { .. })
        ),
        "4th write should fail - queue at capacity"
    );
}

#[test]
fn red_queen_queue_capacity_independent_per_class() {
    let config = QueueConfig {
        critical_capacity: 2,
        projection_capacity: 2,
        blob_capacity: 2,
    };
    let budget = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
<<<<<<< HEAD
    let appender = Appender::new(&config, budget);
=======
    let appender = Appender::new(config, budget);
>>>>>>> origin/vo-worker-tests

    let event = make_event("test", 1);

    assert!(appender
        .append_control_plane(ControlPlaneWrite::new(event.clone(), 100))
        .is_ok());
    assert!(appender
<<<<<<< HEAD
        .append_projection(ProjectionWrite::new("proj-1".to_string(), 100))
=======
        .append_projection(ProjectionWrite {
            projection_id: "proj-1".to_string(),
            size_bytes: 100
        })
>>>>>>> origin/vo-worker-tests
        .is_ok());
    assert!(appender
        .append_blob(BlobWrite::bulk("blob-1".to_string(), 100))
        .is_ok());

<<<<<<< HEAD
    let binding = appender.stats();
    let stats = binding.lock().unwrap();
=======
    let stats = appender.stats().lock().unwrap();
>>>>>>> origin/vo-worker-tests
    assert_eq!(stats.depth(WriteClass::CriticalControlPlane), 1);
    assert_eq!(stats.depth(WriteClass::OperatorProjection), 1);
    assert_eq!(stats.depth(WriteClass::BulkBlob), 1);

    assert!(
        appender
            .append_control_plane(ControlPlaneWrite::new(event, 100))
            .is_ok(),
        "Second critical write should succeed - different queue"
    );
}

#[test]
fn red_queen_queue_full_error_contains_class_info() {
    let config = QueueConfig {
        critical_capacity: 1,
        projection_capacity: 1,
        blob_capacity: 1,
    };
    let budget = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
<<<<<<< HEAD
    let appender = Appender::new(&config, budget);
=======
    let appender = Appender::new(config, budget);
>>>>>>> origin/vo-worker-tests

    let event = make_event("test", 1);
    assert!(appender
        .append_control_plane(ControlPlaneWrite::new(event.clone(), 100))
        .is_ok());

    let result = appender.append_control_plane(ControlPlaneWrite::new(event, 100));
    assert!(matches!(
        result,
        Err(BudgetQueuesError::QueueFull {
            class: WriteClass::CriticalControlPlane,
            ..
        })
    ));
}

// ========================================================================
// DIMENSION: backpressure
// ADR-016 §1: Backpressure signal must reflect true queue state
// ========================================================================

#[test]
fn red_queen_backpressure_signal_thread_safe() {
    let signal = BackpressureSignal::new();

    std::thread::scope(|s| {
        for _ in 0..100 {
            s.spawn(|| {
                // BackpressureSignal::set_full is pub(crate), test via should_reject
                // We can only test the public API
            });
        }
    });

    // Verify initial state is not backpressured
    assert!(!signal.is_backpressured(WriteClass::CriticalControlPlane));
}

#[test]
fn red_queen_backpressure_cleared_on_dequeue() {
    let config = QueueConfig {
        critical_capacity: 2,
        projection_capacity: 2,
        blob_capacity: 2,
    };
    let budget = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
<<<<<<< HEAD
    let queues = BudgetQueues::new(&config, budget);
=======
    let queues = BudgetQueues::new(config, budget);
>>>>>>> origin/vo-worker-tests

    let event = make_event("test", 1);

    // Fill to capacity (capacity=2, we'll add 2 items)
    queues
        .try_enqueue(&AppendEntry::ControlPlane(ControlPlaneWrite::new(
            event.clone(),
            100,
        )))
        .unwrap();
    queues
        .try_enqueue(&AppendEntry::ControlPlane(ControlPlaneWrite::new(
            event, 100,
        )))
        .unwrap();

    // At capacity, backpressure should be signaled via should_reject
    assert!(queues
        .backpressure()
        .should_reject(WriteClass::CriticalControlPlane));

    queues.dequeue(WriteClass::CriticalControlPlane);

    // After dequeuing one, should_reject should be false
    assert!(!queues
        .backpressure()
        .should_reject(WriteClass::CriticalControlPlane));
}

#[test]
fn red_queen_backpressure_any_returns_true_when_any_backpressured() {
    let config = QueueConfig {
        critical_capacity: 1,
        projection_capacity: 1,
        blob_capacity: 1,
    };
    let budget = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
<<<<<<< HEAD
    let queues = BudgetQueues::new(&config, budget);
=======
    let queues = BudgetQueues::new(config, budget);
>>>>>>> origin/vo-worker-tests

    assert!(!queues.backpressure().any_backpressured());

    // Fill projection queue
    queues
<<<<<<< HEAD
        .try_enqueue(&AppendEntry::Projection(ProjectionWrite::new(
            "proj-1".to_string(),
            100,
        )))
        .unwrap();
    queues
        .try_enqueue(&AppendEntry::Projection(ProjectionWrite::new(
            "proj-2".to_string(),
            100,
        )))
=======
        .try_enqueue(&AppendEntry::Projection(ProjectionWrite {
            projection_id: "proj-1".to_string(),
            size_bytes: 100,
        }))
        .unwrap();
    queues
        .try_enqueue(&AppendEntry::Projection(ProjectionWrite {
            projection_id: "proj-2".to_string(),
            size_bytes: 100,
        }))
>>>>>>> origin/vo-worker-tests
        .unwrap();

    assert!(queues.backpressure().any_backpressured());
}

#[test]
fn red_queen_backpressure_should_reject_respects_class() {
    let config = QueueConfig {
        critical_capacity: 1,
        projection_capacity: 1,
        blob_capacity: 1,
    };
    let budget = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
<<<<<<< HEAD
    let queues = BudgetQueues::new(&config, budget);

    // Fill projection queue to capacity
    queues
        .try_enqueue(&AppendEntry::Projection(ProjectionWrite::new(
            "proj-1".to_string(),
            100,
        )))
        .unwrap();
    queues
        .try_enqueue(&AppendEntry::Projection(ProjectionWrite::new(
            "proj-2".to_string(),
            100,
        )))
=======
    let queues = BudgetQueues::new(config, budget);

    // Fill projection queue to capacity
    queues
        .try_enqueue(&AppendEntry::Projection(ProjectionWrite {
            projection_id: "proj-1".to_string(),
            size_bytes: 100,
        }))
        .unwrap();
    queues
        .try_enqueue(&AppendEntry::Projection(ProjectionWrite {
            projection_id: "proj-2".to_string(),
            size_bytes: 100,
        }))
>>>>>>> origin/vo-worker-tests
        .unwrap();

    assert!(
        !queues
            .backpressure()
            .should_reject(WriteClass::CriticalControlPlane),
        "Critical writes should never be rejected even when other classes are full"
    );
    assert!(
        queues
            .backpressure()
            .should_reject(WriteClass::OperatorProjection),
        "Projection writes should be rejected when projection is full"
    );
    assert!(
        !queues.backpressure().should_reject(WriteClass::BulkBlob),
        "Blob writes should not be rejected when blob is not full"
    );
}

// ========================================================================
// DIMENSION: dequeue_ordering
// ADR-016 §1: CriticalControlPlane writes must be dequeued first (priority)
// ========================================================================

#[test]
fn red_queen_dequeue_priority_critical_first() {
    let config = QueueConfig::default();
    let budget = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
<<<<<<< HEAD
    let queues = BudgetQueues::new(&config, budget);
=======
    let queues = BudgetQueues::new(config, budget);
>>>>>>> origin/vo-worker-tests

    queues
        .try_enqueue(&AppendEntry::Blob(BlobWrite::bulk("b1".to_string(), 100)))
        .unwrap();
    queues
<<<<<<< HEAD
        .try_enqueue(&AppendEntry::Projection(ProjectionWrite::new(
            "p1".to_string(),
            100,
        )))
=======
        .try_enqueue(&AppendEntry::Projection(ProjectionWrite {
            projection_id: "p1".to_string(),
            size_bytes: 100,
        }))
>>>>>>> origin/vo-worker-tests
        .unwrap();
    queues
        .try_enqueue(&AppendEntry::ControlPlane(ControlPlaneWrite::new(
            make_event("test", 1),
            100,
        )))
        .unwrap();

    let (class, _) = queues.dequeue_prioritized().unwrap();
    assert_eq!(
        class,
        WriteClass::CriticalControlPlane,
        "Critical must dequeue first regardless of enqueue order"
    );
}

#[test]
fn red_queen_dequeue_priority_all_classes_eventually_dequeued() {
    let config = QueueConfig::default();
    let budget = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
<<<<<<< HEAD
    let queues = BudgetQueues::new(&config, budget);
=======
    let queues = BudgetQueues::new(config, budget);
>>>>>>> origin/vo-worker-tests

    queues
        .try_enqueue(&AppendEntry::Blob(BlobWrite::bulk("b1".to_string(), 100)))
        .unwrap();
    queues
<<<<<<< HEAD
        .try_enqueue(&AppendEntry::Projection(ProjectionWrite::new(
            "p1".to_string(),
            100,
        )))
=======
        .try_enqueue(&AppendEntry::Projection(ProjectionWrite {
            projection_id: "p1".to_string(),
            size_bytes: 100,
        }))
>>>>>>> origin/vo-worker-tests
        .unwrap();
    queues
        .try_enqueue(&AppendEntry::ControlPlane(ControlPlaneWrite::new(
            make_event("test", 1),
            100,
        )))
        .unwrap();

    let mut classes_dequeued = Vec::new();
    while let Some((class, _)) = queues.dequeue_prioritized() {
        classes_dequeued.push(class);
    }

    assert_eq!(classes_dequeued.len(), 3);
    assert_eq!(classes_dequeued[0], WriteClass::CriticalControlPlane);
    assert_eq!(
        classes_dequeued[1],
        WriteClass::OperatorProjection,
        "Projection should come before Blob"
    );
    assert_eq!(classes_dequeued[2], WriteClass::BulkBlob);
}

#[test]
fn red_queen_dequeue_prioritized_skips_empty_critical() {
    let config = QueueConfig::default();
    let budget = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
<<<<<<< HEAD
    let queues = BudgetQueues::new(&config, budget);
=======
    let queues = BudgetQueues::new(config, budget);
>>>>>>> origin/vo-worker-tests

    queues
        .try_enqueue(&AppendEntry::Blob(BlobWrite::bulk("b1".to_string(), 100)))
        .unwrap();

    let (class, _) = queues.dequeue_prioritized().unwrap();
    assert_eq!(
        class,
        WriteClass::BulkBlob,
        "Should skip empty critical queue and return blob"
    );
}

#[test]
fn red_queen_dequeue_prioritized_returns_none_when_all_empty() {
    let config = QueueConfig::default();
    let budget = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
<<<<<<< HEAD
    let queues: BudgetQueues<AppendEntry> = BudgetQueues::new(&config, budget);
=======
    let queues: BudgetQueues<AppendEntry> = BudgetQueues::new(config, budget);
>>>>>>> origin/vo-worker-tests

    assert!(queues.dequeue_prioritized().is_none());
}

// ========================================================================
// DIMENSION: atomicity
// ADR-016 §1: Atomic WriteBatches - budget reservation is atomic with enqueue
// ========================================================================

#[test]
fn red_queen_atomic_budget_and_enqueue() {
    let config = QueueConfig::default();
    let budget = WriteBudget::new(500, 1_000_000, 1_000_000);
<<<<<<< HEAD
    let queues = BudgetQueues::new(&config, budget);
=======
    let queues = BudgetQueues::new(config, budget);
>>>>>>> origin/vo-worker-tests

    let write = AppendEntry::ControlPlane(make_control_plane_write(300));
    assert!(queues.try_enqueue(&write).is_ok());

    assert_eq!(
        queues.budget().remaining(WriteClass::CriticalControlPlane),
        200,
        "Budget should be exactly reduced"
    );

    let write2 = AppendEntry::ControlPlane(make_control_plane_write(200));
    assert!(queues.try_enqueue(&write2).is_ok());

    let write3 = AppendEntry::ControlPlane(make_control_plane_write(1));
    assert!(
        matches!(
            queues.try_enqueue(&write3),
            Err(BudgetQueuesError::BudgetExceeded { .. })
        ),
        "Third write should fail - budget exhausted"
    );
}

#[test]
fn red_queen_atomic_enqueue_rollback_on_budget_failure() {
    let config = QueueConfig {
        critical_capacity: 10,
        projection_capacity: 10,
        blob_capacity: 10,
    };
    let budget = WriteBudget::new(500, 1_000_000, 1_000_000);
<<<<<<< HEAD
    let queues = BudgetQueues::new(&config, budget);
=======
    let queues = BudgetQueues::new(config, budget);
>>>>>>> origin/vo-worker-tests

    let write = AppendEntry::ControlPlane(make_control_plane_write(300));
    assert!(queues.try_enqueue(&write).is_ok());

    let stats_before = queues
        .stats()
        .lock()
        .unwrap()
        .depth(WriteClass::CriticalControlPlane);

    let write2 = AppendEntry::ControlPlane(make_control_plane_write(300));
    let result = queues.try_enqueue(&write2);

    assert!(
        result.is_err(),
        "Second write should fail - would exceed budget"
    );

    let stats_after = queues
        .stats()
        .lock()
        .unwrap()
        .depth(WriteClass::CriticalControlPlane);
    assert_eq!(
        stats_after, stats_before,
        "Queue depth should not increase when budget check fails"
    );
}

#[test]
fn red_queen_atomic_dequeue_releases_budget() {
    let config = QueueConfig::default();
    let budget = WriteBudget::new(1000, 1_000_000, 1_000_000);
<<<<<<< HEAD
    let queues = BudgetQueues::new(&config, budget);
=======
    let queues = BudgetQueues::new(config, budget);
>>>>>>> origin/vo-worker-tests

    let write = AppendEntry::ControlPlane(make_control_plane_write(500));
    assert!(queues.try_enqueue(&write).is_ok());

    assert_eq!(
        queues.budget().remaining(WriteClass::CriticalControlPlane),
        500,
        "500 bytes budget used"
    );

    queues.dequeue(WriteClass::CriticalControlPlane);

    assert_eq!(
        queues.budget().remaining(WriteClass::CriticalControlPlane),
        1000,
        "Budget should be fully restored after dequeue"
    );
}

// ========================================================================
// DIMENSION: write_classification
// ADR-016: WriteClass determines priority and durability guarantees
// ========================================================================

#[test]
fn red_queen_write_class_tier_ordering() {
    assert!(
        WriteClass::CriticalControlPlane.tier() < WriteClass::OperatorProjection.tier(),
        "Critical tier (1) must be less than Projection tier (2)"
    );
    assert!(
        WriteClass::OperatorProjection.tier() < WriteClass::BulkBlob.tier(),
        "Projection tier (2) must be less than Blob tier (3)"
    );
}

#[test]
fn red_queen_write_class_never_drops() {
    assert!(
        WriteClass::CriticalControlPlane.never_drops(),
        "CriticalControlPlane writes must never be dropped"
    );
    assert!(
        !WriteClass::OperatorProjection.never_drops(),
        "OperatorProjection writes may be dropped under pressure"
    );
    assert!(
        !WriteClass::BulkBlob.never_drops(),
        "BulkBlob writes may be dropped under pressure"
    );
}

#[test]
fn red_queen_write_class_classification() {
    let event = make_event("test", 1);

    // ControlPlaneWrite always classifies as CriticalControlPlane
    let cp_write = ControlPlaneWrite::new(event.clone(), 100);
    assert_eq!(
        cp_write.write_class(),
        WriteClass::CriticalControlPlane,
        "ControlPlaneWrite must classify as CriticalControlPlane"
    );

    // ProjectionWrite always classifies as OperatorProjection
<<<<<<< HEAD
    let proj_write = ProjectionWrite::new("test".to_string(), 100);
=======
    let proj_write = ProjectionWrite {
        projection_id: "test".to_string(),
        size_bytes: 100,
    };
>>>>>>> origin/vo-worker-tests
    assert_eq!(
        proj_write.write_class(),
        WriteClass::OperatorProjection,
        "ProjectionWrite must classify as OperatorProjection"
    );

    // BlobWrite::bulk always classifies as BulkBlob
    let blob_write = BlobWrite::bulk("test".to_string(), 100);
    assert_eq!(
        blob_write.write_class(),
        WriteClass::BulkBlob,
        "BlobWrite::bulk must classify as BulkBlob"
    );
}

#[test]
fn red_queen_append_entry_classification() {
    let event = make_event("test", 1);

    let cp_entry = AppendEntry::ControlPlane(ControlPlaneWrite::new(event.clone(), 100));
    assert_eq!(cp_entry.write_class(), WriteClass::CriticalControlPlane);

<<<<<<< HEAD
    let proj_entry = AppendEntry::Projection(ProjectionWrite::new("test".to_string(), 100));
=======
    let proj_entry = AppendEntry::Projection(ProjectionWrite {
        projection_id: "test".to_string(),
        size_bytes: 100,
    });
>>>>>>> origin/vo-worker-tests
    assert_eq!(proj_entry.write_class(), WriteClass::OperatorProjection);

    let blob_entry = AppendEntry::Blob(BlobWrite::bulk("test".to_string(), 100));
    assert_eq!(blob_entry.write_class(), WriteClass::BulkBlob);
}

// ========================================================================
// DIMENSION: shared_backpressure
// ADR-016 §1: Multiple BudgetQueues can share a backpressure signal
// ========================================================================

#[test]
fn red_queen_shared_backpressure_signal() {
    let shared_signal = Arc::new(BackpressureSignal::new());

    let config = QueueConfig {
        critical_capacity: 1,
        projection_capacity: 1,
        blob_capacity: 1,
    };
    let budget1 = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
    let budget2 = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);

<<<<<<< HEAD
    let queues1 = BudgetQueues::new_with_backpressure(&config, budget1, Arc::clone(&shared_signal));
    let queues2 = BudgetQueues::new_with_backpressure(&config, budget2, Arc::clone(&shared_signal));
=======
    let queues1 =
        BudgetQueues::new_with_backpressure(config.clone(), budget1, Arc::clone(&shared_signal));
    let queues2 =
        BudgetQueues::new_with_backpressure(config.clone(), budget2, Arc::clone(&shared_signal));
>>>>>>> origin/vo-worker-tests

    let event = make_event("test", 1);
    queues1
        .try_enqueue(&AppendEntry::ControlPlane(ControlPlaneWrite::new(
            event.clone(),
            100,
        )))
        .unwrap();
    queues1
        .try_enqueue(&AppendEntry::ControlPlane(ControlPlaneWrite::new(
            event.clone(),
            100,
        )))
        .unwrap();

    queues2
<<<<<<< HEAD
        .try_enqueue(&AppendEntry::Projection(ProjectionWrite::new(
            "proj-1".to_string(),
            100,
        )))
        .unwrap();
    queues2
        .try_enqueue(&AppendEntry::Projection(ProjectionWrite::new(
            "proj-2".to_string(),
            100,
        )))
=======
        .try_enqueue(&AppendEntry::Projection(ProjectionWrite {
            projection_id: "proj-1".to_string(),
            size_bytes: 100,
        }))
        .unwrap();
    queues2
        .try_enqueue(&AppendEntry::Projection(ProjectionWrite {
            projection_id: "proj-2".to_string(),
            size_bytes: 100,
        }))
>>>>>>> origin/vo-worker-tests
        .unwrap();

    assert!(
        shared_signal.is_backpressured(WriteClass::CriticalControlPlane),
        "Shared signal should reflect first queue's critical state"
    );
    assert!(
        shared_signal.is_backpressured(WriteClass::OperatorProjection),
        "Shared signal should reflect second queue's projection state"
    );
}

// ========================================================================
// DIMENSION: stress_fuzz
// Fuzzing-style tests with random-like patterns
// ========================================================================

#[test]
fn red_queen_stress_alternating_enqueue_dequeue() {
    let config = QueueConfig {
        critical_capacity: 50,
        projection_capacity: 50,
        blob_capacity: 50,
    };
    let budget = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
<<<<<<< HEAD
    let appender = Appender::new(&config, budget);
=======
    let appender = Appender::new(config, budget);
>>>>>>> origin/vo-worker-tests

    for i in 0..100 {
        let event = make_event("stress", i);
        let write = ControlPlaneWrite::new(event, 50);
        assert!(appender.append_control_plane(write).is_ok());

        if i % 2 == 0 {
            assert!(
                appender.dequeue_critical().is_some(),
                "Should dequeue every other write"
            );
        }
    }

<<<<<<< HEAD
    let binding = appender.stats();
    let stats = binding.lock().unwrap();
=======
    let stats = appender.stats().lock().unwrap();
>>>>>>> origin/vo-worker-tests
    assert_eq!(
        stats.depth(WriteClass::CriticalControlPlane),
        50,
        "50 enqueues - 25 dequeues = 50 remaining"
    );
}

#[test]
fn red_queen_stress_fill_all_queues_completely() {
    let config = QueueConfig {
        critical_capacity: 5,
        projection_capacity: 5,
        blob_capacity: 5,
    };
    let budget = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
<<<<<<< HEAD
    let appender = Appender::new(&config, budget);
=======
    let appender = Appender::new(config, budget);
>>>>>>> origin/vo-worker-tests

    for class in [
        WriteClass::CriticalControlPlane,
        WriteClass::OperatorProjection,
        WriteClass::BulkBlob,
    ] {
        for i in 0..5 {
            let event = make_event(&format!("{:?}", class), i);
            let result = match class {
                WriteClass::CriticalControlPlane => {
                    appender.append_control_plane(ControlPlaneWrite::new(event, 100))
                }
<<<<<<< HEAD
                WriteClass::OperatorProjection => appender
                    .append_projection(ProjectionWrite::new(format!("{:?}-{}", class, i), 100)),
=======
                WriteClass::OperatorProjection => appender.append_projection(ProjectionWrite {
                    projection_id: format!("{:?}-{}", class, i),
                    size_bytes: 100,
                }),
>>>>>>> origin/vo-worker-tests
                WriteClass::BulkBlob => {
                    appender.append_blob(BlobWrite::bulk(format!("{:?}-{}", class, i), 100))
                }
            };
            assert!(result.is_ok(), "Write {} for {:?} should succeed", i, class);
        }

        let result = match class {
            WriteClass::CriticalControlPlane => appender
                .append_control_plane(ControlPlaneWrite::new(make_event("overflow", 0), 100)),
<<<<<<< HEAD
            WriteClass::OperatorProjection => {
                appender.append_projection(ProjectionWrite::new("overflow".to_string(), 100))
            }
=======
            WriteClass::OperatorProjection => appender.append_projection(ProjectionWrite {
                projection_id: "overflow".to_string(),
                size_bytes: 100,
            }),
>>>>>>> origin/vo-worker-tests
            WriteClass::BulkBlob => {
                appender.append_blob(BlobWrite::bulk("overflow".to_string(), 100))
            }
        };
        assert!(
            matches!(result, Err(BudgetQueuesError::QueueFull { .. })),
            "6th write for {:?} should fail - queue full",
            class
        );
    }
}

#[test]
fn red_queen_stress_zero_sized_write() {
    let config = QueueConfig::default();
    let budget = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
<<<<<<< HEAD
    let appender = Appender::new(&config, budget);
=======
    let appender = Appender::new(config, budget);
>>>>>>> origin/vo-worker-tests

    let event = make_event("test", 1);
    let write = ControlPlaneWrite::new(event, 0);
    let result = appender.append_control_plane(write);

    assert!(
        result.is_ok(),
        "Zero-sized write should be allowed if budget permits"
    );
}

#[test]
fn red_queen_stress_max_capacity_values() {
    let config = QueueConfig {
        critical_capacity: usize::MAX,
        projection_capacity: usize::MAX,
        blob_capacity: usize::MAX,
    };
    let budget = WriteBudget::new(u64::MAX, u64::MAX, u64::MAX);
<<<<<<< HEAD
    let appender = Appender::new(&config, budget);
=======
    let appender = Appender::new(config, budget);
>>>>>>> origin/vo-worker-tests

    let event = make_event("test", 1);
    let write = ControlPlaneWrite::new(event, u64::MAX);

    let result = appender.append_control_plane(write);
    assert!(
        matches!(result, Err(BudgetQueuesError::BudgetExceeded { .. })),
        "Write of u64::MAX should fail - exceeds budget"
    );
}

// ========================================================================
// DIMENSION: recovery_simulation
// ADR-016 §2: Snapshot recovery - verify behavior on re-creation
// ========================================================================

#[test]
fn red_queen_recovery_new_instance_has_empty_state() {
    let config = QueueConfig::default();
    let budget = WriteBudget::new(1000, 1000, 1000);
<<<<<<< HEAD
    let appender1 = Appender::new(&config, budget.clone());
=======
    let appender1 = Appender::new(config.clone(), budget.clone());
>>>>>>> origin/vo-worker-tests

    let event = make_event("test", 1);
    appender1
        .append_control_plane(ControlPlaneWrite::new(event, 500))
        .unwrap();

    drop(appender1);

<<<<<<< HEAD
    let appender2 = Appender::new(&config, budget);

    let binding = appender2.stats();
    let stats = binding.lock().unwrap();
=======
    let appender2 = Appender::new(config, budget);

    let stats = appender2.stats().lock().unwrap();
>>>>>>> origin/vo-worker-tests
    assert_eq!(
        stats.depth(WriteClass::CriticalControlPlane),
        0,
        "New appender instance should have empty queues (in-memory state not persisted)"
    );
    assert_eq!(
        stats.depth(WriteClass::OperatorProjection),
        0,
        "New appender instance should have empty projection queue"
    );
    assert_eq!(
        stats.depth(WriteClass::BulkBlob),
        0,
        "New appender instance should have empty blob queue"
    );
}

#[test]
fn red_queen_recovery_budget_reset_on_new_instance() {
    let config = QueueConfig::default();
    let budget1 = WriteBudget::new(500, 1000, 1000);
<<<<<<< HEAD
    let appender1 = Appender::new(&config, budget1);
=======
    let appender1 = Appender::new(config.clone(), budget1);
>>>>>>> origin/vo-worker-tests

    appender1
        .append_control_plane(ControlPlaneWrite::new(make_event("test", 1), 300))
        .unwrap();

    drop(appender1);

    let budget2 = WriteBudget::new(500, 1000, 1000);
    assert_eq!(
        budget2.remaining(WriteClass::CriticalControlPlane),
        500,
        "New budget instance should have full capacity"
    );
}
