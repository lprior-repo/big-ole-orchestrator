use super::appender::Appender;
use super::backpressure::{BackpressureEvent, BackpressureSignal};
use super::budget::WriteBudget;
use super::entries::{AppendEntry, BlobWrite, ControlPlaneWrite, ProjectionWrite};
use super::latency::CommitLatencyTracker;
use super::queue::{BudgetQueuesError, ClassifiedWrite, QueueConfig};
use super::write_class::WriteClass;
use vo_types::events::EventEnvelope;
use vo_types::events::EventMetadata;

#[test]
fn write_class_tier() {
    assert_eq!(WriteClass::CriticalControlPlane.tier(), 1);
    assert_eq!(WriteClass::OperatorProjection.tier(), 2);
    assert_eq!(WriteClass::BulkBlob.tier(), 3);
}

#[test]
fn write_class_never_drops() {
    assert!(WriteClass::CriticalControlPlane.never_drops());
    assert!(!WriteClass::OperatorProjection.never_drops());
    assert!(!WriteClass::BulkBlob.never_drops());
}

#[test]
fn write_class_from_str() {
    assert_eq!(
        "critical_control_plane".parse::<WriteClass>().unwrap(),
        WriteClass::CriticalControlPlane
    );
    assert_eq!(
        "operator_projection".parse::<WriteClass>().unwrap(),
        WriteClass::OperatorProjection
    );
    assert_eq!(
        "bulk_blob".parse::<WriteClass>().unwrap(),
        WriteClass::BulkBlob
    );
    assert!("invalid".parse::<WriteClass>().is_err());
}

#[test]
fn write_budget_remaining() {
    let budget = WriteBudget::new(100, 200, 300);
    assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 100);
    assert_eq!(budget.remaining(WriteClass::OperatorProjection), 200);
    assert_eq!(budget.remaining(WriteClass::BulkBlob), 300);
}

#[test]
fn write_budget_reserve() {
    let budget = WriteBudget::new(100, 200, 300);
    assert!(budget.reserve(WriteClass::CriticalControlPlane, 50).is_ok());
    assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 50);
}

#[test]
fn write_budget_exceeded() {
    let budget = WriteBudget::new(100, 200, 300);
    let result = budget.reserve(WriteClass::CriticalControlPlane, 150);
    assert!(result.is_err());
    assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 100);
}

#[test]
fn queue_config_default() {
    let config = QueueConfig::default();
    assert_eq!(config.critical_capacity, 1024);
    assert_eq!(config.projection_capacity, 512);
    assert_eq!(config.blob_capacity, 256);
}

#[test]
fn append_entry_classification() {
    let event = EventEnvelope {
        schema_version: 1,
        instance_id: "inst-1".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: EventMetadata::default(),
    };

    let cp_write = AppendEntry::ControlPlane(ControlPlaneWrite::new(event.clone(), 100));
    assert_eq!(cp_write.write_class(), WriteClass::CriticalControlPlane);

    let proj_write = AppendEntry::Projection(ProjectionWrite::new("proj-1".to_string(), 200));
    assert_eq!(proj_write.write_class(), WriteClass::OperatorProjection);

    let blob_write = AppendEntry::Blob(BlobWrite::bulk("blob-1".to_string(), 300));
    assert_eq!(blob_write.write_class(), WriteClass::BulkBlob);
}

#[test]
fn appender_queues_control_plane_write() {
    let config = QueueConfig::default();
    let budget = WriteBudget::new(10000, 10000, 10000);
    let appender = Appender::new(&config, budget);

    let event = EventEnvelope {
        schema_version: 1,
        instance_id: "inst-1".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: EventMetadata::default(),
    };
    let write = ControlPlaneWrite::new(event, 100);

    let result = appender.append_control_plane(write);
    assert!(result.is_ok());
}

#[test]
fn appender_rejects_when_budget_exhausted() {
    let config = QueueConfig::default();
    let budget = WriteBudget::new(50, 50, 50);
    let appender = Appender::new(&config, budget);

    let event = EventEnvelope {
        schema_version: 1,
        instance_id: "inst-1".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({"data": "this is larger than 50 bytes to force budget exceeded"}),
        metadata: EventMetadata::default(),
    };
    let write = ControlPlaneWrite::new(event, 100);

    let result = appender.append_control_plane(write);
    assert!(matches!(
        result,
        Err(BudgetQueuesError::BudgetExceeded { .. })
    ));
}

#[test]
fn appender_rejects_when_queue_full() {
    let config = QueueConfig {
        critical_capacity: 1,
        projection_capacity: 1,
        blob_capacity: 1,
    };
    let budget = WriteBudget::new(10000, 10000, 10000);
    let appender = Appender::new(&config, budget);

    let event = EventEnvelope {
        schema_version: 1,
        instance_id: "inst-1".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: EventMetadata::default(),
    };

    let write1 = ControlPlaneWrite::new(event.clone(), 100);
    assert!(appender.append_control_plane(write1).is_ok());

    let write2 = ControlPlaneWrite::new(event, 100);
    let result = appender.append_control_plane(write2);
    assert!(matches!(result, Err(BudgetQueuesError::QueueFull { .. })));
}

#[test]
fn appender_dequeue_returns_queued_items() {
    let config = QueueConfig::default();
    let budget = WriteBudget::new(10000, 10000, 10000);
    let appender = Appender::new(&config, budget);

    let event = EventEnvelope {
        schema_version: 1,
        instance_id: "inst-1".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: EventMetadata::default(),
    };
    let write = ControlPlaneWrite::new(event, 100);
    assert!(appender.append_control_plane(write).is_ok());

    let dequeued = appender.dequeue_critical();
    assert!(dequeued.is_some());

    let dequeued2 = appender.dequeue_critical();
    assert!(dequeued2.is_none());
}

#[test]
fn backpressure_signal_initial_not_backpressured() {
    let signal = BackpressureSignal::new();
    assert!(!signal.is_backpressured(WriteClass::CriticalControlPlane));
    assert!(!signal.is_backpressured(WriteClass::OperatorProjection));
    assert!(!signal.is_backpressured(WriteClass::BulkBlob));
    assert!(!signal.any_backpressured());
}

#[test]
fn backpressure_signal_set_full_emits_event() {
    let signal = BackpressureSignal::new();
    signal.set_full(WriteClass::OperatorProjection, 50, 100);

    assert!(signal.is_backpressured(WriteClass::OperatorProjection));
    assert!(!signal.is_backpressured(WriteClass::CriticalControlPlane));
    assert!(!signal.is_backpressured(WriteClass::BulkBlob));
    assert!(signal.any_backpressured());

    let event = signal.last_event();
    assert!(matches!(
        event,
        Some(BackpressureEvent::QueueFull {
            class: WriteClass::OperatorProjection,
            depth: 50,
            capacity: 100,
        })
    ));
}

#[test]
fn backpressure_signal_set_writable_clears_backpressure() {
    let signal = BackpressureSignal::new();
    signal.set_full(WriteClass::OperatorProjection, 50, 50);
    assert!(signal.is_backpressured(WriteClass::OperatorProjection));

    signal.set_writable(WriteClass::OperatorProjection, 10);
    assert!(!signal.is_backpressured(WriteClass::OperatorProjection));

    let event = signal.last_event();
    assert!(matches!(
        event,
        Some(BackpressureEvent::QueueWritable {
            class: WriteClass::OperatorProjection,
            remaining_capacity: 10,
        })
    ));
}

#[test]
fn backpressure_signal_critical_never_rejects() {
    let signal = BackpressureSignal::new();
    signal.set_full(WriteClass::CriticalControlPlane, 1024, 1024);
    signal.set_full(WriteClass::OperatorProjection, 100, 100);

    assert!(!signal.should_reject(WriteClass::CriticalControlPlane));
    assert!(signal.should_reject(WriteClass::OperatorProjection));
    assert!(!signal.should_reject(WriteClass::BulkBlob));
}

#[test]
fn backpressure_signal_any_backpressured() {
    let signal = BackpressureSignal::new();
    assert!(!signal.any_backpressured());

    signal.set_full(WriteClass::BulkBlob, 256, 256);
    assert!(signal.any_backpressured());

    signal.set_writable(WriteClass::BulkBlob, 1);
    assert!(!signal.any_backpressured());
}

#[test]
fn commit_latency_tracker_initial_no_data() {
    let tracker = CommitLatencyTracker::default();
    assert!(tracker.average_latency_ms().is_none());
    assert!(tracker.time_since_last_commit().is_none());
    assert_eq!(tracker.sample_count(), 0);
}

#[test]
fn commit_latency_tracker_records_commits() {
    let tracker = CommitLatencyTracker::default();
    tracker.record_commit(100);
    tracker.record_commit(200);

    assert_eq!(tracker.sample_count(), 2);
    assert_eq!(tracker.average_latency_ms(), Some(150));
    assert!(tracker.time_since_last_commit().is_some());
}

#[test]
fn budget_queues_emits_backpressure_on_full() {
    let config = QueueConfig {
        critical_capacity: 1,
        projection_capacity: 1,
        blob_capacity: 1,
    };
    let budget = WriteBudget::new(10000, 10000, 10000);
    let queues = super::queue::BudgetQueues::new(&config, budget);

    let event = EventEnvelope {
        schema_version: 1,
        instance_id: "inst-1".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: EventMetadata::default(),
    };

    let write1 = AppendEntry::ControlPlane(ControlPlaneWrite::new(event.clone(), 100));
    assert!(queues.try_enqueue(&write1).is_ok());
    assert!(!queues
        .backpressure()
        .is_backpressured(WriteClass::CriticalControlPlane));

    let write2 = AppendEntry::ControlPlane(ControlPlaneWrite::new(event, 100));
    let result = queues.try_enqueue(&write2);
    assert!(matches!(result, Err(BudgetQueuesError::QueueFull { .. })));
    assert!(queues
        .backpressure()
        .is_backpressured(WriteClass::CriticalControlPlane));
}

#[test]
fn budget_queues_clears_backpressure_on_dequeue() {
    let config = QueueConfig {
        critical_capacity: 1,
        projection_capacity: 1,
        blob_capacity: 1,
    };
    let budget = WriteBudget::new(10000, 10000, 10000);
    let queues = super::queue::BudgetQueues::new(&config, budget);

    let event = EventEnvelope {
        schema_version: 1,
        instance_id: "inst-1".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: EventMetadata::default(),
    };

    let write1 = AppendEntry::ControlPlane(ControlPlaneWrite::new(event.clone(), 100));
    assert!(queues.try_enqueue(&write1).is_ok());

    let write2 = AppendEntry::ControlPlane(ControlPlaneWrite::new(event.clone(), 100));
    assert!(matches!(
        queues.try_enqueue(&write2),
        Err(BudgetQueuesError::QueueFull { .. })
    ));

    assert!(queues
        .backpressure()
        .is_backpressured(WriteClass::CriticalControlPlane));

    let dequeued = queues.dequeue(WriteClass::CriticalControlPlane);
    assert!(dequeued.is_some());
    assert!(!queues
        .backpressure()
        .is_backpressured(WriteClass::CriticalControlPlane));
}

#[test]
fn dequeue_prioritized_returns_critical_first() {
    let config = QueueConfig::default();
    let budget = WriteBudget::new(10000, 10000, 10000);
    let queues = super::queue::BudgetQueues::new(&config, budget);

    let event = EventEnvelope {
        schema_version: 1,
        instance_id: "inst-1".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: EventMetadata::default(),
    };

    queues
        .try_enqueue(&AppendEntry::Blob(BlobWrite::bulk(
            "blob-1".to_string(),
            100,
        )))
        .unwrap();
    queues
        .try_enqueue(&AppendEntry::Projection(ProjectionWrite::new(
            "proj-1".to_string(),
            100,
        )))
        .unwrap();
    queues
        .try_enqueue(&AppendEntry::ControlPlane(ControlPlaneWrite::new(
            event.clone(),
            100,
        )))
        .unwrap();

    let (class, _) = queues.dequeue_prioritized().unwrap();
    assert_eq!(class, WriteClass::CriticalControlPlane);

    let (class, _) = queues.dequeue_prioritized().unwrap();
    assert_eq!(class, WriteClass::OperatorProjection);

    let (class, _) = queues.dequeue_prioritized().unwrap();
    assert_eq!(class, WriteClass::BulkBlob);

    assert!(queues.dequeue_prioritized().is_none());
}

#[test]
fn dequeue_prioritized_skips_empty_queues() {
    let config = QueueConfig::default();
    let budget = WriteBudget::new(10000, 10000, 10000);
    let queues = super::queue::BudgetQueues::new(&config, budget);

    queues
        .try_enqueue(&AppendEntry::Blob(BlobWrite::bulk(
            "blob-1".to_string(),
            100,
        )))
        .unwrap();
    queues
        .try_enqueue(&AppendEntry::Projection(ProjectionWrite::new(
            "proj-1".to_string(),
            100,
        )))
        .unwrap();

    let (class, _) = queues.dequeue_prioritized().unwrap();
    assert_eq!(class, WriteClass::OperatorProjection);

    let (class, _) = queues.dequeue_prioritized().unwrap();
    assert_eq!(class, WriteClass::BulkBlob);
}

#[test]
fn appender_backpressure_signal_integrated() {
    let config = QueueConfig {
        critical_capacity: 2,
        projection_capacity: 1,
        blob_capacity: 1,
    };
    let budget = WriteBudget::new(10000, 10000, 10000);
    let appender = Appender::new(&config, budget);

    let signal = appender.backpressure().clone();

    let event = EventEnvelope {
        schema_version: 1,
        instance_id: "inst-1".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: EventMetadata::default(),
    };

    let write1 = ProjectionWrite::new("proj-1".to_string(), 100);
    assert!(appender.append_projection(write1).is_ok());

    let write2 = ProjectionWrite::new("proj-2".to_string(), 100);
    assert!(matches!(
        appender.append_projection(write2),
        Err(BudgetQueuesError::QueueFull { .. })
    ));

    assert!(signal.is_backpressured(WriteClass::OperatorProjection));
}

use serial_test::serial;

fn test_event() -> EventEnvelope {
    EventEnvelope {
        schema_version: 1,
        instance_id: "inst-1".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: EventMetadata::default(),
    }
}

#[test]
#[serial]
fn metrics_queue_depth_and_rejection_emitted() {
    let recorder = metrics_util::debugging::DebuggingRecorder::new();
    let snapshot = recorder.snapshotter();
    metrics::set_global_recorder(recorder).expect("install recorder");

    let config = QueueConfig {
        critical_capacity: 1,
        projection_capacity: 1,
        blob_capacity: 1,
    };
    let budget = WriteBudget::new(10000, 10000, 10000);
    let queues = super::queue::BudgetQueues::<AppendEntry>::new(&config, budget);

    queues
        .try_enqueue(&AppendEntry::ControlPlane(ControlPlaneWrite::new(
            test_event(),
            100,
        )))
        .ok();

    let entries = snapshot.snapshot().into_vec();
    let depth_gauges: Vec<_> = entries
        .iter()
        .filter(|(key, _, _, val)| {
            key.key().name() == "vo_storage.queue_depth"
                && matches!(val, metrics_util::debugging::DebugValue::Gauge(_))
        })
        .collect();

    assert!(
        !depth_gauges.is_empty(),
        "expected queue_depth gauge after enqueue"
    );
    let (key, _, _, val) = &depth_gauges[0];
    let labels: Vec<_> = key.key().labels().collect();
    assert!(labels.iter().any(|l| l.value() == "critical_control_plane"));
    if let metrics_util::debugging::DebugValue::Gauge(v) = val {
        assert_eq!(v.0, 1.0);
    }

    queues.dequeue(WriteClass::CriticalControlPlane);

    let entries = snapshot.snapshot().into_vec();
    let depth_after: Vec<_> = entries
        .iter()
        .filter(|(key, _, _, val)| {
            key.key().name() == "vo_storage.queue_depth"
                && matches!(val, metrics_util::debugging::DebugValue::Gauge(_))
                && key
                    .key()
                    .labels()
                    .any(|l| l.value() == "critical_control_plane")
        })
        .collect();

    if let metrics_util::debugging::DebugValue::Gauge(v) = &depth_after.last().unwrap().3 {
        assert_eq!(v.0, 0.0, "gauge should be 0 after dequeue");
    }

    let _ = queues.try_enqueue(&AppendEntry::Projection(ProjectionWrite::new(
        "p1".to_string(),
        100,
    )));
    let _ = queues.try_enqueue(&AppendEntry::Projection(ProjectionWrite::new(
        "p2".to_string(),
        100,
    )));

    let entries = snapshot.snapshot().into_vec();
    let reject_counters: Vec<_> = entries
        .iter()
        .filter(|(key, _, _, val)| {
            key.key().name() == "vo_storage.write_rejected_total"
                && matches!(val, metrics_util::debugging::DebugValue::Counter(_))
        })
        .collect();

    assert!(
        !reject_counters.is_empty(),
        "expected rejection counter after queue full"
    );
    let (key, _, _, val) = &reject_counters[0];
    let labels: Vec<_> = key.key().labels().collect();
    assert!(labels.iter().any(|l| l.value() == "operator_projection"));
    assert!(labels.iter().any(|l| l.value() == "queue_full"));
    if let metrics_util::debugging::DebugValue::Counter(v) = val {
        assert_eq!(*v, 1);
    }

    let budget_config = QueueConfig::default();
    let budget_queues = WriteBudget::new(10, 10, 10);
    let q2 = super::queue::BudgetQueues::<AppendEntry>::new(&budget_config, budget_queues);
    let _ = q2.try_enqueue(&AppendEntry::Blob(BlobWrite::bulk("b1".to_string(), 100)));

    let entries = snapshot.snapshot().into_vec();
    let budget_rejects: Vec<_> = entries
        .iter()
        .filter(|(key, _, _, val)| {
            key.key().name() == "vo_storage.write_rejected_total"
                && matches!(val, metrics_util::debugging::DebugValue::Counter(_))
                && key.key().labels().any(|l| l.value() == "bulk_blob")
                && key.key().labels().any(|l| l.value() == "budget_exceeded")
        })
        .collect();

    assert!(
        !budget_rejects.is_empty(),
        "expected budget_exceeded rejection counter"
    );
}
