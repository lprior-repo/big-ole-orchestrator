use crate::workload_class::{RejectionDetail, RejectionReason, WorkloadClass, WorkloadClassError};

#[test]
fn parse_exact_critical() {
    assert_eq!(WorkloadClass::parse("exact_critical"), Ok(WorkloadClass::ExactCritical));
}

#[test]
fn parse_standard() {
    assert_eq!(WorkloadClass::parse("standard"), Ok(WorkloadClass::Standard));
}

#[test]
fn parse_recovery() {
    assert_eq!(WorkloadClass::parse("recovery"), Ok(WorkloadClass::Recovery));
}

#[test]
fn parse_unsafe_bulk() {
    assert_eq!(WorkloadClass::parse("unsafe_bulk"), Ok(WorkloadClass::UnsafeBulk));
}

#[test]
fn parse_unknown_returns_error() {
    let err = WorkloadClass::parse("nonexistent").unwrap_err();
    assert!(matches!(err, WorkloadClassError::UnknownClass(s) if s == "nonexistent"));
}

#[test]
fn rank_ordering() {
    assert!(WorkloadClass::ExactCritical.rank() < WorkloadClass::Standard.rank());
    assert!(WorkloadClass::Standard.rank() < WorkloadClass::Recovery.rank());
    assert!(WorkloadClass::Recovery.rank() < WorkloadClass::UnsafeBulk.rank());
}

#[test]
fn exact_critical_never_starved() {
    assert!(WorkloadClass::ExactCritical.never_starved());
    assert!(WorkloadClass::Recovery.never_starved());
    assert!(!WorkloadClass::Standard.never_starved());
    assert!(!WorkloadClass::UnsafeBulk.never_starved());
}

#[test]
fn only_unsafe_bulk_capped_under_contention() {
    assert!(WorkloadClass::UnsafeBulk.is_capped_under_contention());
    assert!(!WorkloadClass::ExactCritical.is_capped_under_contention());
    assert!(!WorkloadClass::Standard.is_capped_under_contention());
    assert!(!WorkloadClass::Recovery.is_capped_under_contention());
}

#[test]
fn rejection_detail_display() {
    let detail = RejectionDetail::budget_exhausted(WorkloadClass::Standard);
    let s = format!("{}", detail);
    assert!(s.contains("Standard"));
    assert!(s.contains("budget exhausted"));
}

#[test]
fn rejection_reason_variants() {
    let _ = RejectionDetail {
        class: WorkloadClass::Standard,
        reason: RejectionReason::BudgetExhausted,
    };
    let _ = RejectionDetail {
        class: WorkloadClass::Standard,
        reason: RejectionReason::WorkflowCapExceeded,
    };
    let _ = RejectionDetail {
        class: WorkloadClass::Standard,
        reason: RejectionReason::GlobalConcurrencyLimit,
    };
}
