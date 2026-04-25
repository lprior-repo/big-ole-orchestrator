//! DIMENSION: stress_fuzz
//! Fuzzing-style tests with random-like patterns

#![allow(clippy::unwrap_used)]

use vo_storage::append::{
    BudgetQueuesError, ControlPlaneWrite, ProjectionWrite, QueueConfig, WriteBudget, WriteClass,
};

use super::helpers::make_event;

#[test]
fn red_queen_stress_alternating_enqueue_dequeue() {
    let config = QueueConfig {
        critical_capacity: 50,
        projection_capacity: 50,
        blob_capacity: 50,
    };
    let budget = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
    let appender = super::super::Appender::new(&config, budget);

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

    let binding = appender.stats();
    let stats = binding.lock().unwrap();
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
    let appender = super::super::Appender::new(&config, budget);

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
                WriteClass::OperatorProjection => appender
                    .append_projection(ProjectionWrite::new(format!("{:?}-{}", class, i), 100)),
                WriteClass::BulkBlob => {
                    appender.append_blob(super::super::BlobWrite::bulk(format!("{:?}-{}", class, i), 100))
                }
            };
            assert!(result.is_ok(), "Write {} for {:?} should succeed", i, class);
        }

        let result = match class {
            WriteClass::CriticalControlPlane => appender
                .append_control_plane(ControlPlaneWrite::new(make_event("overflow", 0), 100)),
            WriteClass::OperatorProjection => {
                appender.append_projection(ProjectionWrite::new("overflow".to_string(), 100))
            }
            WriteClass::BulkBlob => {
                appender.append_blob(super::super::BlobWrite::bulk("overflow".to_string(), 100))
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
    let appender = super::super::Appender::new(&config, budget);

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
    let appender = super::super::Appender::new(&config, budget);

    let event = make_event("test", 1);
    let write = ControlPlaneWrite::new(event, u64::MAX);

    let result = appender.append_control_plane(write);
    assert!(
        matches!(result, Err(BudgetQueuesError::BudgetExceeded { .. })),
        "Write of u64::MAX should fail - exceeds budget"
    );
}
