//! DIMENSION: recovery_simulation
//! ADR-016 §2: Snapshot recovery - verify behavior on re-creation

#![allow(clippy::unwrap_used)]

use vo_storage::append::{ControlPlaneWrite, QueueConfig, WriteBudget, WriteClass};

use super::helpers::make_event;

#[test]
fn red_queen_recovery_new_instance_has_empty_state() {
    let config = QueueConfig::default();
    let budget = WriteBudget::new(1000, 1000, 1000);
    let appender1 = super::super::Appender::new(&config, budget.clone());

    let event = make_event("test", 1);
    appender1
        .append_control_plane(ControlPlaneWrite::new(event, 500))
        .unwrap();

    drop(appender1);

    let appender2 = super::super::Appender::new(&config, budget);

    let binding = appender2.stats();
    let stats = binding.lock().unwrap();
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
    let appender1 = super::super::Appender::new(&config, budget1);

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
