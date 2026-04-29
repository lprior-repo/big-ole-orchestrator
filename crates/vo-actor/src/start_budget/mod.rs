//! StartError and ReservedPermitBudget for actor start operations (ADR-033).
//!
//! This module contains the error types and permit budget tracking for
//! workload admission control.

pub use crate::fairness::WorkloadClass;

/// Errors from actor start operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StartError {
    #[error("Budget exhausted for {class:?}: requested {requested}, available {available}")]
    BudgetExhaustion {
        class: WorkloadClass,
        requested: u32,
        available: u32,
    },
    #[error("Invalid config: {0}")]
    InvalidConfig(String),
    #[error("At capacity: {running}/{max} instances running")]
    AtCapacity { running: u32, max: u32 },
    #[error("Instance {0} already exists")]
    AlreadyExists(String),
    #[error("Spawn failed: {0}")]
    SpawnFailed(String),
}

/// Reserved permit budget tracking per workload class.
/// Ensures each class maintains its reserved capacity per ADR-033.
#[derive(Debug, Clone)]
pub struct ReservedPermitBudget {
    max_per_class: u32,
    class_counts: std::collections::HashMap<WorkloadClass, u32>,
}

impl ReservedPermitBudget {
    /// Creates a new budget with the specified maximum per class.
    ///
    /// # Errors
    /// Returns `StartError::InvalidConfig` if `max_per_class` is zero.
    #[track_caller]
    pub fn new(max_per_class: u32) -> Result<Self, StartError> {
        if max_per_class == 0 {
            return Err(StartError::InvalidConfig(
                "max_per_class must be > 0".to_string(),
            ));
        }
        Ok(Self {
            max_per_class,
            class_counts: std::collections::HashMap::new(),
        })
    }

    /// Attempts to acquire a permit for the given class.
    ///
    /// # Errors
    /// Returns `StartError::BudgetExhaustion` if no permits available.
    pub fn try_acquire(&mut self, class: WorkloadClass) -> Result<(), StartError> {
        let current = self.class_counts.get(&class).copied().unwrap_or(0);
        if current >= self.max_per_class {
            return Err(StartError::BudgetExhaustion {
                class,
                requested: 1,
                available: self.max_per_class - current,
            });
        }
        *self.class_counts.entry(class).or_insert(0) += 1;
        Ok(())
    }

    /// Releases a permit for the given class.
    /// If count is already zero, this is a no-op.
    pub fn release(&mut self, class: WorkloadClass) {
        let count = self.class_counts.get(&class).copied().unwrap_or(0);
        if count == 0 {
            return;
        }
        self.class_counts.insert(class, count - 1);
    }

    /// Resets all class counts to zero.
    pub fn reset(&mut self) {
        self.class_counts.clear();
    }

    /// Returns the number of available permits for the given class.
    #[must_use]
    pub fn available(&self, class: WorkloadClass) -> u32 {
        let used = self.class_counts.get(&class).copied().unwrap_or(0);
        self.max_per_class.saturating_sub(used)
    }

    /// Returns true if the given class has no available permits.
    #[must_use]
    pub fn is_exhausted(&self, class: WorkloadClass) -> bool {
        self.available(class) == 0
    }
}
