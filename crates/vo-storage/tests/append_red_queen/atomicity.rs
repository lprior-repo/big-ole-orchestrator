//! DIMENSION: atomicity
//! ADR-016 §1: Atomic WriteBatches - budget reservation is atomic with enqueue

#![allow(clippy::unwrap_used)]

use vo_storage::append::{
    BudgetQueues, BudgetQueuesError, ControlPlaneWrite, QueueConfig, WriteBudget, WriteClass,
};

use super::helpers::make_event;

#[test]
fn red_queen_atomic_budget_and_enqueue() {
    let config = QueueConfig::default();
    let budget = WriteBudget::new(500, 1_000_000, 1_000_000);
    let queues = BudgetQueues::new(&config, budget);

    let write = super::AppendEntry::ControlPlane(super::helpers::make_control_plane_write(300));
    assert!(queues.try_enqueue(&write).is_ok());

    assert_eq!(
        queues.budget().remaining(WriteClass::CriticalControlPlane),
        200,
        "Budget should be exactly reduced"
    );

    let write2 = super::AppendEntry::ControlPlane(super::helpers::make_control_plane_write(200));
    assert!(queues.try_enqueue(&write2).is_ok());

    let write3 = super::AppendEntry::ControlPlane(super::helpers::make_control_plane_write(1));
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
    let queues = BudgetQueues::new(&config, budget);

    let write = super::AppendEntry::ControlPlane(super::helpers::make_control_plane_write(300));
    assert!(queues.try_enqueue(&write).is_ok());

    let stats_before = queues
        .stats()
        .lock()
        .unwrap()
        .depth(WriteClass::CriticalControlPlane);

    let write2 = super::AppendEntry::ControlPlane(super::helpers::make_control_plane_write(300));
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
    let queues = BudgetQueues::new(&config, budget);

    let write = super::AppendEntry::ControlPlane(super::helpers::make_control_plane_write(500));
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
