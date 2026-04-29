//! DIMENSION: dequeue_ordering
//! ADR-016 §1: CriticalControlPlane writes must be dequeued first (priority)

#![allow(clippy::unwrap_used)]

use vo_storage::append::{
    BudgetQueues, BlobWrite, ControlPlaneWrite, ProjectionWrite, QueueConfig, WriteBudget,
    WriteClass,
};

use super::helpers::make_event;

#[test]
fn red_queen_dequeue_priority_critical_first() {
    let config = QueueConfig::default();
    let budget = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
    let queues = BudgetQueues::new(&config, budget);

    queues
        .try_enqueue(&super::AppendEntry::Blob(BlobWrite::bulk("b1".to_string(), 100)))
        .unwrap();
    queues
        .try_enqueue(&super::AppendEntry::Projection(ProjectionWrite::new(
            "p1".to_string(),
            100,
        )))
        .unwrap();
    queues
        .try_enqueue(&super::AppendEntry::ControlPlane(ControlPlaneWrite::new(
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
    let queues = BudgetQueues::new(&config, budget);

    queues
        .try_enqueue(&super::AppendEntry::Blob(BlobWrite::bulk("b1".to_string(), 100)))
        .unwrap();
    queues
        .try_enqueue(&super::AppendEntry::Projection(ProjectionWrite::new(
            "p1".to_string(),
            100,
        )))
        .unwrap();
    queues
        .try_enqueue(&super::AppendEntry::ControlPlane(ControlPlaneWrite::new(
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
    let queues = BudgetQueues::new(&config, budget);

    queues
        .try_enqueue(&super::AppendEntry::Blob(BlobWrite::bulk("b1".to_string(), 100)))
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
    let queues: BudgetQueues<super::AppendEntry> = BudgetQueues::new(&config, budget);

    assert!(queues.dequeue_prioritized().is_none());
}
