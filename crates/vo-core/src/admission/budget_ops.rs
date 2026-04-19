//! Pure budget calculation functions for admission control.
//!
//! Contains the pure data-calc functions for checking, acquiring, releasing
//! slots and computing degraded mode from pressure state.

use super::budget::WorkloadBudget;
use super::workload::{DegradedMode, WorkloadClass};

// ─────────────────────────────────────────────────────────────────────────────
// BudgetCheckResult Enum
// ─────────────────────────────────────────────────────────────────────────────

/// Result of a budget check for a workload class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetCheckResult {
    /// Slot available, work accepted.
    Accepted { remaining: u32 },
    /// No slots available for this class.
    Rejected { reason: BudgetRejectionReason },
    /// Class is blocked by degraded mode.
    DegradedBlocked { mode: DegradedMode },
}

/// Reasons for budget rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetRejectionReason {
    /// All slots for this class are in use.
    SlotsExhausted { class: WorkloadClass, max: u32 },
    /// Global budget exhausted (total across all classes).
    GlobalBudgetExhausted,
}

// ─────────────────────────────────────────────────────────────────────────────
// Pure Calculation Functions
// ─────────────────────────────────────────────────────────────────────────────

/// Check if a workload class can acquire a slot.
///
/// Pure function, no I/O or side effects.
///
/// # Arguments
/// * `budget` - The current workload budget
/// * `class` - The workload class requesting admission
///
/// # Returns
/// `BudgetCheckResult::Accepted` if slot is available,
/// `BudgetCheckResult::Rejected` if exhausted,
/// `BudgetCheckResult::DegradedBlocked` if degraded mode prevents admission.
#[must_use]
pub fn check_budget(budget: &WorkloadBudget, class: WorkloadClass) -> BudgetCheckResult {
    if !is_class_accepted_in_mode(class, budget.degraded_mode()) {
        return BudgetCheckResult::DegradedBlocked {
            mode: budget.degraded_mode(),
        };
    }

    match budget.allocation_for(class) {
        Some(allocation) if allocation.can_acquire() => BudgetCheckResult::Accepted {
            remaining: allocation.remaining(),
        },
        Some(allocation) => BudgetCheckResult::Rejected {
            reason: BudgetRejectionReason::SlotsExhausted {
                class,
                max: allocation.max_slots,
            },
        },
        None => BudgetCheckResult::Rejected {
            reason: BudgetRejectionReason::SlotsExhausted { class, max: 0 },
        },
    }
}

/// Acquire a slot for a workload class.
///
/// Pure function - returns a new budget with the slot reserved.
///
/// # Arguments
/// * `budget` - The current workload budget
/// * `class` - The workload class acquiring a slot
///
/// # Returns
/// `Ok(updated_budget)` if slot acquired, `Err(reason)` if rejected.
pub fn acquire_slot(
    budget: &WorkloadBudget,
    class: WorkloadClass,
) -> Result<WorkloadBudget, BudgetRejectionReason> {
    match check_budget(budget, class) {
        BudgetCheckResult::Accepted { .. } => {
            let mut new_allocations = budget.allocations().to_vec();
            if let Some(idx) = new_allocations.iter().position(|a| a.class == class) {
                new_allocations[idx].used_slots += 1;
            }
            Ok(WorkloadBudget {
                allocations: new_allocations,
                total_max_slots: budget.total_max_slots,
                total_used_slots: budget.total_used_slots + 1,
                degraded_mode: budget.degraded_mode.clone(),
            })
        }
        BudgetCheckResult::Rejected { reason } => Err(reason),
        BudgetCheckResult::DegradedBlocked { .. } => {
            Err(BudgetRejectionReason::SlotsExhausted { class, max: 0 })
        }
    }
}

/// Release a slot for a workload class.
///
/// Pure function - returns a new budget with the slot freed.
///
/// # Arguments
/// * `budget` - The current workload budget
/// * `class` - The workload class releasing a slot
///
/// # Returns
/// Updated budget with the slot released. Never fails.
#[must_use]
pub fn release_slot(budget: &WorkloadBudget, class: WorkloadClass) -> WorkloadBudget {
    let mut new_allocations = budget.allocations().to_vec();
    if let Some(idx) = new_allocations.iter().position(|a| a.class == class) {
        new_allocations[idx].used_slots = new_allocations[idx].used_slots.saturating_sub(1);
    }
    WorkloadBudget {
        allocations: new_allocations,
        total_max_slots: budget.total_max_slots,
        total_used_slots: budget.total_used_slots.saturating_sub(1),
        degraded_mode: budget.degraded_mode.clone(),
    }
}

/// Compute degraded mode from pressure state.
///
/// Pure function that determines the appropriate degraded mode based on
/// which pressure indicators are active.
///
/// # Arguments
/// * `pressure` - Current write pressure state
///
/// # Returns
/// `DegradedMode::Normal` if no indicators active,
/// `DegradedMode::Degraded` if 1-2 indicators,
/// `DegradedMode::Critical` if 3+ indicators or critical stalls active.
#[must_use]
pub fn compute_degraded_mode(pressure: &super::types::WritePressureState) -> DegradedMode {
    use super::types::PressureIndicator;

    let mut indicators = Vec::new();

    if pressure.writer_queue_depth > 0 {
        indicators.push(PressureIndicator::WriterQueueDepth);
    }
    if pressure.batch_commit_latency_ms > 0 {
        indicators.push(PressureIndicator::BatchCommitLatency);
    }
    if pressure.blob_queue_depth > 0 {
        indicators.push(PressureIndicator::BlobQueueDepth);
    }
    if pressure.compaction_stall_active {
        indicators.push(PressureIndicator::CompactionStall);
    }
    if pressure.storage_stall_active {
        indicators.push(PressureIndicator::StorageStall);
    }

    match indicators.len() {
        0 => DegradedMode::Normal,
        1..=2 => DegradedMode::Degraded {
            triggers: indicators,
        },
        _ => DegradedMode::Critical {
            triggers: indicators,
        },
    }
}

/// Check if a class is accepted in the given degraded mode.
///
/// Pure function that encodes the admission rules:
///
/// - Normal: All classes accepted
/// - Degraded: Live, Recovery, TimerResume accepted; NonCritical, Background restricted
/// - Critical: Only Live, Recovery accepted
#[must_use]
pub fn is_class_accepted_in_mode(class: WorkloadClass, mode: DegradedMode) -> bool {
    match mode {
        DegradedMode::Normal => true,
        DegradedMode::Degraded { .. } => !class.is_deferred_in_degraded(),
        DegradedMode::Critical { .. } => class.is_accepted_in_critical(),
    }
}

/// Set the degraded mode on a budget.
///
/// Pure function - returns a new budget with the updated mode.
#[must_use]
pub fn set_degraded_mode(budget: WorkloadBudget, mode: DegradedMode) -> WorkloadBudget {
    WorkloadBudget {
        allocations: budget.allocations,
        total_max_slots: budget.total_max_slots,
        total_used_slots: budget.total_used_slots,
        degraded_mode: mode,
    }
}
