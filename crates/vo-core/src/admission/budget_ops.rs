//! Pure budget calculation functions for admission control.
//!
//! This module implements the **Calc** layer of the D-C-A (Data-Calc-Actions)
//! pattern for admission budget management per ADR-013 (System Resilience)
//! and ADR-033 (Fairness and Workload Classes).
//!
//! All functions are pure — no I/O, no mutation, no side effects.
//! They accept immutable references and return new values.

use super::budget::{
    AdmissionBudgetConfig, BudgetSnapshot, BudgetThresholds, ClassBudgetSnapshot,
};
use super::workload::{BudgetAllocation, DegradedMode, WorkloadBudget, WorkloadClass};

// ─────────────────────────────────────────────────────────────────────────────
// Budget Construction
// ─────────────────────────────────────────────────────────────────────────────

/// Build a `WorkloadBudget` from an `AdmissionBudgetConfig`.
///
/// Creates the budget with per-class allocations matching the configuration,
/// all slots initially unused, in Normal degraded mode.
#[must_use]
pub fn build_budget_from_config(config: &AdmissionBudgetConfig) -> WorkloadBudget {
    let allocations: Vec<BudgetAllocation> = config
        .class_configs
        .iter()
        .map(|c| BudgetAllocation::new(c.class, c.max_slots, c.reserved_min))
        .collect();
    let total_max_slots: u32 = allocations.iter().map(|a| a.max_slots).sum();
    WorkloadBudget::from_parts(allocations, total_max_slots, DegradedMode::Normal)
}

// ─────────────────────────────────────────────────────────────────────────────
// Utilization Computation
// ─────────────────────────────────────────────────────────────────────────────

/// Compute per-class utilization as permille (0–1000).
///
/// Returns 0 if the class has no allocation or capacity is zero.
#[must_use]
pub fn compute_class_utilization(budget: &WorkloadBudget, class: WorkloadClass) -> u32 {
    match budget.allocation_for(class) {
        Some(a) if a.max_slots > 0 => {
            (a.used_slots as u64 * 1000 / a.max_slots as u64) as u32
        }
        _ => 0,
    }
}

/// Compute total budget utilization as permille (0–1000).
///
/// Returns 0 if total capacity is zero.
#[must_use]
pub fn compute_total_utilization(budget: &WorkloadBudget) -> u32 {
    let total_max = budget.total_max();
    if total_max == 0 {
        return 0;
    }
    (budget.total_used() as u64 * 1000 / total_max as u64) as u32
}

/// Compute effective capacity for a class considering degraded mode.
///
/// In Normal mode, returns the full remaining capacity.
/// In Degraded mode, returns 0 for NonCritical and Background classes.
/// In Critical mode, returns 0 for all classes except Live and Recovery.
#[must_use]
pub fn compute_effective_capacity(budget: &WorkloadBudget, class: WorkloadClass) -> u32 {
    match budget.degraded_mode() {
        DegradedMode::Normal => budget.remaining(class),
        DegradedMode::Degraded { .. } => {
            if class.is_deferred_in_degraded() {
                0
            } else {
                budget.remaining(class)
            }
        }
        DegradedMode::Critical { .. } => {
            if class.is_accepted_in_critical() {
                budget.remaining(class)
            } else {
                0
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Throttling Decisions
// ─────────────────────────────────────────────────────────────────────────────

/// Determine if a class should be throttled based on utilization threshold.
///
/// Returns `true` if the class utilization permille exceeds the threshold
/// OR if the class has zero effective capacity due to degraded mode.
#[must_use]
pub fn should_throttle_class(
    budget: &WorkloadBudget,
    class: WorkloadClass,
    threshold_permille: u32,
) -> bool {
    let effective = compute_effective_capacity(budget, class);
    if effective == 0 {
        return true;
    }
    compute_class_utilization(budget, class) > threshold_permille
}

/// Determine if the global budget should throttle new admissions.
///
/// Returns `true` if total utilization exceeds the global throttle threshold.
#[must_use]
pub fn should_throttle_global(budget: &WorkloadBudget, threshold_permille: u32) -> bool {
    compute_total_utilization(budget) > threshold_permille
}

// ─────────────────────────────────────────────────────────────────────────────
// Fair-Share Computation
// ─────────────────────────────────────────────────────────────────────────────

/// Compute fair-share allocation for a class based on config weights.
///
/// Returns the number of slots this class is entitled to based on its
/// weight proportion of total weight, applied to available (non-reserved) capacity.
///
/// Fair-share only applies to slots beyond the reserved minimum.
/// Reserved slots are always guaranteed to the owning class.
#[must_use]
pub fn compute_fair_share(config: &AdmissionBudgetConfig, class: WorkloadClass) -> u32 {
    let total_weight = config.total_weight();
    if total_weight == 0 {
        return 0;
    }

    let class_cfg = match config.class_config(class) {
        Some(c) => c,
        None => return 0,
    };

    let total_capacity = config.total_max_slots();
    let total_reserved = config.total_reserved();
    let shareable = total_capacity.saturating_sub(total_reserved);

    let share_of_shareable = (class_cfg.weight as u64 * shareable as u64
        / total_weight as u64) as u32;

    class_cfg.reserved_min.saturating_add(share_of_shareable)
}

// ─────────────────────────────────────────────────────────────────────────────
// Snapshot Computation
// ─────────────────────────────────────────────────────────────────────────────

/// Compute a full point-in-time snapshot of budget utilization.
///
/// Produces a `BudgetSnapshot` with per-class details including utilization
/// permille, available slots, and current degraded mode.
#[must_use]
pub fn compute_snapshot(budget: &WorkloadBudget) -> BudgetSnapshot {
    let class_snapshots: Vec<ClassBudgetSnapshot> = WorkloadClass::all_by_priority()
        .iter()
        .filter_map(|&class| {
            budget.allocation_for(class).map(|a| ClassBudgetSnapshot {
                class,
                capacity: a.max_slots,
                used: a.used_slots,
                reserved: a.reserved_min,
                available: a.remaining(),
                utilization_permille: if a.max_slots > 0 {
                    (a.used_slots as u64 * 1000 / a.max_slots as u64) as u32
                } else {
                    0
                },
            })
        })
        .collect();

    BudgetSnapshot {
        total_capacity: budget.total_max(),
        total_used: budget.total_used(),
        total_reserved: budget.total_reserved(),
        class_snapshots,
        degraded_mode: budget.degraded_mode(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Composite Throttling Check
// ─────────────────────────────────────────────────────────────────────────────

/// Evaluate whether a class should be admitted based on budget state and thresholds.
///
/// Combines degraded mode checks, class-level utilization, and global throttling
/// into a single admission decision. Returns `true` if the class should be
/// admitted (not throttled).
#[must_use]
pub fn is_admittable(
    budget: &WorkloadBudget,
    class: WorkloadClass,
    thresholds: &BudgetThresholds,
) -> bool {
    if compute_effective_capacity(budget, class) == 0 {
        return false;
    }
    if should_throttle_global(budget, thresholds.global_throttle_permille)
        && !class.never_starved()
    {
        return false;
    }
    if should_throttle_class(budget, class, thresholds.critical_utilization_permille)
        && !class.never_starved()
    {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::admission::workload::{
        acquire_slot, set_degraded_mode,
    };
    use crate::admission::types::PressureIndicator;

    fn fresh_budget() -> WorkloadBudget {
        let config = AdmissionBudgetConfig::standard();
        build_budget_from_config(&config)
    }

    #[test]
    fn build_budget_from_config_matches_standard() {
        let budget = fresh_budget();
        assert_eq!(budget.total_max(), 400);
        assert_eq!(budget.total_used(), 0);
        assert_eq!(budget.total_reserved(), 90);
    }

    #[test]
    fn build_budget_from_config_all_classes_present() {
        let budget = fresh_budget();
        for class in WorkloadClass::all_by_priority() {
            assert!(budget.allocation_for(*class).is_some());
        }
    }

    #[test]
    fn compute_class_utilization_empty() {
        let budget = fresh_budget();
        assert_eq!(
            compute_class_utilization(&budget, WorkloadClass::Live),
            0
        );
    }

    #[test]
    fn compute_class_utilization_half() {
        let budget = fresh_budget();
        let budget = acquire_slot(&budget, WorkloadClass::Live).unwrap();
        assert_eq!(
            compute_class_utilization(&budget, WorkloadClass::Live),
            20
        );
    }

    #[test]
    fn compute_class_utilization_full() {
        let config = AdmissionBudgetConfig {
            class_configs: vec![super::super::budget::ClassBudgetConfig::new(
                WorkloadClass::Live,
                2,
                2,
                10,
            )],
            degraded_indicator_threshold: 1,
            critical_indicator_threshold: 3,
            critical_stall_is_critical: true,
        };
        let budget = build_budget_from_config(&config);
        let b = acquire_slot(&budget, WorkloadClass::Live).unwrap();
        let b = acquire_slot(&b, WorkloadClass::Live).unwrap();
        assert_eq!(compute_class_utilization(&b, WorkloadClass::Live), 1000);
    }

    #[test]
    fn compute_total_utilization_empty() {
        let budget = fresh_budget();
        assert_eq!(compute_total_utilization(&budget), 0);
    }

    #[test]
    fn compute_total_utilization_after_acquires() {
        let budget = fresh_budget();
        let b = acquire_slot(&budget, WorkloadClass::Live).unwrap();
        let b = acquire_slot(&b, WorkloadClass::Recovery).unwrap();
        let util = compute_total_utilization(&b);
        assert_eq!(util, 5);
    }

    #[test]
    fn compute_effective_capacity_normal_mode() {
        let budget = fresh_budget();
        for class in WorkloadClass::all_by_priority() {
            assert!(compute_effective_capacity(&budget, *class) > 0);
        }
    }

    #[test]
    fn compute_effective_capacity_degraded_mode() {
        let budget = fresh_budget();
        let degraded = DegradedMode::Degraded {
            triggers: vec![PressureIndicator::WriterQueueDepth],
        };
        let budget = set_degraded_mode(budget, degraded);
        assert!(compute_effective_capacity(&budget, WorkloadClass::Live) > 0);
        assert!(compute_effective_capacity(&budget, WorkloadClass::Recovery) > 0);
        assert_eq!(
            compute_effective_capacity(&budget, WorkloadClass::NonCritical),
            0
        );
        assert_eq!(
            compute_effective_capacity(&budget, WorkloadClass::Background),
            0
        );
    }

    #[test]
    fn compute_effective_capacity_critical_mode() {
        let budget = fresh_budget();
        let critical = DegradedMode::Critical {
            triggers: vec![
                PressureIndicator::CompactionStall,
                PressureIndicator::StorageStall,
            ],
        };
        let budget = set_degraded_mode(budget, critical);
        assert!(compute_effective_capacity(&budget, WorkloadClass::Live) > 0);
        assert!(compute_effective_capacity(&budget, WorkloadClass::Recovery) > 0);
        assert_eq!(
            compute_effective_capacity(&budget, WorkloadClass::TimerResume),
            0
        );
        assert_eq!(
            compute_effective_capacity(&budget, WorkloadClass::NonCritical),
            0
        );
        assert_eq!(
            compute_effective_capacity(&budget, WorkloadClass::Background),
            0
        );
    }

    #[test]
    fn should_throttle_class_normal_utilization() {
        let budget = fresh_budget();
        assert!(!should_throttle_class(&budget, WorkloadClass::Live, 800));
    }

    #[test]
    fn should_throttle_class_degraded_blocks_deferred() {
        let budget = fresh_budget();
        let degraded = DegradedMode::Degraded {
            triggers: vec![PressureIndicator::WriterQueueDepth],
        };
        let budget = set_degraded_mode(budget, degraded);
        assert!(should_throttle_class(&budget, WorkloadClass::Background, 800));
    }

    #[test]
    fn should_throttle_global_below_threshold() {
        let budget = fresh_budget();
        assert!(!should_throttle_global(&budget, 900));
    }

    #[test]
    fn compute_fair_share_proportional() {
        let config = AdmissionBudgetConfig::standard();
        let live_share = compute_fair_share(&config, WorkloadClass::Live);
        let bg_share = compute_fair_share(&config, WorkloadClass::Background);
        assert!(live_share > bg_share, "Live should get more fair-share than Background");
    }

    #[test]
    fn compute_fair_share_includes_reserved() {
        let config = AdmissionBudgetConfig::standard();
        let live_share = compute_fair_share(&config, WorkloadClass::Live);
        assert!(
            live_share >= 50,
            "Live fair-share should include its 50 reserved slots"
        );
    }

    #[test]
    fn compute_snapshot_all_classes_present() {
        let budget = fresh_budget();
        let snapshot = compute_snapshot(&budget);
        assert_eq!(snapshot.class_snapshots.len(), 5);
        assert_eq!(snapshot.total_capacity, 400);
        assert_eq!(snapshot.total_used, 0);
    }

    #[test]
    fn compute_snapshot_reflects_usage() {
        let budget = fresh_budget();
        let b = acquire_slot(&budget, WorkloadClass::Live).unwrap();
        let b = acquire_slot(&b, WorkloadClass::Live).unwrap();
        let snapshot = compute_snapshot(&b);
        assert_eq!(snapshot.total_used, 2);
        let live_snap = snapshot.class_snapshot(WorkloadClass::Live).unwrap();
        assert_eq!(live_snap.used, 2);
        assert_eq!(live_snap.utilization_permille, 40);
    }

    #[test]
    fn is_admittable_normal_mode() {
        let budget = fresh_budget();
        let thresholds = BudgetThresholds::default();
        for class in WorkloadClass::all_by_priority() {
            assert!(
                is_admittable(&budget, *class, &thresholds),
                "{:?} should be admittable in normal mode",
                class
            );
        }
    }

    #[test]
    fn is_admittable_degraded_blocks_deferred() {
        let budget = fresh_budget();
        let degraded = DegradedMode::Degraded {
            triggers: vec![PressureIndicator::WriterQueueDepth],
        };
        let budget = set_degraded_mode(budget, degraded);
        let thresholds = BudgetThresholds::default();
        assert!(is_admittable(&budget, WorkloadClass::Live, &thresholds));
        assert!(!is_admittable(&budget, WorkloadClass::Background, &thresholds));
    }

    #[test]
    fn is_admittable_never_starved_survives_global_throttle() {
        let config = AdmissionBudgetConfig {
            class_configs: vec![
                super::super::budget::ClassBudgetConfig::new(
                    WorkloadClass::Live,
                    10,
                    10,
                    10,
                ),
                super::super::budget::ClassBudgetConfig::new(
                    WorkloadClass::Background,
                    100,
                    0,
                    1,
                ),
            ],
            degraded_indicator_threshold: 1,
            critical_indicator_threshold: 3,
            critical_stall_is_critical: true,
        };
        let mut budget = build_budget_from_config(&config);
        for _ in 0..80 {
            budget = acquire_slot(&budget, WorkloadClass::Background).unwrap();
        }
        let thresholds = BudgetThresholds {
            global_throttle_permille: 500,
            ..BudgetThresholds::default()
        };
        assert!(
            is_admittable(&budget, WorkloadClass::Live, &thresholds),
            "Live should survive global throttle (never starved)"
        );
        assert!(
            !is_admittable(&budget, WorkloadClass::Background, &thresholds),
            "Background should be blocked by global throttle"
        );
    }

    #[test]
    fn compute_snapshot_degraded_mode_preserved() {
        let budget = fresh_budget();
        let degraded = DegradedMode::Degraded {
            triggers: vec![PressureIndicator::CompactionStall],
        };
        let budget = set_degraded_mode(budget, degraded);
        let snapshot = compute_snapshot(&budget);
        assert!(snapshot.degraded_mode.is_degraded());
    }
}
