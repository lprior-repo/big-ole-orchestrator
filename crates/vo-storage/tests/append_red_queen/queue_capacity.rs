//! DIMENSION: queue_capacity
//! ADR-016 §1: Queues are bounded - must handle overflow correctly

#![allow(clippy::unwrap_used)]

use vo_storage::append::{
    BudgetQueuesError, ControlPlaneWrite, ProjectionWrite, QueueConfig, WriteBudget, WriteClass,
};

use super::helpers::make_event;

#[test]
fn red_queen_queue_capacity_exact_fill() {
    let config = QueueConfig {
        critical_capacity: 3,
        projection_capacity: 3,
        blob_capacity: 3,
    };
    let budget = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
    let appender = super::super::Appender::new(&config, budget);

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
    let appender = super::super::Appender::new(&config, budget);

    let event = make_event("test", 1);

    assert!(appender
        .append_control_plane(ControlPlaneWrite::new(event.clone(), 100))
        .is_ok());
    assert!(appender
        .append_projection(ProjectionWrite::new("proj-1".to_string(), 100))
        .is_ok());
    assert!(appender
        .append_blob(super::super::BlobWrite::bulk("blob-1".to_string(), 100))
        .is_ok());

    let binding = appender.stats();
    let stats = binding.lock().unwrap();
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
    let appender = super::super::Appender::new(&config, budget);

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
