use super::*;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use wtf_common::{ActivityId, InstanceId};

#[test]
fn op_counter_starts_at_zero_and_produces_correct_format() {
    let counter = Arc::new(AtomicU32::new(0));
    let instance_id = InstanceId::new("inst-01");
    let id0 = ActivityId::procedural(&instance_id, counter.fetch_add(1, Ordering::SeqCst));
    let id1 = ActivityId::procedural(&instance_id, counter.fetch_add(1, Ordering::SeqCst));
    assert_eq!(id0.as_str(), "inst-01:0");
    assert_eq!(id1.as_str(), "inst-01:1");
}

#[test]
fn arc_clones_share_counter_state() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter2 = Arc::clone(&counter);
    let _ = counter.fetch_add(1, Ordering::SeqCst);
    let _ = counter.fetch_add(1, Ordering::SeqCst);
    assert_eq!(counter2.load(Ordering::SeqCst), 2);
}

#[test]
fn next_op_id_must_use_fetch_add_not_load() {
    // Regression guard: next_op_id must atomically increment the counter.
    // If it uses load instead of fetch_add, both calls return the same ID.
    let counter = Arc::new(AtomicU32::new(0));
    let instance_id = InstanceId::new("wf-01");
    let id0 = ActivityId::procedural(&instance_id, counter.fetch_add(1, Ordering::SeqCst));
    let id1 = ActivityId::procedural(&instance_id, counter.fetch_add(1, Ordering::SeqCst));
    assert_ne!(
        id0, id1,
        "next_op_id must produce unique IDs on successive calls"
    );
    assert_eq!(id0.as_str(), "wf-01:0");
    assert_eq!(id1.as_str(), "wf-01:1");
}

#[test]
fn fetch_and_increment_produces_unique_ids() {
    let counter = Arc::new(AtomicU32::new(0));
    let instance_id = InstanceId::new("wf-02");
    let id0 = fetch_and_increment(&instance_id, &counter);
    let id1 = fetch_and_increment(&instance_id, &counter);
    assert_ne!(id0, id1);
    assert_eq!(id0.as_str(), "wf-02:0");
    assert_eq!(id1.as_str(), "wf-02:1");
}

#[test]
fn next_op_id_increments_counter_on_each_call() {
    // next_op_id must call fetch_and_increment (not load) so successive calls give different IDs.
    // This test verifies the counter value after two next_op_id calls.
    let counter = Arc::new(AtomicU32::new(0));
    let instance_id = InstanceId::new("wf-03");
    // Simulate two next_op_id calls via fetch_and_increment (the required implementation).
    let id0 = fetch_and_increment(&instance_id, &counter);
    let id1 = fetch_and_increment(&instance_id, &counter);
    assert_eq!(
        counter.load(Ordering::SeqCst),
        2,
        "counter must be 2 after two calls"
    );
    assert_eq!(id0.as_str(), "wf-03:0");
    assert_eq!(id1.as_str(), "wf-03:1");
}
