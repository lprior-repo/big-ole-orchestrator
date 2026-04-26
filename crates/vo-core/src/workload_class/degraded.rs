//! Degraded mode budget restrictions per ADR-013.

use super::budget::WorkloadBudget;
use super::class::WorkloadClass;
use super::error::WorkloadClassError;

/// Budget wrapper that enforces degraded-mode admission per ADR-013.
///
/// When degraded mode is active, non-critical workload classes are restricted:
/// - `ExactCritical` and `Recovery` remain fully available (protected classes)
/// - `Standard` and `UnsafeBulk` are blocked (non-critical classes)
///
/// # Invariants
/// - `inner` is always valid (WorkloadBudget invariant preserved)
/// - When `degraded` is `true`, only protected classes can acquire permits
#[derive(Clone, Debug)]
pub struct DegradedBudget {
    inner: WorkloadBudget,
    degraded: bool,
}

impl DegradedBudget {
    /// Creates a new `DegradedBudget` with the given reserved permit counts.
    #[must_use]
    pub fn new(exact_critical: u32, standard: u32, recovery: u32, unsafe_bulk: u32) -> Self {
        Self {
            inner: WorkloadBudget::new(exact_critical, standard, recovery, unsafe_bulk),
            degraded: false,
        }
    }

    /// Creates a `DegradedBudget` with sensible defaults for a medium-scale deployment.
    #[must_use]
    pub fn default_budget() -> Self {
        Self::new(50, 200, 30, 20)
    }

    /// Returns the current degraded mode status.
    #[must_use]
    pub fn is_degraded(&self) -> bool {
        self.degraded
    }

    /// Enters degraded mode, restricting non-critical workload classes.
    pub fn enter_degraded(&mut self) {
        self.degraded = true;
    }

    /// Exits degraded mode, restoring normal admission for all classes.
    pub fn exit_degraded(&mut self) {
        self.degraded = false;
    }

    /// Returns whether a permit can be acquired for the given class.
    ///
    /// Returns `false` for non-critical classes (`Standard`, `UnsafeBulk`)
    /// when degraded mode is active, regardless of remaining budget.
    #[must_use]
    pub fn can_acquire(&self, class: WorkloadClass) -> bool {
        if self.degraded && class.is_non_critical() {
            return false;
        }
        self.inner.can_acquire(class)
    }

    /// Acquires a permit for the given class.
    ///
    /// Returns an error if:
    /// - The class is non-critical and degraded mode is active
    /// - The class budget is exhausted
    pub fn acquire(&self, class: WorkloadClass) -> Result<(), WorkloadClassError> {
        if self.degraded && class.is_non_critical() {
            return Err(WorkloadClassError::BudgetExceeded {
                class,
                requested: 1,
                available: 0,
            });
        }
        self.inner.acquire(class)
    }

    /// Releases a previously acquired permit.
    pub fn release(&self, class: WorkloadClass) {
        self.inner.release(class)
    }

    /// Returns remaining permits for a given class.
    ///
    /// Returns 0 for non-critical classes when degraded mode is active.
    #[must_use]
    pub fn remaining(&self, class: WorkloadClass) -> u32 {
        if self.degraded && class.is_non_critical() {
            return 0;
        }
        self.inner.remaining(class)
    }

    /// Returns the total reserved permits across all classes.
    #[must_use]
    pub fn total_reserved(&self) -> u32 {
        self.inner.total_reserved()
    }

    /// Returns the total used permits across all classes.
    #[must_use]
    pub fn total_used(&self) -> u32 {
        self.inner.total_used()
    }

    /// Returns the reserved permit count for a given class.
    #[must_use]
    pub fn reserved_for(&self, class: WorkloadClass) -> u32 {
        self.inner.reserved_for(class)
    }

    /// Returns the inner budget for cases that need direct access.
    #[must_use]
    pub fn inner(&self) -> &WorkloadBudget {
        &self.inner
    }
}
