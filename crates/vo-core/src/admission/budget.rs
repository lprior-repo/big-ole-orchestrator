//! Budget allocation and workload budget types for admission control.
//!
//! Tracks per-class slot usage and budget allocation for the five workload
//! classes defined in ADR-013.

use serde::{Deserialize, Serialize};

use super::workload::{DegradedMode, WorkloadClass};

// ─────────────────────────────────────────────────────────────────────────────
// BudgetAllocation Struct
// ─────────────────────────────────────────────────────────────────────────────

/// Budget allocation for a single workload class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetAllocation {
    /// The workload class this allocation is for.
    pub class: WorkloadClass,
    /// Maximum concurrent slots for this class.
    pub max_slots: u32,
    /// Currently used slots.
    pub used_slots: u32,
    /// Minimum reserved slots (cannot be borrowed by other classes).
    pub reserved_min: u32,
}

impl BudgetAllocation {
    /// Creates a new budget allocation.
    #[must_use]
    pub fn new(class: WorkloadClass, max_slots: u32, reserved_min: u32) -> Self {
        Self {
            class,
            max_slots,
            used_slots: 0,
            reserved_min,
        }
    }

    /// Returns the remaining slots for this class.
    #[must_use]
    pub fn remaining(&self) -> u32 {
        self.max_slots.saturating_sub(self.used_slots)
    }

    /// Returns `true` if a slot can be acquired for this class.
    #[must_use]
    pub fn can_acquire(&self) -> bool {
        self.used_slots < self.max_slots
    }

    /// Returns `true` if this allocation is exhausted.
    #[must_use]
    pub fn is_exhausted(&self) -> bool {
        self.used_slots >= self.max_slots
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// WorkloadBudget Struct
// ─────────────────────────────────────────────────────────────────────────────

/// Total budget across all workload classes per ADR-013.
///
/// Tracks per-class slot usage and degraded mode state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkloadBudget {
    /// Per-class allocations ordered by priority.
    pub(crate) allocations: Vec<BudgetAllocation>,
    /// Total maximum slots across all classes.
    pub(crate) total_max_slots: u32,
    /// Total currently used slots.
    pub(crate) total_used_slots: u32,
    /// Current degraded mode state.
    pub(crate) degraded_mode: DegradedMode,
}

impl Default for WorkloadBudget {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkloadBudget {
    /// Creates a new workload budget with sensible defaults.
    ///
    /// Default allocation per class:
    /// - Live: 50 slots, 50 reserved
    /// - Recovery: 30 slots, 30 reserved
    /// - TimerResume: 20 slots, 10 reserved
    /// - NonCritical: 100 slots, 0 reserved
    /// - Background: 200 slots, 0 reserved
    #[must_use]
    pub fn new() -> Self {
        let allocations = vec![
            BudgetAllocation::new(WorkloadClass::Live, 50, 50),
            BudgetAllocation::new(WorkloadClass::Recovery, 30, 30),
            BudgetAllocation::new(WorkloadClass::TimerResume, 20, 10),
            BudgetAllocation::new(WorkloadClass::NonCritical, 100, 0),
            BudgetAllocation::new(WorkloadClass::Background, 200, 0),
        ];
        let total_max_slots: u32 = allocations.iter().map(|a| a.max_slots).sum();
        Self {
            allocations,
            total_max_slots,
            total_used_slots: 0,
            degraded_mode: DegradedMode::Normal,
        }
    }

    /// Creates a budget with custom allocations.
    ///
    /// The arrays must have exactly 5 elements matching the 5 WorkloadClass variants
    /// in priority order: Live, Recovery, TimerResume, NonCritical, Background.
    #[must_use]
    pub fn with_allocations(max_slots: [u32; 5], reserved_min: [u32; 5]) -> Self {
        let classes = WorkloadClass::all_by_priority();
        let allocations: Vec<BudgetAllocation> = classes
            .iter()
            .zip(max_slots.iter().zip(reserved_min.iter()))
            .map(|(class, (max, reserved))| BudgetAllocation::new(*class, *max, *reserved))
            .collect();
        let total_max_slots: u32 = allocations.iter().map(|a| a.max_slots).sum();
        Self {
            allocations,
            total_max_slots,
            total_used_slots: 0,
            degraded_mode: DegradedMode::Normal,
        }
    }

    /// Returns the allocation for a specific class.
    #[must_use]
    pub fn allocation_for(&self, class: WorkloadClass) -> Option<&BudgetAllocation> {
        self.allocations.iter().find(|a| a.class == class)
    }

    /// Returns the remaining slots for a specific class.
    #[must_use]
    pub fn remaining(&self, class: WorkloadClass) -> u32 {
        self.allocation_for(class)
            .map(|a| a.remaining())
            .unwrap_or(0)
    }

    /// Returns `true` if a slot can be acquired for the given class.
    #[must_use]
    pub fn can_acquire(&self, class: WorkloadClass) -> bool {
        match self.allocation_for(class) {
            Some(a) => a.can_acquire(),
            None => false,
        }
    }

    /// Returns the current degraded mode.
    #[must_use]
    pub fn degraded_mode(&self) -> DegradedMode {
        self.degraded_mode.clone()
    }

    /// Returns the total reserved slots across all classes.
    #[must_use]
    pub fn total_reserved(&self) -> u32 {
        self.allocations.iter().map(|a| a.reserved_min).sum()
    }

    /// Returns the total used slots across all classes.
    #[must_use]
    pub fn total_used(&self) -> u32 {
        self.total_used_slots
    }

    /// Returns the total maximum slots.
    #[must_use]
    pub fn total_max(&self) -> u32 {
        self.total_max_slots
    }

    /// Returns all allocations.
    #[must_use]
    pub fn allocations(&self) -> &[BudgetAllocation] {
        &self.allocations
    }
}
