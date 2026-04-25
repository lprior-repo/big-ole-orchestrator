//! DIMENSION: budget_exhaustion
//! ADR-016 §1: Budget tracking must be consistent under concurrent access

#![allow(clippy::unwrap_used)]

use vo_storage::append::{
    BudgetQueues, BudgetQueuesError, ControlPlaneWrite, QueueConfig, WriteBudget, WriteClass,
};

use super::helpers::make_event;

#[test]
fn red_queen_budget_exhaustion_boundary() {
    let config = QueueConfig::default();
    let budget = WriteBudget::new(500, 1_000_000, 1_000_000);
    let appender = super::super::Appender::new(&config, budget);

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
    let appender = super::super::Appender::new(&config, budget);

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
    let queues = BudgetQueues::new(&config, budget);

    let write1 = super::AppendEntry::ControlPlane(super::helpers::make_control_plane_write(100));
    assert!(queues.try_enqueue(&write1).is_ok());

    let write2 = super::AppendEntry::ControlPlane(super::helpers::make_control_plane_write(100));
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
    let queues = BudgetQueues::new(&config, budget);

    let write = super::AppendEntry::ControlPlane(super::helpers::make_control_plane_write(300));
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
    let queues = BudgetQueues::new(&config, budget);

    let write1 = super::AppendEntry::ControlPlane(super::helpers::make_control_plane_write(100));
    assert!(queues.try_enqueue(&write1).is_ok());

    let initial_remaining = queues.budget().remaining(WriteClass::CriticalControlPlane);

    let write2 = super::AppendEntry::ControlPlane(super::helpers::make_control_plane_write(100));
    let result = queues.try_enqueue(&write2);
    assert!(result.is_err(), "Second write should fail - queue full");

    assert_eq!(
        queues.budget().remaining(WriteClass::CriticalControlPlane),
        initial_remaining,
        "Budget should not change when queue enqueue fails"
    );
}
