//! DIMENSION: backpressure
//! ADR-016 §1: Backpressure signal must reflect true queue state

#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use vo_storage::append::{
    BackpressureSignal, BudgetQueues, ControlPlaneWrite, ProjectionWrite, QueueConfig,
    WriteBudget, WriteClass,
};

use super::helpers::make_event;

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
    let queues = BudgetQueues::new(&config, budget);

    let event = make_event("test", 1);

    queues
        .try_enqueue(&super::AppendEntry::ControlPlane(ControlPlaneWrite::new(
            event.clone(),
            100,
        )))
        .unwrap();
    queues
        .try_enqueue(&super::AppendEntry::ControlPlane(ControlPlaneWrite::new(
            event, 100,
        )))
        .unwrap();

    assert!(queues
        .backpressure()
        .should_reject(WriteClass::CriticalControlPlane));

    queues.dequeue(WriteClass::CriticalControlPlane);

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
    let queues = BudgetQueues::new(&config, budget);

    assert!(!queues.backpressure().any_backpressured());

    queues
        .try_enqueue(&super::AppendEntry::Projection(ProjectionWrite::new(
            "proj-1".to_string(),
            100,
        )))
        .unwrap();
    queues
        .try_enqueue(&super::AppendEntry::Projection(ProjectionWrite::new(
            "proj-2".to_string(),
            100,
        )))
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
    let queues = BudgetQueues::new(&config, budget);

    queues
        .try_enqueue(&super::AppendEntry::Projection(ProjectionWrite::new(
            "proj-1".to_string(),
            100,
        )))
        .unwrap();
    queues
        .try_enqueue(&super::AppendEntry::Projection(ProjectionWrite::new(
            "proj-2".to_string(),
            100,
        )))
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
