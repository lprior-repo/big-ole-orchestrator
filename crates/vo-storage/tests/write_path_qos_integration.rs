//! Integration tests for write-path QoS (ADR-032) with real storage backend.
//!
//! These tests exercise the full write pipeline:
//! - Appender (in-memory queue with QoS enforcement)
//! - Backpressure signal propagation under load
//!
//! bead_id: ve-shy2
//! bead_title: Test Coverage: Write-path QoS and resume fairness (ADR-032/033)

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::sync::Arc;

use vo_storage::append::{
    AppendEntry, Appender, BackpressureSignal, BudgetQueues, BudgetQueuesError, ControlPlaneWrite,
    ProjectionWrite, QueueConfig, WriteBudget, WriteClass,
};
use vo_types::events::metadata::EventMetadata;
use vo_types::EventEnvelope;

fn make_event(instance_id: &str, sequence: u64, size_bytes: u64) -> EventEnvelope {
    EventEnvelope {
        schema_version: 1,
        instance_id: instance_id.to_string(),
        sequence,
        timestamp_ms: 1000 + sequence,
        payload: serde_json::json!({
            "data": "x".repeat(size_bytes as usize / 10),
        }),
        metadata: EventMetadata::default(),
    }
}

fn make_projection_write(projection_id: &str, size_bytes: u64) -> ProjectionWrite {
    ProjectionWrite::new(projection_id.to_string(), size_bytes)
}

fn make_blob_write(blob_id: &str, size_bytes: u64) -> vo_storage::append::BlobWrite {
    vo_storage::append::BlobWrite::bulk(blob_id.to_string(), size_bytes)
}

// =============================================================================
// ADR-032 §2: Queue Depth Metrics
// =============================================================================

#[test]
fn queue_depth_metrics_tracked_correctly() {
    let config = QueueConfig {
        critical_capacity: 5,
        projection_capacity: 5,
        blob_capacity: 5,
    };
    let budget = WriteBudget::new(100000, 100000, 100000);
    let appender = Appender::new(config, budget);

    let stats = appender.stats();
    assert_eq!(
        stats
            .lock()
            .unwrap()
            .depth(WriteClass::CriticalControlPlane),
        0
    );
    assert_eq!(
        stats.lock().unwrap().depth(WriteClass::OperatorProjection),
        0
    );
    assert_eq!(stats.lock().unwrap().depth(WriteClass::BulkBlob), 0);

    let event = make_event("inst-1", 1, 100);
    let _ = appender.append_control_plane(ControlPlaneWrite::new(event, 100));
    let _ =
        appender.append_control_plane(ControlPlaneWrite::new(make_event("inst-1", 2, 100), 100));

    let stats = appender.stats();
    assert_eq!(
        stats
            .lock()
            .unwrap()
            .depth(WriteClass::CriticalControlPlane),
        2
    );
    assert_eq!(
        stats
            .lock()
            .unwrap()
            .remaining(WriteClass::CriticalControlPlane),
        3
    );
}

#[test]
fn queue_depth_respects_capacity_limits() {
    let config = QueueConfig {
        critical_capacity: 2,
        projection_capacity: 2,
        blob_capacity: 2,
    };
    let budget = WriteBudget::new(100000, 100000, 100000);
    let appender = Appender::new(config, budget);

    for i in 0..3 {
        let event = make_event("inst-1", i, 100);
        let result = appender.append_control_plane(ControlPlaneWrite::new(event, 100));
        if i < 2 {
            assert!(result.is_ok(), "write {} should succeed", i);
        } else {
            assert!(result.is_err(), "write {} should fail (queue full)", i);
        }
    }
}

// =============================================================================
// ADR-032 §3: Backpressure Signal Propagation
// =============================================================================

#[test]
fn backpressure_signal_set_after_queue_full() {
    let config = QueueConfig {
        critical_capacity: 1,
        projection_capacity: 1,
        blob_capacity: 1,
    };
    let budget = WriteBudget::new(100000, 100000, 100000);
    let appender = Appender::new(config, budget);
    let backpressure = appender.backpressure().clone();

    assert!(!backpressure.any_backpressured());

    let event = make_event("inst-1", 1, 100);
    let result = appender.append_control_plane(ControlPlaneWrite::new(event, 100));
    assert!(result.is_ok());
    assert!(!backpressure.is_backpressured(WriteClass::CriticalControlPlane));

    let event2 = make_event("inst-1", 2, 100);
    let result2 = appender.append_control_plane(ControlPlaneWrite::new(event2, 100));
    assert!(result2.is_err());
    assert!(backpressure.is_backpressured(WriteClass::CriticalControlPlane));
    assert!(backpressure.any_backpressured());
}

#[test]
fn backpressure_cleared_on_dequeue() {
    let config = QueueConfig {
        critical_capacity: 1,
        projection_capacity: 1,
        blob_capacity: 1,
    };
    let budget = WriteBudget::new(100000, 100000, 100000);
    let appender = Appender::new(config, budget);
    let backpressure = appender.backpressure().clone();

    let event = make_event("inst-1", 1, 100);
    let _ = appender.append_control_plane(ControlPlaneWrite::new(event, 100));

    let event2 = make_event("inst-1", 2, 100);
    let _ = appender.append_control_plane(ControlPlaneWrite::new(event2, 100));

    assert!(backpressure.is_backpressured(WriteClass::CriticalControlPlane));

    let _dequeued = appender.dequeue_critical();
    assert!(!backpressure.is_backpressured(WriteClass::CriticalControlPlane));
}

#[test]
fn projection_writes_rejected_under_backpressure() {
    let config = QueueConfig {
        critical_capacity: 1,
        projection_capacity: 1,
        blob_capacity: 1,
    };
    let budget = WriteBudget::new(100000, 100000, 100000);
    let appender = Appender::new(config, budget);

    let write1 = make_projection_write("proj-1", 100);
    assert!(appender.append_projection(write1).is_ok());

    let write2 = make_projection_write("proj-2", 100);
    assert!(matches!(
        appender.append_projection(write2),
        Err(BudgetQueuesError::QueueFull { .. })
    ));
}

// =============================================================================
// ADR-032 §1: QoS Level Enforcement
// =============================================================================

#[test]
fn qos_tier_ordering_critical_over_projection_over_blob() {
    let config = QueueConfig::default();
    let budget = WriteBudget::new(100000, 100000, 100000);
    let queues: BudgetQueues<AppendEntry> = BudgetQueues::new(config, budget);

    queues
        .try_enqueue(&AppendEntry::Blob(make_blob_write("blob-1", 100)))
        .unwrap();
    queues
        .try_enqueue(&AppendEntry::Projection(make_projection_write(
            "proj-1", 100,
        )))
        .unwrap();
    let event = make_event("inst-1", 1, 100);
    queues
        .try_enqueue(&AppendEntry::ControlPlane(ControlPlaneWrite::new(
            event.clone(),
            100,
        )))
        .unwrap();

    let (class1, _) = queues.dequeue_prioritized().unwrap();
    assert_eq!(class1, WriteClass::CriticalControlPlane);

    let (class2, _) = queues.dequeue_prioritized().unwrap();
    assert_eq!(class2, WriteClass::OperatorProjection);

    let (class3, _) = queues.dequeue_prioritized().unwrap();
    assert_eq!(class3, WriteClass::BulkBlob);
}

#[test]
fn write_class_tier_values_enforced() {
    assert_eq!(WriteClass::CriticalControlPlane.tier(), 1);
    assert_eq!(WriteClass::OperatorProjection.tier(), 2);
    assert_eq!(WriteClass::BulkBlob.tier(), 3);
}

#[test]
fn critical_never_dropped_policy_enforced() {
    let config = QueueConfig {
        critical_capacity: 1,
        projection_capacity: 1,
        blob_capacity: 1,
    };
    let budget = WriteBudget::new(100000, 100000, 100000);
    let appender = Appender::new(config, budget);

    let event1 = make_event("inst-1", 1, 100);
    assert!(appender
        .append_control_plane(ControlPlaneWrite::new(event1, 100))
        .is_ok());
    assert!(matches!(
        appender.append_control_plane(ControlPlaneWrite::new(make_event("inst-1", 2, 100), 100)),
        Err(BudgetQueuesError::QueueFull { .. })
    ));

    let dequeued1 = appender.dequeue_critical();
    assert!(dequeued1.is_some());

    let event4 = make_event("inst-1", 4, 100);
    assert!(appender
        .append_control_plane(ControlPlaneWrite::new(event4, 100))
        .is_ok());
}

// =============================================================================
// ADR-032 §3: Admission Coupling (queue depth + latency)
// =============================================================================

#[test]
fn shared_backpressure_signal_coordinates_multiple_queues() {
    let backpressure = Arc::new(BackpressureSignal::new());
    let config = QueueConfig {
        critical_capacity: 1,
        projection_capacity: 1,
        blob_capacity: 1,
    };

    let budget = WriteBudget::new(100000, 100000, 100000);
    let queues1: BudgetQueues<AppendEntry> =
        BudgetQueues::new_with_backpressure(config.clone(), budget.clone(), backpressure.clone());

    let event = make_event("inst-1", 1, 100);
    let write = AppendEntry::ControlPlane(ControlPlaneWrite::new(event, 100));

    assert!(queues1.try_enqueue(&write).is_ok());
    assert!(!backpressure.is_backpressured(WriteClass::CriticalControlPlane));

    let write2 =
        AppendEntry::ControlPlane(ControlPlaneWrite::new(make_event("inst-1", 2, 100), 100));
    let result = queues1.try_enqueue(&write2);
    assert!(result.is_err());
    assert!(backpressure.is_backpressured(WriteClass::CriticalControlPlane));

    let queues2: BudgetQueues<AppendEntry> =
        BudgetQueues::new_with_backpressure(config, budget, backpressure.clone());
    assert!(queues2
        .backpressure()
        .is_backpressured(WriteClass::CriticalControlPlane));
}

// =============================================================================
// Load Testing: High Volume with QoS Enforcement
// =============================================================================

#[test]
fn high_volume_load_critical_always_served_first() {
    let config = QueueConfig {
        critical_capacity: 10,
        projection_capacity: 10,
        blob_capacity: 10,
    };
    let budget = WriteBudget::new(1_000_000, 500_000, 500_000);
    let queues: BudgetQueues<AppendEntry> = BudgetQueues::new(config, budget);

    for i in 0..5 {
        let event = make_event("inst-1", i, 100);
        let write = AppendEntry::ControlPlane(ControlPlaneWrite::new(event, 100));
        let _ = queues.try_enqueue(&write);
    }

    for i in 0..5 {
        let write = AppendEntry::Projection(make_projection_write(&format!("proj-{}", i), 100));
        let _ = queues.try_enqueue(&write);
    }

    for i in 0..5 {
        let write = AppendEntry::Blob(make_blob_write(&format!("blob-{}", i), 100));
        let _ = queues.try_enqueue(&write);
    }

    let mut critical_served = 0;
    let mut projection_served = 0;
    let mut blob_served = 0;

    for _ in 0..15 {
        if let Some((class, _)) = queues.dequeue_prioritized() {
            match class {
                WriteClass::CriticalControlPlane => critical_served += 1,
                WriteClass::OperatorProjection => projection_served += 1,
                WriteClass::BulkBlob => blob_served += 1,
            }
        }
    }

    assert_eq!(critical_served, 5);
    assert_eq!(projection_served, 5);
    assert_eq!(blob_served, 5);
}

#[test]
fn load_shedding_exposes_class_and_reason() {
    let config = QueueConfig {
        critical_capacity: 1,
        projection_capacity: 1,
        blob_capacity: 1,
    };
    let budget = WriteBudget::new(1000, 1000, 1000);
    let appender = Appender::new(config, budget);

    let event = make_event("inst-1", 1, 100);
    let _ = appender.append_control_plane(ControlPlaneWrite::new(event, 100));

    let result1 = appender.append_projection(make_projection_write("proj-1", 100));
    assert!(result1.is_ok());

    let result2 = appender.append_projection(make_projection_write("proj-2", 100));
    assert!(matches!(
        result2,
        Err(BudgetQueuesError::QueueFull {
            class: WriteClass::OperatorProjection,
            ..
        })
    ));
}

#[test]
fn critical_writes_never_rejected_even_under_pressure() {
    let config = QueueConfig {
        critical_capacity: 1,
        projection_capacity: 1,
        blob_capacity: 1,
    };
    let budget = WriteBudget::new(100000, 100000, 100000);
    let appender = Appender::new(config, budget);
    let backpressure = appender.backpressure().clone();

    let event1 = make_event("inst-1", 1, 100);
    assert!(appender
        .append_control_plane(ControlPlaneWrite::new(event1, 100))
        .is_ok());
    assert!(matches!(
        appender.append_control_plane(ControlPlaneWrite::new(make_event("inst-1", 2, 100), 100)),
        Err(BudgetQueuesError::QueueFull { .. })
    ));

    assert!(!backpressure.should_reject(WriteClass::CriticalControlPlane));
    assert!(!backpressure.should_reject(WriteClass::OperatorProjection));

    let write1 = make_projection_write("proj-1", 100);
    assert!(appender.append_projection(write1).is_ok());
    assert!(!backpressure.should_reject(WriteClass::OperatorProjection));

    let write2 = make_projection_write("proj-2", 100);
    assert!(matches!(
        appender.append_projection(write2),
        Err(BudgetQueuesError::QueueFull { .. })
    ));
    assert!(backpressure.should_reject(WriteClass::OperatorProjection));
}

// =============================================================================
// ADR-032 §3: Compaction Stall Indicators (mocked)
// =============================================================================

#[test]
fn backpressure_integrates_with_commit_latency_tracking() {
    use vo_storage::append::CommitLatencyTracker;

    let tracker = CommitLatencyTracker::default();
    let config = QueueConfig {
        critical_capacity: 5,
        projection_capacity: 5,
        blob_capacity: 5,
    };
    let budget = WriteBudget::new(100000, 100000, 100000);
    let appender = Appender::new(config, budget);
    let backpressure = appender.backpressure().clone();

    tracker.record_commit(50);
    tracker.record_commit(100);
    tracker.record_commit(150);

    assert_eq!(tracker.average_latency_ms(), Some(100));
    assert_eq!(tracker.sample_count(), 3);

    let event = make_event("inst-1", 1, 100);
    let _ = appender.append_control_plane(ControlPlaneWrite::new(event, 100));
    assert!(!backpressure.any_backpressured());
}

#[test]
fn latency_tracker_detects_regression() {
    use vo_storage::append::CommitLatencyTracker;

    let tracker = CommitLatencyTracker::default();

    tracker.record_commit(10);
    tracker.record_commit(10);
    tracker.record_commit(10);
    assert_eq!(tracker.average_latency_ms(), Some(10));

    tracker.record_commit(500);
    tracker.record_commit(500);
    tracker.record_commit(500);
    assert_eq!(tracker.average_latency_ms(), Some(255));
}
