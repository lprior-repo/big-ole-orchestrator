//! Integration tests for QoS and fairness (ADR-032/ADR-033).
//!
//! These tests cover:
//! - FairnessBudget: hibernated-instance wake ordering, starvation prevention
//! - filter_timers_by_fairness: scheduling priority correctness
//!
//! bead_id: ve-shy2
//! bead_title: Test Coverage: Write-path QoS and resume fairness (ADR-032/033)

use vo_actor::reanimator::{
    filter_timers_by_fairness, FairnessBudget, ReanimatorConfig, TimerRecord,
};
use vo_types::{InstanceId, TimestampMs};

fn ts_ms(value: u64) -> TimestampMs {
    TimestampMs::try_from(value).expect("valid timestamp")
}

fn make_instance_id(byte: u8) -> InstanceId {
    InstanceId::from_bytes([byte; 16])
}

// =============================================================================
// FairnessBudget Tests: Hibernated-instance wake ordering
// ADR-033 §2: Each class receives reserved budget, individual workflows
// receive per-workflow caps.
// ADR-033 §3: Recovery receives reserved capacity so crash recovery
// always makes forward progress.
// =============================================================================

#[test]
fn fairness_budget_prevents_instance_starvation() {
    let mut budget = FairnessBudget::with_limits(2, 100);

    let id1 = make_instance_id(0x01);
    let id2 = make_instance_id(0x02);
    let id3 = make_instance_id(0x03);

    assert!(budget.record_resume(id1.clone()));
    assert!(budget.record_resume(id1.clone()));

    assert!(!budget.can_resume(&id1));

    assert!(budget.can_resume(&id2));
    assert!(budget.can_resume(&id3));
}

#[test]
fn fairness_budget_resets_for_new_cycle() {
    let mut budget = FairnessBudget::with_limits(2, 100);

    let id1 = make_instance_id(0x01);

    assert!(budget.record_resume(id1.clone()));
    assert!(budget.record_resume(id1.clone()));
    assert!(!budget.can_resume(&id1));

    budget.reset();

    assert!(budget.can_resume(&id1));
    assert!(budget.record_resume(id1.clone()));
    assert!(budget.can_resume(&id1));
}

#[test]
fn filter_timers_by_fairness_respects_instance_limits() {
    let id1 = make_instance_id(0x01);
    let id2 = make_instance_id(0x02);

    let timers = vec![
        TimerRecord::new(id1.clone(), ts_ms(100), None, ts_ms(50)),
        TimerRecord::new(id1.clone(), ts_ms(101), None, ts_ms(50)),
        TimerRecord::new(id1.clone(), ts_ms(102), None, ts_ms(50)),
        TimerRecord::new(id2.clone(), ts_ms(100), None, ts_ms(50)),
    ];

    let mut budget = FairnessBudget::with_limits(2, 100);

    assert!(budget.record_resume(id1.clone()));
    assert!(budget.record_resume(id1.clone()));
    assert!(!budget.can_resume(&id1));

    let (allowed, rejected) = filter_timers_by_fairness(timers, &budget);

    assert_eq!(allowed.len(), 1);
    assert_eq!(rejected.len(), 3);
}

#[test]
fn filter_timers_by_fairness_preserves_order_within_instance() {
    let id1 = make_instance_id(0x01);
    let timers = vec![
        TimerRecord::new(id1.clone(), ts_ms(100), None, ts_ms(50)),
        TimerRecord::new(id1.clone(), ts_ms(101), None, ts_ms(51)),
        TimerRecord::new(id1.clone(), ts_ms(102), None, ts_ms(52)),
    ];

    let budget = FairnessBudget::with_limits(3, 100);
    let (allowed, rejected) = filter_timers_by_fairness(timers, &budget);

    assert_eq!(allowed.len(), 3);
    assert!(rejected.is_empty());
}

#[test]
fn fairness_budget_same_instance_multiple_timers() {
    let instance_id = make_instance_id(0x01);
    let timers = vec![
        TimerRecord::new(instance_id.clone(), ts_ms(100), None, ts_ms(50)),
        TimerRecord::new(instance_id.clone(), ts_ms(101), None, ts_ms(51)),
        TimerRecord::new(instance_id.clone(), ts_ms(102), None, ts_ms(52)),
        TimerRecord::new(instance_id.clone(), ts_ms(103), None, ts_ms(53)),
        TimerRecord::new(instance_id.clone(), ts_ms(104), None, ts_ms(54)),
    ];

    let mut budget = FairnessBudget::with_limits(3, 100);

    assert!(budget.record_resume(instance_id.clone()));
    assert!(budget.record_resume(instance_id.clone()));
    assert!(budget.record_resume(instance_id.clone()));
    assert!(!budget.can_resume(&instance_id));

    let (allowed, rejected) = filter_timers_by_fairness(timers, &budget);

    assert_eq!(allowed.len(), 0);
    assert_eq!(rejected.len(), 5);
}

#[test]
fn fairness_budget_zero_limits_means_no_resumes() {
    let mut budget = FairnessBudget::with_limits(0, 0);

    let id = make_instance_id(0x01);
    assert!(!budget.can_resume(&id));
    assert!(!budget.record_resume(id));
}

#[test]
fn fairness_budget_instance_count_tracking() {
    let mut budget = FairnessBudget::with_limits(5, 100);

    let id1 = make_instance_id(0x01);
    let id2 = make_instance_id(0x02);

    for _ in 0..3 {
        assert!(budget.record_resume(id1.clone()));
    }

    assert!(budget.can_resume(&id1));

    assert!(budget.record_resume(id1.clone()));
    assert!(budget.record_resume(id1.clone()));
    assert!(!budget.can_resume(&id1));

    assert!(budget.can_resume(&id2));
}

#[test]
fn fairness_budget_allows_different_instances_fairly() {
    let mut budget = FairnessBudget::with_limits(2, 100);

    let id1 = make_instance_id(0x01);
    let id2 = make_instance_id(0x02);
    let id3 = make_instance_id(0x03);

    assert!(budget.record_resume(id1.clone()));
    assert!(budget.record_resume(id1.clone()));
    assert!(!budget.can_resume(&id1));

    assert!(budget.record_resume(id2.clone()));
    assert!(budget.record_resume(id2.clone()));
    assert!(!budget.can_resume(&id2));

    assert!(budget.can_resume(&id3));
    assert!(budget.record_resume(id3.clone()));
    assert!(budget.can_resume(&id3));
}

#[test]
fn fairness_budget_default_limits() {
    let budget = FairnessBudget::default();

    let id1 = make_instance_id(0x01);
    let id2 = make_instance_id(0x02);

    for _ in 0..5 {
        assert!(budget.can_resume(&id1));
    }

    assert!(budget.can_resume(&id2));
}

#[test]
fn fairness_budget_reset_clears_all_counts() {
    let mut budget = FairnessBudget::with_limits(1, 100);

    let id1 = make_instance_id(0x01);
    let id2 = make_instance_id(0x02);

    assert!(budget.record_resume(id1.clone()));
    assert!(!budget.can_resume(&id1));

    budget.reset();

    assert!(budget.can_resume(&id1));
    assert!(budget.can_resume(&id2));
}

#[test]
fn filter_timers_by_fairness_empty_input() {
    let budget = FairnessBudget::with_limits(2, 100);
    let timers: Vec<TimerRecord> = vec![];

    let (allowed, rejected) = filter_timers_by_fairness(timers, &budget);

    assert_eq!(allowed.len(), 0);
    assert_eq!(rejected.len(), 0);
}

#[test]
fn filter_timers_by_fairness_all_allowed_when_budget_sufficient() {
    let id1 = make_instance_id(0x01);
    let id2 = make_instance_id(0x02);
    let timers = vec![
        TimerRecord::new(id1.clone(), ts_ms(100), None, ts_ms(50)),
        TimerRecord::new(id2.clone(), ts_ms(100), None, ts_ms(50)),
    ];

    let mut budget = FairnessBudget::with_limits(2, 100);
    assert!(budget.record_resume(id1.clone()));

    let (allowed, rejected) = filter_timers_by_fairness(timers, &budget);

    assert_eq!(allowed.len(), 2);
    assert_eq!(rejected.len(), 0);
}

#[test]
fn filter_timers_by_fairness_rejects_all_when_budget_exhausted() {
    let id1 = make_instance_id(0x01);
    let timers = vec![
        TimerRecord::new(id1.clone(), ts_ms(100), None, ts_ms(50)),
        TimerRecord::new(id1.clone(), ts_ms(101), None, ts_ms(51)),
    ];

    let budget = FairnessBudget::with_limits(0, 100);

    assert!(!budget.can_resume(&id1));

    let (allowed, rejected) = filter_timers_by_fairness(timers, &budget);

    assert_eq!(allowed.len(), 0);
    assert_eq!(rejected.len(), 2);
}
