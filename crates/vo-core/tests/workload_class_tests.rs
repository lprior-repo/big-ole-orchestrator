//! WorkloadClass and WorkloadBudget integration tests.

use vo_core::workload_class::{RejectionDetail, RejectionReason, WorkloadBudget, WorkloadClass};
use vo_core::write_class::WriteClass;

#[test]
fn workload_class_priority_ordering() {
    let classes = WorkloadClass::all_by_priority();
    assert_eq!(classes.len(), 4);
    assert_eq!(classes[0], WorkloadClass::ExactCritical);
    assert_eq!(classes[1], WorkloadClass::Standard);
    assert_eq!(classes[2], WorkloadClass::Recovery);
    assert_eq!(classes[3], WorkloadClass::UnsafeBulk);
}

#[test]
fn workload_class_rank_determines_priority() {
    assert_eq!(WorkloadClass::ExactCritical.rank(), 0);
    assert_eq!(WorkloadClass::Standard.rank(), 1);
    assert_eq!(WorkloadClass::Recovery.rank(), 2);
    assert_eq!(WorkloadClass::UnsafeBulk.rank(), 3);
}

#[test]
fn workload_class_never_starved_flag() {
    assert!(
        WorkloadClass::ExactCritical.never_starved(),
        "ExactCritical should never be starved"
    );
    assert!(
        WorkloadClass::Recovery.never_starved(),
        "Recovery should never be starved"
    );
    assert!(
        !WorkloadClass::Standard.never_starved(),
        "Standard may be starved"
    );
    assert!(
        !WorkloadClass::UnsafeBulk.never_starved(),
        "UnsafeBulk may be starved"
    );
}

#[test]
fn workload_class_is_capped_under_contention() {
    assert!(
        WorkloadClass::UnsafeBulk.is_capped_under_contention(),
        "UnsafeBulk should be capped under contention"
    );
    assert!(
        !WorkloadClass::ExactCritical.is_capped_under_contention(),
        "ExactCritical should not be capped"
    );
}

#[test]
fn workload_class_parse_roundtrip() {
    for class in WorkloadClass::all_by_priority() {
        let parsed = WorkloadClass::parse(class.as_str()).expect("parse should succeed");
        assert_eq!(
            parsed,
            *class,
            "parse(\"{}\") should round-trip",
            class.as_str()
        );
    }
}

#[test]
fn workload_class_json_roundtrip() {
    for class in WorkloadClass::all_by_priority() {
        let json = serde_json::to_string(&class).expect("serialization should succeed");
        let parsed: WorkloadClass =
            serde_json::from_str(&json).expect("deserialization should succeed");
        assert_eq!(
            parsed, *class,
            "JSON round-trip should preserve {:?}",
            class
        );
    }
}

#[test]
fn workload_budget_acquire_and_release() {
    let budget = WorkloadBudget::new(10, 20, 5, 8);

    assert_eq!(budget.remaining(WorkloadClass::ExactCritical), 10);
    assert_eq!(budget.remaining(WorkloadClass::Standard), 20);

    budget
        .acquire(WorkloadClass::ExactCritical)
        .expect("acquire should succeed");
    assert_eq!(budget.remaining(WorkloadClass::ExactCritical), 9);

    budget.release(WorkloadClass::ExactCritical);
    assert_eq!(budget.remaining(WorkloadClass::ExactCritical), 10);
}

#[test]
fn workload_budget_exhaustion_blocks_acquire() {
    let budget = WorkloadBudget::new(1, 0, 0, 0);

    budget
        .acquire(WorkloadClass::ExactCritical)
        .expect("first acquire should succeed");
    let result = budget.acquire(WorkloadClass::ExactCritical);
    assert!(result.is_err(), "second acquire should fail when exhausted");

    let err = result.unwrap_err();
    match err {
        vo_core::workload_class::WorkloadClassError::BudgetExceeded { class, .. } => {
            assert_eq!(class, WorkloadClass::ExactCritical);
        }
        vo_core::workload_class::WorkloadClassError::UnknownClass(_) => {
            panic!("Unexpected UnknownClass error")
        }
    }
}

#[test]
fn workload_budget_classes_are_isolated() {
    let budget = WorkloadBudget::new(1, 1, 1, 1);

    budget
        .acquire(WorkloadClass::ExactCritical)
        .expect("acquire should succeed");
    assert!(
        budget.can_acquire(WorkloadClass::Standard),
        "Standard should be unaffected by ExactCritical exhaustion"
    );
}

#[test]
fn workload_budget_total_reserved_and_used() {
    let budget = WorkloadBudget::new(10, 20, 5, 8);
    assert_eq!(budget.total_reserved(), 43);

    budget.acquire(WorkloadClass::ExactCritical).unwrap();
    budget.acquire(WorkloadClass::Standard).unwrap();
    assert_eq!(budget.total_used(), 2);
}

#[test]
fn workload_budget_default_budget_has_sensible_values() {
    let budget = WorkloadBudget::default_budget();
    assert!(budget.total_reserved() > 0);

    for class in WorkloadClass::all_by_priority() {
        assert!(
            budget.can_acquire(*class),
            "default budget should allow acquiring {:?}",
            class
        );
    }
}

#[test]
fn rejection_detail_factory_methods() {
    let detail = RejectionDetail::budget_exhausted(WorkloadClass::UnsafeBulk);
    assert_eq!(detail.class, WorkloadClass::UnsafeBulk);
    assert_eq!(detail.reason, RejectionReason::BudgetExhausted);

    let detail = RejectionDetail::workflow_cap_exceeded(WorkloadClass::Standard);
    assert_eq!(detail.class, WorkloadClass::Standard);
    assert_eq!(detail.reason, RejectionReason::WorkflowCapExceeded);

    let detail = RejectionDetail::global_limit(WorkloadClass::ExactCritical);
    assert_eq!(detail.class, WorkloadClass::ExactCritical);
    assert_eq!(detail.reason, RejectionReason::GlobalConcurrencyLimit);
}

#[test]
fn rejection_detail_display_includes_class_and_reason() {
    let detail = RejectionDetail::budget_exhausted(WorkloadClass::UnsafeBulk);
    let msg = detail.to_string();
    assert!(msg.contains("UnsafeBulk"));
    assert!(msg.contains("budget exhausted"));
}

#[test]
fn workload_class_and_write_class_both_support_critical_writes() {
    assert!(
        WriteClass::CriticalControlPlane.never_drops(),
        "CriticalControlPlane writes should never be dropped"
    );
    assert!(
        WorkloadClass::ExactCritical.never_starved(),
        "ExactCritical workloads should never be starved"
    );

    assert_ne!(
        WriteClass::CriticalControlPlane.never_drops(),
        WriteClass::BulkBlob.never_drops(),
        "Critical and Bulk should have different drop policies"
    );
    assert_ne!(
        WorkloadClass::ExactCritical.never_starved(),
        WorkloadClass::UnsafeBulk.never_starved(),
        "ExactCritical and UnsafeBulk should have different starvation policies"
    );
}

#[test]
fn workload_budget_and_write_budget_compose_independently() {
    let write_budget = vo_core::write_class::WriteBudget::new(100, 200, 300);
    let workload_budget = WorkloadBudget::new(1, 200, 30, 20);

    assert!(
        write_budget.can_write(WriteClass::CriticalControlPlane, 50),
        "write budget should allow critical write"
    );
    assert!(
        workload_budget.can_acquire(WorkloadClass::ExactCritical),
        "workload budget should allow exact critical acquisition"
    );

    write_budget
        .reserve(WriteClass::CriticalControlPlane, 100)
        .expect("reserve should succeed");
    workload_budget
        .acquire(WorkloadClass::ExactCritical)
        .expect("acquire should succeed");

    assert!(
        !write_budget.can_write(WriteClass::CriticalControlPlane, 1),
        "write budget should be exhausted"
    );
    assert!(
        !workload_budget.can_acquire(WorkloadClass::ExactCritical),
        "workload budget should be exhausted"
    );

    assert!(
        write_budget.can_write(WriteClass::BulkBlob, 50),
        "different write class should be unaffected"
    );
    assert!(
        workload_budget.can_acquire(WorkloadClass::Standard),
        "different workload class should be unaffected"
    );
}

#[test]
fn rejection_detail_and_write_class_both_handle_pressure() {
    let write_rejection = WriteClass::BulkBlob.never_drops();
    let workload_rejection = WorkloadClass::UnsafeBulk.is_capped_under_contention();

    assert!(!write_rejection, "BulkBlob may be dropped under pressure");
    assert!(workload_rejection, "UnsafeBulk is capped under contention");

    let detail = RejectionDetail::budget_exhausted(WorkloadClass::UnsafeBulk);
    let msg = detail.to_string();
    assert!(
        msg.contains("UnsafeBulk"),
        "rejection detail should identify the workload class"
    );
}
