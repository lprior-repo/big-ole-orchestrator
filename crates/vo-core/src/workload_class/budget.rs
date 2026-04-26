//! Per-class execution permit budget per ADR-033.

use std::cell::RefCell;

use super::class::WorkloadClass;
use super::error::WorkloadClassError;

/// Per-class execution permit budget per ADR-033.
///
/// Tracks reserved and used permits per workload class.
///
/// # Invariants
/// - `used[class] <= reserved[class]` always holds.
#[derive(Clone, Debug)]
pub struct WorkloadBudget {
    reserved: [u32; 4],
    used: RefCell<[u32; 4]>,
}

impl WorkloadBudget {
    fn class_index(class: WorkloadClass) -> usize {
        class.rank() as usize
    }

    /// Creates a budget with per-class reserved permit counts.
    #[must_use]
    pub fn new(exact_critical: u32, standard: u32, recovery: u32, unsafe_bulk: u32) -> Self {
        Self {
            reserved: [exact_critical, standard, recovery, unsafe_bulk],
            used: RefCell::new([0, 0, 0, 0]),
        }
    }

    /// Returns a budget with sensible defaults for a medium-scale deployment.
    #[must_use]
    pub fn default_budget() -> Self {
        Self::new(50, 200, 30, 20)
    }

    /// Returns remaining permits for a given class.
    #[must_use]
    pub fn remaining(&self, class: WorkloadClass) -> u32 {
        let idx = Self::class_index(class);
        let used = self.used.borrow();
        self.reserved[idx].saturating_sub(used[idx])
    }

    /// Checks if a permit can be acquired for the given class.
    #[must_use]
    pub fn can_acquire(&self, class: WorkloadClass) -> bool {
        self.remaining(class) > 0
    }

    /// Acquires a permit for the given class.
    pub fn acquire(&self, class: WorkloadClass) -> Result<(), WorkloadClassError> {
        let idx = Self::class_index(class);
        if self.remaining(class) == 0 {
            return Err(WorkloadClassError::BudgetExceeded {
                class,
                requested: 1,
                available: 0,
            });
        }
        self.used.borrow_mut()[idx] += 1;
        Ok(())
    }

    /// Releases a previously acquired permit.
    pub fn release(&self, class: WorkloadClass) {
        let idx = Self::class_index(class);
        let used = &mut self.used.borrow_mut()[idx];
        *used = used.saturating_sub(1);
    }

    /// Returns the total reserved permits across all classes.
    #[must_use]
    pub fn total_reserved(&self) -> u32 {
        self.reserved.iter().sum()
    }

    /// Returns the total used permits across all classes.
    #[must_use]
    pub fn total_used(&self) -> u32 {
        self.used.borrow().iter().sum()
    }

    /// Returns the reserved permit count for a given class.
    #[must_use]
    pub fn reserved_for(&self, class: WorkloadClass) -> u32 {
        self.reserved[Self::class_index(class)]
    }
}

/// The specific reason for a budget rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RejectionReason {
    BudgetExhausted,
    WorkflowCapExceeded,
    GlobalConcurrencyLimit,
}

/// Detail about a rejected workload request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectionDetail {
    /// The workload class of the rejected request.
    pub class: WorkloadClass,
    /// Human-readable reason for rejection.
    pub reason: RejectionReason,
}

impl RejectionDetail {
    #[must_use]
    pub fn budget_exhausted(class: WorkloadClass) -> Self {
        Self {
            class,
            reason: RejectionReason::BudgetExhausted,
        }
    }

    #[must_use]
    pub fn workflow_cap_exceeded(class: WorkloadClass) -> Self {
        Self {
            class,
            reason: RejectionReason::WorkflowCapExceeded,
        }
    }

    #[must_use]
    pub fn global_limit(class: WorkloadClass) -> Self {
        Self {
            class,
            reason: RejectionReason::GlobalConcurrencyLimit,
        }
    }
}

impl std::fmt::Display for RejectionDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "rejected {:?}: {}",
            self.class,
            match &self.reason {
                RejectionReason::BudgetExhausted => "class budget exhausted",
                RejectionReason::WorkflowCapExceeded => "per-workflow cap exceeded",
                RejectionReason::GlobalConcurrencyLimit => "global concurrency limit reached",
            }
        )
    }
}
