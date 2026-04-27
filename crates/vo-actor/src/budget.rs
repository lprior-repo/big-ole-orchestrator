//! Reserved permit budget tracking per workload class (ADR-033).

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

#[cfg(test)]
mod tests {
    use super::*;

    mod workload_class_tests {
        use super::*;

        #[test]
        fn workload_class_variants_exist() {
            assert!(matches!(WorkloadClass::Recovery, WorkloadClass::Recovery));
            assert!(matches!(
                WorkloadClass::NewInstance,
                WorkloadClass::NewInstance
            ));
            assert!(matches!(WorkloadClass::Internal, WorkloadClass::Internal));
        }

        #[test]
        fn workload_class_debug_format() {
            assert_eq!(format!("{:?}", WorkloadClass::Recovery), "Recovery");
            assert_eq!(format!("{:?}", WorkloadClass::NewInstance), "NewInstance");
            assert_eq!(format!("{:?}", WorkloadClass::Internal), "Internal");
        }

        #[test]
        fn workload_class_eq() {
            assert_eq!(WorkloadClass::Recovery, WorkloadClass::Recovery);
            assert_eq!(WorkloadClass::NewInstance, WorkloadClass::NewInstance);
            assert_eq!(WorkloadClass::Internal, WorkloadClass::Internal);
            assert_ne!(WorkloadClass::Recovery, WorkloadClass::NewInstance);
        }

        #[test]
        fn workload_class_clone() {
            let a = WorkloadClass::Recovery;
            let b = a;
            assert_eq!(a, b);
        }

        #[test]
        fn workload_class_copy() {
            let a = WorkloadClass::Recovery;
            let b = a;
            assert_eq!(a, b);
        }
    }

    mod start_error_tests {
        use super::*;

        #[test]
        fn budget_exhaustion_contains_fields() {
            let err = StartError::BudgetExhaustion {
                class: WorkloadClass::Recovery,
                requested: 1,
                available: 0,
            };
            assert!(matches!(err, StartError::BudgetExhaustion { .. }));
        }

        #[test]
        fn budget_exhaustion_display() {
            let err = StartError::BudgetExhaustion {
                class: WorkloadClass::Recovery,
                requested: 1,
                available: 0,
            };
            let display = format!("{}", err);
            assert!(display.contains("Recovery"));
            assert!(display.contains("requested"));
            assert!(display.contains("available"));
        }

        #[test]
        fn invalid_config_display() {
            let err = StartError::InvalidConfig("test error".to_string());
            let display = format!("{}", err);
            assert!(display.contains("Invalid config"));
            assert!(display.contains("test error"));
        }

        #[test]
        fn budget_exhaustion_partial_eq() {
            let err1 = StartError::BudgetExhaustion {
                class: WorkloadClass::Recovery,
                requested: 1,
                available: 0,
            };
            let err2 = StartError::BudgetExhaustion {
                class: WorkloadClass::Recovery,
                requested: 1,
                available: 0,
            };
            assert_eq!(err1, err2);
        }

        #[test]
        fn budget_exhaustion_different_classes_not_equal() {
            let err1 = StartError::BudgetExhaustion {
                class: WorkloadClass::Recovery,
                requested: 1,
                available: 0,
            };
            let err2 = StartError::BudgetExhaustion {
                class: WorkloadClass::NewInstance,
                requested: 1,
                available: 0,
            };
            assert_ne!(err1, err2);
        }
    }

    mod reserved_permit_budget_tests {
        use super::*;

        #[test]
        fn budget_creation() {
            let budget = ReservedPermitBudget::new(5).unwrap();
            assert_eq!(budget.available(WorkloadClass::Recovery), 5);
            assert_eq!(budget.available(WorkloadClass::NewInstance), 5);
            assert_eq!(budget.available(WorkloadClass::Internal), 5);
        }

        #[test]
        fn budget_acquire_decrements_available() {
            let mut budget = ReservedPermitBudget::new(5).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            assert_eq!(budget.available(WorkloadClass::Recovery), 4);
        }

        #[test]
        fn budget_acquire_multiple() {
            let mut budget = ReservedPermitBudget::new(5).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            assert_eq!(budget.available(WorkloadClass::Recovery), 3);
        }

        #[test]
        fn budget_acquire_returns_err_when_exhausted() {
            let mut budget = ReservedPermitBudget::new(2).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            let result = budget.try_acquire(WorkloadClass::Recovery);
            assert!(matches!(
                result,
                Err(StartError::BudgetExhaustion {
                    class: WorkloadClass::Recovery,
                    requested: 1,
                    available: 0,
                })
            ));
        }

        #[test]
        fn budget_release_increments_available() {
            let mut budget = ReservedPermitBudget::new(5).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.release(WorkloadClass::Recovery);
            assert_eq!(budget.available(WorkloadClass::Recovery), 4);
        }

        #[test]
        fn budget_release_on_zero_is_noop() {
            let mut budget = ReservedPermitBudget::new(5).unwrap();
            budget.release(WorkloadClass::Recovery);
            assert_eq!(budget.available(WorkloadClass::Recovery), 5);
        }

        #[test]
        fn budget_reset_clears_counts() {
            let mut budget = ReservedPermitBudget::new(5).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.try_acquire(WorkloadClass::NewInstance).unwrap();
            budget.reset();
            assert_eq!(budget.available(WorkloadClass::Recovery), 5);
            assert_eq!(budget.available(WorkloadClass::NewInstance), 5);
        }

        #[test]
        fn budget_is_exhausted_false_when_available() {
            let budget = ReservedPermitBudget::new(5).unwrap();
            assert!(!budget.is_exhausted(WorkloadClass::Recovery));
        }

        #[test]
        fn budget_is_exhausted_true_when_empty() {
            let mut budget = ReservedPermitBudget::new(2).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            assert!(budget.is_exhausted(WorkloadClass::Recovery));
        }

        #[test]
        fn budget_classes_are_independent() {
            let mut budget = ReservedPermitBudget::new(3).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            assert!(budget.try_acquire(WorkloadClass::Internal).is_ok());
            assert_eq!(budget.available(WorkloadClass::Internal), 2);
        }

        #[test]
        fn budget_exhaustion_error_contains_class_and_available() {
            let mut budget = ReservedPermitBudget::new(1).unwrap();
            budget.try_acquire(WorkloadClass::Recovery).unwrap();
            let result = budget.try_acquire(WorkloadClass::Recovery);
            match result {
                Err(StartError::BudgetExhaustion {
                    class,
                    requested: _,
                    available,
                }) => {
                    assert_eq!(class, WorkloadClass::Recovery);
                    assert_eq!(available, 0);
                }
                _ => panic!("Expected BudgetExhaustion error"),
            }
        }
    }
}
