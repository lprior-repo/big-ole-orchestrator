//! Unit tests for workload classification and budget tracking.

use super::budget::WorkloadBudget;
use super::types::{RejectionDetail, RejectionReason, WorkloadClass, WorkloadClassError};

// ── WorkloadClass ─────────────────────────────────────────────────────────────

#[test]
fn parse_exact_critical() {
    assert_eq!(
        WorkloadClass::parse("exact_critical"),
        Ok(WorkloadClass::ExactCritical)
    );
}

#[test]
fn parse_standard() {
    assert_eq!(
        WorkloadClass::parse("standard"),
        Ok(WorkloadClass::Standard)
    );
}

#[test]
fn parse_unsafe_bulk() {
    assert_eq!(
        WorkloadClass::parse("unsafe_bulk"),
        Ok(WorkloadClass::UnsafeBulk)
    );
}

#[test]
fn parse_recovery() {
    assert_eq!(
        WorkloadClass::parse("recovery"),
        Ok(WorkloadClass::Recovery)
    );
}

#[test]
fn parse_unknown_returns_err() {
    assert!(WorkloadClass::parse("garbage").is_err());
}

#[test]
fn parse_empty_returns_err() {
    assert!(WorkloadClass::parse("").is_err());
}

#[test]
fn rank_exact_critical_is_0() {
    assert_eq!(WorkloadClass::ExactCritical.rank(), 0);
}

#[test]
fn rank_standard_is_1() {
    assert_eq!(WorkloadClass::Standard.rank(), 1);
}

#[test]
fn rank_recovery_is_2() {
    assert_eq!(WorkloadClass::Recovery.rank(), 2);
}

#[test]
fn rank_unsafe_bulk_is_3() {
    assert_eq!(WorkloadClass::UnsafeBulk.rank(), 3);
}

#[test]
fn classes_ordered_by_priority() {
    assert!(WorkloadClass::ExactCritical < WorkloadClass::Standard);
    assert!(WorkloadClass::Standard < WorkloadClass::Recovery);
    assert!(WorkloadClass::Recovery < WorkloadClass::UnsafeBulk);
}

#[test]
fn never_starved_exact_critical() {
    assert!(WorkloadClass::ExactCritical.never_starved());
}

#[test]
fn never_starved_recovery() {
    assert!(WorkloadClass::Recovery.never_starved());
}

#[test]
fn not_never_starved_standard() {
    assert!(!WorkloadClass::Standard.never_starved());
}

#[test]
fn not_never_starved_unsafe_bulk() {
    assert!(!WorkloadClass::UnsafeBulk.never_starved());
}

#[test]
fn only_unsafe_bulk_is_capped() {
    assert!(WorkloadClass::UnsafeBulk.is_capped_under_contention());
    assert!(!WorkloadClass::ExactCritical.is_capped_under_contention());
    assert!(!WorkloadClass::Standard.is_capped_under_contention());
    assert!(!WorkloadClass::Recovery.is_capped_under_contention());
}

#[test]
fn as_str_roundtrips() {
    for class in WorkloadClass::all_by_priority() {
        assert_eq!(WorkloadClass::parse(class.as_str()), Ok(*class));
    }
}

#[test]
fn all_by_priority_contains_all() {
    assert_eq!(WorkloadClass::all_by_priority().len(), 4);
}

#[test]
fn default_is_standard() {
    assert_eq!(WorkloadClass::default(), WorkloadClass::Standard);
}

#[test]
fn from_str_delegates_to_parse() {
    assert_eq!(
        "exact_critical".parse::<WorkloadClass>(),
        Ok(WorkloadClass::ExactCritical)
    );
}

#[test]
fn json_roundtrip_preserves_variant() {
    for class in WorkloadClass::all_by_priority() {
        let json = serde_json::to_string(&class).unwrap();
        let parsed: WorkloadClass = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, *class);
    }
}

// ── WorkloadBudget ────────────────────────────────────────────────────────────

#[test]
fn budget_remaining_matches_initial() {
    let budget = WorkloadBudget::new(10, 20, 8, 5);
    assert_eq!(budget.remaining(WorkloadClass::ExactCritical), 10);
    assert_eq!(budget.remaining(WorkloadClass::Standard), 20);
    assert_eq!(budget.remaining(WorkloadClass::UnsafeBulk), 5);
    assert_eq!(budget.remaining(WorkloadClass::Recovery), 8);
}

#[test]
fn budget_acquire_deducts_permit() {
    let budget = WorkloadBudget::new(10, 20, 8, 5);
    budget.acquire(WorkloadClass::ExactCritical).unwrap();
    assert_eq!(budget.remaining(WorkloadClass::ExactCritical), 9);
}

#[test]
fn budget_release_restores_permit() {
    let budget = WorkloadBudget::new(10, 20, 8, 5);
    budget.acquire(WorkloadClass::Standard).unwrap();
    budget.release(WorkloadClass::Standard);
    assert_eq!(budget.remaining(WorkloadClass::Standard), 20);
}

#[test]
fn budget_acquire_fails_when_exhausted() {
    let budget = WorkloadBudget::new(1, 0, 0, 0);
    budget.acquire(WorkloadClass::ExactCritical).unwrap();
    assert!(budget.acquire(WorkloadClass::ExactCritical).is_err());
}

#[test]
fn budget_can_acquire_reflects_state() {
    let budget = WorkloadBudget::new(1, 0, 0, 0);
    assert!(budget.can_acquire(WorkloadClass::ExactCritical));
    budget.acquire(WorkloadClass::ExactCritical).unwrap();
    assert!(!budget.can_acquire(WorkloadClass::ExactCritical));
}

#[test]
fn budget_classes_dont_interfere() {
    let budget = WorkloadBudget::new(1, 1, 1, 1);
    budget.acquire(WorkloadClass::ExactCritical).unwrap();
    assert!(budget.can_acquire(WorkloadClass::Standard));
}

#[test]
fn budget_total_reserved_and_used() {
    let budget = WorkloadBudget::new(10, 20, 8, 5);
    assert_eq!(budget.total_reserved(), 43);
    assert_eq!(budget.total_used(), 0);
    budget.acquire(WorkloadClass::ExactCritical).unwrap();
    assert_eq!(budget.total_used(), 1);
}

#[test]
fn budget_reserved_for() {
    let budget = WorkloadBudget::new(10, 20, 8, 5);
    assert_eq!(budget.reserved_for(WorkloadClass::ExactCritical), 10);
}

#[test]
fn budget_default_budget() {
    let budget = WorkloadBudget::default_budget();
    assert!(budget.total_reserved() > 0);
    for class in WorkloadClass::all_by_priority() {
        assert!(budget.can_acquire(*class));
    }
}

// ── RejectionDetail ──────────────────────────────────────────────────────────

#[test]
fn rejection_detail_budget_exhausted() {
    let detail = RejectionDetail::budget_exhausted(WorkloadClass::UnsafeBulk);
    assert_eq!(detail.reason, RejectionReason::BudgetExhausted);
}

#[test]
fn rejection_detail_workflow_cap() {
    let detail = RejectionDetail::workflow_cap_exceeded(WorkloadClass::Standard);
    assert_eq!(detail.reason, RejectionReason::WorkflowCapExceeded);
}

#[test]
fn rejection_detail_global_limit() {
    let detail = RejectionDetail::global_limit(WorkloadClass::ExactCritical);
    assert_eq!(detail.reason, RejectionReason::GlobalConcurrencyLimit);
}

#[test]
fn rejection_detail_display_includes_class() {
    let detail = RejectionDetail::budget_exhausted(WorkloadClass::UnsafeBulk);
    let msg = detail.to_string();
    assert!(msg.contains("UnsafeBulk"));
    assert!(msg.contains("budget exhausted"));
}
