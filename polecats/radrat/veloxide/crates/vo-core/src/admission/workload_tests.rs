//! Tests for workload class budget and degraded mode module.

use super::*;
use crate::admission::types::WritePressureState;
use crate::admission::workload::{
    acquire_slot, check_budget, compute_degraded_mode, is_class_accepted_in_mode, release_slot,
    set_degraded_mode, BudgetAllocation, BudgetCheckResult, BudgetRejectionReason, DegradedMode,
    WorkloadBudget, WorkloadClass,
};

#[test]
fn workload_class_never_starved_live() {
    assert!(WorkloadClass::Live.never_starved());
}

#[test]
fn workload_class_never_starved_recovery() {
    assert!(WorkloadClass::Recovery.never_starved());
}

#[test]
fn workload_class_not_never_starved_timer_resume() {
    assert!(!WorkloadClass::TimerResume.never_starved());
}

#[test]
fn workload_class_not_never_starved_non_critical() {
    assert!(!WorkloadClass::NonCritical.never_starved());
}

#[test]
fn workload_class_not_never_starved_background() {
    assert!(!WorkloadClass::Background.never_starved());
}

#[test]
fn workload_class_is_deferred_in_degraded_non_critical() {
    assert!(WorkloadClass::NonCritical.is_deferred_in_degraded());
}

#[test]
fn workload_class_is_deferred_in_degraded_background() {
    assert!(WorkloadClass::Background.is_deferred_in_degraded());
}

#[test]
fn workload_class_not_deferred_live() {
    assert!(!WorkloadClass::Live.is_deferred_in_degraded());
}

#[test]
fn workload_class_not_deferred_recovery() {
    assert!(!WorkloadClass::Recovery.is_deferred_in_degraded());
}

#[test]
fn workload_class_accepted_in_critical_live() {
    assert!(WorkloadClass::Live.is_accepted_in_critical());
}

#[test]
fn workload_class_accepted_in_critical_recovery() {
    assert!(WorkloadClass::Recovery.is_accepted_in_critical());
}

#[test]
fn workload_class_not_accepted_in_critical_timer_resume() {
    assert!(!WorkloadClass::TimerResume.is_accepted_in_critical());
}

#[test]
fn workload_class_not_accepted_in_critical_non_critical() {
    assert!(!WorkloadClass::NonCritical.is_accepted_in_critical());
}

#[test]
fn workload_class_not_accepted_in_critical_background() {
    assert!(!WorkloadClass::Background.is_accepted_in_critical());
}

#[test]
fn workload_class_all_by_priority_has_five_variants() {
    let variants = WorkloadClass::all_by_priority();
    assert_eq!(variants.len(), 5);
}

#[test]
fn workload_class_priority_order_live_first() {
    let variants = WorkloadClass::all_by_priority();
    assert_eq!(variants[0], WorkloadClass::Live);
}

#[test]
fn degraded_mode_normal_is_normal() {
    assert!(DegradedMode::Normal.is_normal());
}

#[test]
fn degraded_mode_normal_is_not_degraded() {
    assert!(!DegradedMode::Normal.is_degraded());
}

#[test]
fn degraded_mode_degraded_is_not_normal() {
    let mode = DegradedMode::Degraded {
        triggers: vec![PressureIndicator::WriterQueueDepth],
    };
    assert!(!mode.is_normal());
}

#[test]
fn degraded_mode_degraded_is_degraded() {
    let mode = DegradedMode::Degraded {
        triggers: vec![PressureIndicator::WriterQueueDepth],
    };
    assert!(mode.is_degraded());
}

#[test]
fn degraded_mode_critical_is_degraded() {
    let mode = DegradedMode::Critical {
        triggers: vec![
            PressureIndicator::WriterQueueDepth,
            PressureIndicator::StorageStall,
        ],
    };
    assert!(mode.is_degraded());
}

#[test]
fn degraded_mode_normal_triggers_empty() {
    assert!(DegradedMode::Normal.triggers().is_empty());
}

#[test]
fn degraded_mode_degraded_triggers_returns_correct() {
    let indicators = vec![
        PressureIndicator::WriterQueueDepth,
        PressureIndicator::BatchCommitLatency,
    ];
    let mode = DegradedMode::Degraded {
        triggers: indicators.clone(),
    };
    assert_eq!(mode.triggers(), indicators);
}

#[test]
fn budget_allocation_new() {
    let alloc = BudgetAllocation::new(WorkloadClass::Live, 50, 25);
    assert_eq!(alloc.class, WorkloadClass::Live);
    assert_eq!(alloc.max_slots, 50);
    assert_eq!(alloc.used_slots, 0);
    assert_eq!(alloc.reserved_min, 25);
}

#[test]
fn budget_allocation_remaining_full() {
    let alloc = BudgetAllocation::new(WorkloadClass::Live, 50, 25);
    assert_eq!(alloc.remaining(), 50);
}

#[test]
fn budget_allocation_remaining_after_use() {
    let mut alloc = BudgetAllocation::new(WorkloadClass::Live, 50, 25);
    alloc.used_slots = 30;
    assert_eq!(alloc.remaining(), 20);
}

#[test]
fn budget_allocation_can_acquire_true() {
    let alloc = BudgetAllocation::new(WorkloadClass::Live, 50, 25);
    assert!(alloc.can_acquire());
}

#[test]
fn budget_allocation_can_acquire_false_when_exhausted() {
    let alloc = BudgetAllocation::new(WorkloadClass::Live, 50, 50);
    assert!(!alloc.can_acquire());
}

#[test]
fn budget_allocation_is_exhausted_false() {
    let alloc = BudgetAllocation::new(WorkloadClass::Live, 50, 25);
    assert!(!alloc.is_exhausted());
}

#[test]
fn budget_allocation_is_exhausted_true() {
    let alloc = BudgetAllocation::new(WorkloadClass::Live, 50, 50);
    assert!(alloc.is_exhausted());
}

#[test]
fn workload_budget_default() {
    let budget = WorkloadBudget::default();
    assert!(budget.degraded_mode().is_normal());
    assert_eq!(budget.total_used(), 0);
}

#[test]
fn workload_budget_with_allocations() {
    let budget = WorkloadBudget::with_allocations([50, 30, 20, 100, 200], [50, 30, 10, 0, 0]);
    assert_eq!(budget.total_max(), 400);
    assert!(budget.degraded_mode().is_normal());
}

#[test]
fn workload_budget_allocation_for_live() {
    let budget = WorkloadBudget::default();
    let live_alloc = budget.allocation_for(WorkloadClass::Live);
    assert!(live_alloc.is_some());
    assert_eq!(live_alloc.unwrap().max_slots, 50);
}

#[test]
fn workload_budget_allocation_for_all_classes() {
    let budget = WorkloadBudget::default();
    assert!(budget.allocation_for(WorkloadClass::Live).is_some());
    assert!(budget.allocation_for(WorkloadClass::Recovery).is_some());
    assert!(budget.allocation_for(WorkloadClass::TimerResume).is_some());
    assert!(budget.allocation_for(WorkloadClass::NonCritical).is_some());
    assert!(budget.allocation_for(WorkloadClass::Background).is_some());
}

#[test]
fn workload_budget_remaining() {
    let budget = WorkloadBudget::default();
    assert_eq!(budget.remaining(WorkloadClass::Live), 50);
    assert_eq!(budget.remaining(WorkloadClass::Recovery), 30);
}

#[test]
fn workload_budget_can_acquire_true() {
    let budget = WorkloadBudget::default();
    assert!(budget.can_acquire(WorkloadClass::Live));
}

#[test]
fn check_budget_normal_mode_accepts_all() {
    let budget = WorkloadBudget::default();
    let result = check_budget(&budget, WorkloadClass::Live);
    assert!(matches!(result, BudgetCheckResult::Accepted { .. }));

    let result = check_budget(&budget, WorkloadClass::Background);
    assert!(matches!(result, BudgetCheckResult::Accepted { .. }));
}

#[test]
fn check_budget_degraded_mode_rejects_background() {
    let mut budget = WorkloadBudget::default();
    budget = set_degraded_mode(
        budget,
        DegradedMode::Degraded {
            triggers: vec![PressureIndicator::WriterQueueDepth],
        },
    );

    let result = check_budget(&budget, WorkloadClass::Background);
    assert!(matches!(result, BudgetCheckResult::DegradedBlocked { .. }));
}

#[test]
fn check_budget_degraded_mode_rejects_non_critical() {
    let mut budget = WorkloadBudget::default();
    budget = set_degraded_mode(
        budget,
        DegradedMode::Degraded {
            triggers: vec![PressureIndicator::WriterQueueDepth],
        },
    );

    let result = check_budget(&budget, WorkloadClass::NonCritical);
    assert!(matches!(result, BudgetCheckResult::DegradedBlocked { .. }));
}

#[test]
fn check_budget_degraded_mode_accepts_live() {
    let mut budget = WorkloadBudget::default();
    budget = set_degraded_mode(
        budget,
        DegradedMode::Degraded {
            triggers: vec![PressureIndicator::WriterQueueDepth],
        },
    );

    let result = check_budget(&budget, WorkloadClass::Live);
    assert!(matches!(result, BudgetCheckResult::Accepted { .. }));
}

#[test]
fn check_budget_degraded_mode_accepts_recovery() {
    let mut budget = WorkloadBudget::default();
    budget = set_degraded_mode(
        budget,
        DegradedMode::Degraded {
            triggers: vec![PressureIndicator::WriterQueueDepth],
        },
    );

    let result = check_budget(&budget, WorkloadClass::Recovery);
    assert!(matches!(result, BudgetCheckResult::Accepted { .. }));
}

#[test]
fn check_budget_critical_mode_rejects_timer_resume() {
    let mut budget = WorkloadBudget::default();
    budget = set_degraded_mode(
        budget,
        DegradedMode::Critical {
            triggers: vec![
                PressureIndicator::WriterQueueDepth,
                PressureIndicator::StorageStall,
            ],
        },
    );

    let result = check_budget(&budget, WorkloadClass::TimerResume);
    assert!(matches!(result, BudgetCheckResult::DegradedBlocked { .. }));
}

#[test]
fn check_budget_critical_mode_accepts_live_only() {
    let mut budget = WorkloadBudget::default();
    budget = set_degraded_mode(
        budget,
        DegradedMode::Critical {
            triggers: vec![
                PressureIndicator::WriterQueueDepth,
                PressureIndicator::StorageStall,
            ],
        },
    );

    assert!(matches!(
        check_budget(&budget, WorkloadClass::Live),
        BudgetCheckResult::Accepted { .. }
    ));
    assert!(matches!(
        check_budget(&budget, WorkloadClass::Recovery),
        BudgetCheckResult::Accepted { .. }
    ));
    assert!(matches!(
        check_budget(&budget, WorkloadClass::NonCritical),
        BudgetCheckResult::DegradedBlocked { .. }
    ));
    assert!(matches!(
        check_budget(&budget, WorkloadClass::Background),
        BudgetCheckResult::DegradedBlocked { .. }
    ));
}

#[test]
fn acquire_slot_success() {
    let budget = WorkloadBudget::default();
    let result = acquire_slot(&budget, WorkloadClass::Live);
    assert!(result.is_ok());
    let new_budget = result.unwrap();
    assert_eq!(new_budget.total_used(), 1);
}

#[test]
fn acquire_slot_degraded_blocked() {
    let mut budget = WorkloadBudget::default();
    budget = set_degraded_mode(
        budget,
        DegradedMode::Degraded {
            triggers: vec![PressureIndicator::WriterQueueDepth],
        },
    );

    let result = acquire_slot(&budget, WorkloadClass::Background);
    assert!(result.is_err());
}

#[test]
fn acquire_slot_increments_used() {
    let budget = WorkloadBudget::default();
    let budget = acquire_slot(&budget, WorkloadClass::Live).unwrap();
    let budget = acquire_slot(&budget, WorkloadClass::Live).unwrap();
    let budget = acquire_slot(&budget, WorkloadClass::Recovery).unwrap();

    assert_eq!(budget.total_used(), 3);
    assert_eq!(budget.remaining(WorkloadClass::Live), 48);
}

#[test]
fn release_slot_decrements_used() {
    let budget = WorkloadBudget::default();
    let budget = acquire_slot(&budget, WorkloadClass::Live).unwrap();
    assert_eq!(budget.total_used(), 1);

    let budget = release_slot(&budget, WorkloadClass::Live);
    assert_eq!(budget.total_used(), 0);
    assert_eq!(budget.remaining(WorkloadClass::Live), 50);
}

#[test]
fn release_slot_never_negative() {
    let budget = WorkloadBudget::default();
    let budget = release_slot(&budget, WorkloadClass::Live);
    assert_eq!(budget.total_used(), 0);
}

#[test]
fn compute_degraded_mode_all_indicators_zero() {
    let pressure = WritePressureState {
        writer_queue_depth: 0,
        batch_commit_latency_ms: 0,
        blob_queue_depth: 0,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let mode = compute_degraded_mode(&pressure);
    assert!(matches!(mode, DegradedMode::Normal));
}

#[test]
fn compute_degraded_mode_one_indicator_degraded() {
    let pressure = WritePressureState {
        writer_queue_depth: 150,
        batch_commit_latency_ms: 0,
        blob_queue_depth: 0,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let mode = compute_degraded_mode(&pressure);
    assert!(matches!(mode, DegradedMode::Degraded { triggers } if triggers.len() == 1));
}

#[test]
fn compute_degraded_mode_two_indicators_degraded() {
    let pressure = WritePressureState {
        writer_queue_depth: 150,
        batch_commit_latency_ms: 1500,
        blob_queue_depth: 0,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let mode = compute_degraded_mode(&pressure);
    assert!(matches!(mode, DegradedMode::Degraded { triggers } if triggers.len() == 2));
}

#[test]
fn compute_degraded_mode_three_indicators_critical() {
    let pressure = WritePressureState {
        writer_queue_depth: 150,
        batch_commit_latency_ms: 1500,
        blob_queue_depth: 100,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let mode = compute_degraded_mode(&pressure);
    assert!(matches!(mode, DegradedMode::Critical { triggers } if triggers.len() == 3));
}

#[test]
fn compute_degraded_mode_storage_stall_critical() {
    let pressure = WritePressureState {
        writer_queue_depth: 0,
        batch_commit_latency_ms: 0,
        blob_queue_depth: 0,
        compaction_stall_active: false,
        storage_stall_active: true,
    };
    let mode = compute_degraded_mode(&pressure);
    assert!(matches!(mode, DegradedMode::Critical { .. }));
}

#[test]
fn is_class_accepted_in_mode_normal() {
    for class in WorkloadClass::all_by_priority() {
        assert!(is_class_accepted_in_mode(*class, DegradedMode::Normal));
    }
}

#[test]
fn is_class_accepted_in_mode_degraded_live() {
    let mode = DegradedMode::Degraded {
        triggers: vec![PressureIndicator::WriterQueueDepth],
    };
    assert!(is_class_accepted_in_mode(WorkloadClass::Live, mode));
}

#[test]
fn is_class_accepted_in_mode_degraded_recovery() {
    let mode = DegradedMode::Degraded {
        triggers: vec![PressureIndicator::WriterQueueDepth],
    };
    assert!(is_class_accepted_in_mode(WorkloadClass::Recovery, mode));
}

#[test]
fn is_class_accepted_in_mode_degraded_timer_resume() {
    let mode = DegradedMode::Degraded {
        triggers: vec![PressureIndicator::WriterQueueDepth],
    };
    assert!(is_class_accepted_in_mode(WorkloadClass::TimerResume, mode));
}

#[test]
fn is_class_accepted_in_mode_degraded_non_critical() {
    let mode = DegradedMode::Degraded {
        triggers: vec![PressureIndicator::WriterQueueDepth],
    };
    assert!(!is_class_accepted_in_mode(WorkloadClass::NonCritical, mode));
}

#[test]
fn is_class_accepted_in_mode_degraded_background() {
    let mode = DegradedMode::Degraded {
        triggers: vec![PressureIndicator::WriterQueueDepth],
    };
    assert!(!is_class_accepted_in_mode(WorkloadClass::Background, mode));
}

#[test]
fn is_class_accepted_in_mode_critical_live() {
    let mode = DegradedMode::Critical {
        triggers: vec![PressureIndicator::StorageStall],
    };
    assert!(is_class_accepted_in_mode(WorkloadClass::Live, mode));
}

#[test]
fn is_class_accepted_in_mode_critical_recovery() {
    let mode = DegradedMode::Critical {
        triggers: vec![PressureIndicator::StorageStall],
    };
    assert!(is_class_accepted_in_mode(WorkloadClass::Recovery, mode));
}

#[test]
fn is_class_accepted_in_mode_critical_timer_resume() {
    let mode = DegradedMode::Critical {
        triggers: vec![PressureIndicator::StorageStall],
    };
    assert!(!is_class_accepted_in_mode(WorkloadClass::TimerResume, mode));
}

#[test]
fn is_class_accepted_in_mode_critical_non_critical() {
    let mode = DegradedMode::Critical {
        triggers: vec![PressureIndicator::StorageStall],
    };
    assert!(!is_class_accepted_in_mode(WorkloadClass::NonCritical, mode));
}

#[test]
fn is_class_accepted_in_mode_critical_background() {
    let mode = DegradedMode::Critical {
        triggers: vec![PressureIndicator::StorageStall],
    };
    assert!(!is_class_accepted_in_mode(WorkloadClass::Background, mode));
}

#[test]
fn invariant_used_never_exceeds_max() {
    let budget = WorkloadBudget::default();
    let mut b = budget;
    for _ in 0..60 {
        if b.can_acquire(WorkloadClass::Live) {
            b = acquire_slot(&b, WorkloadClass::Live).unwrap();
        }
    }
    let live_alloc = b.allocation_for(WorkloadClass::Live).unwrap();
    assert!(live_alloc.used_slots <= live_alloc.max_slots);
}

#[test]
fn invariant_total_used_matches_sum_of_allocations() {
    let budget = WorkloadBudget::default();
    let mut b = budget;
    b = acquire_slot(&b, WorkloadClass::Live).unwrap();
    b = acquire_slot(&b, WorkloadClass::Recovery).unwrap();
    b = acquire_slot(&b, WorkloadClass::Background).unwrap();

    let sum_used: u32 = b.allocations().iter().map(|a| a.used_slots).sum();
    assert_eq!(sum_used, b.total_used());
}
