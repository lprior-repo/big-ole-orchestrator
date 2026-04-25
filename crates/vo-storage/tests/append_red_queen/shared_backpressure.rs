//! DIMENSION: shared_backpressure
//! ADR-016 §1: Multiple BudgetQueues can share a backpressure signal

#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use vo_storage::append::{
    BackpressureSignal, BudgetQueues, ControlPlaneWrite, ProjectionWrite, QueueConfig,
    WriteBudget, WriteClass,
};

use super::helpers::make_event;

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

    let queues1 = BudgetQueues::new_with_backpressure(&config, budget1, Arc::clone(&shared_signal));
    let queues2 = BudgetQueues::new_with_backpressure(&config, budget2, Arc::clone(&shared_signal));

    let event = make_event("test", 1);
    queues1
        .try_enqueue(&super::AppendEntry::ControlPlane(ControlPlaneWrite::new(
            event.clone(),
            100,
        )))
        .unwrap();
    queues1
        .try_enqueue(&super::AppendEntry::ControlPlane(ControlPlaneWrite::new(
            event.clone(),
            100,
        )))
        .unwrap();

    queues2
        .try_enqueue(&super::AppendEntry::Projection(ProjectionWrite::new(
            "proj-1".to_string(),
            100,
        )))
        .unwrap();
    queues2
        .try_enqueue(&super::AppendEntry::Projection(ProjectionWrite::new(
            "proj-2".to_string(),
            100,
        )))
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
