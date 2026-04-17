use std::cell::RefCell;

use crate::workload_class::{WorkloadClass, WorkloadClassError};

#[derive(Clone, Debug)]
pub struct WorkloadBudget {
    reserved: [u32; 4],
    used: RefCell<[u32; 4]>,
}

impl WorkloadBudget {
    fn class_index(class: WorkloadClass) -> usize {
        class.rank() as usize
    }

    #[must_use]
    pub fn new(exact_critical: u32, standard: u32, recovery: u32, unsafe_bulk: u32) -> Self {
        Self {
            reserved: [exact_critical, standard, recovery, unsafe_bulk],
            used: RefCell::new([0, 0, 0, 0]),
        }
    }

    #[must_use]
    pub fn default_budget() -> Self {
        Self::new(50, 200, 30, 20)
    }

    #[must_use]
    pub fn remaining(&self, class: WorkloadClass) -> u32 {
        let idx = Self::class_index(class);
        let used = self.used.borrow();
        self.reserved[idx].saturating_sub(used[idx])
    }

    #[must_use]
    pub fn can_acquire(&self, class: WorkloadClass) -> bool {
        self.remaining(class) > 0
    }

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

    pub fn release(&self, class: WorkloadClass) {
        let idx = Self::class_index(class);
        let used = &mut self.used.borrow_mut()[idx];
        *used = used.saturating_sub(1);
    }

    #[must_use]
    pub fn total_reserved(&self) -> u32 {
        self.reserved.iter().sum()
    }

    #[must_use]
    pub fn total_used(&self) -> u32 {
        self.used.borrow().iter().sum()
    }

    #[must_use]
    pub fn reserved_for(&self, class: WorkloadClass) -> u32 {
        self.reserved[Self::class_index(class)]
    }
}

#[derive(Clone, Debug)]
pub struct DegradedBudget {
    inner: WorkloadBudget,
    degraded: bool,
}

impl DegradedBudget {
    #[must_use]
    pub fn new(exact_critical: u32, standard: u32, recovery: u32, unsafe_bulk: u32) -> Self {
        Self {
            inner: WorkloadBudget::new(exact_critical, standard, recovery, unsafe_bulk),
            degraded: false,
        }
    }

    #[must_use]
    pub fn default_budget() -> Self {
        Self::new(50, 200, 30, 20)
    }

    #[must_use]
    pub fn is_degraded(&self) -> bool {
        self.degraded
    }

    pub fn enter_degraded(&mut self) {
        self.degraded = true;
    }

    pub fn exit_degraded(&mut self) {
        self.degraded = false;
    }

    #[must_use]
    pub fn can_acquire(&self, class: WorkloadClass) -> bool {
        if self.degraded && class.is_non_critical() {
            return false;
        }
        self.inner.can_acquire(class)
    }

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

    pub fn release(&self, class: WorkloadClass) {
        self.inner.release(class)
    }

    #[must_use]
    pub fn remaining(&self, class: WorkloadClass) -> u32 {
        if self.degraded && class.is_non_critical() {
            return 0;
        }
        self.inner.remaining(class)
    }

    #[must_use]
    pub fn total_reserved(&self) -> u32 {
        self.inner.total_reserved()
    }

    #[must_use]
    pub fn total_used(&self) -> u32 {
        self.inner.total_used()
    }

    #[must_use]
    pub fn reserved_for(&self, class: WorkloadClass) -> u32 {
        self.inner.reserved_for(class)
    }

    #[must_use]
    pub fn inner(&self) -> &WorkloadBudget {
        &self.inner
    }
}

#[cfg(test)]
#[allow(unused_doc_comments)]
mod proptest_workload_invariants {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn rank_in_range(variant in proptest::sample::select(
            WorkloadClass::all_by_priority().to_vec()
        )) {
            prop_assert!((0..=3u8).contains(&variant.rank()));
        }
    }

    proptest! {
        #[test]
        fn never_starved_matches_protected(variant in proptest::sample::select(
            WorkloadClass::all_by_priority().to_vec()
        )) {
            let never = variant.never_starved();
            let is_protected = matches!(variant, WorkloadClass::ExactCritical | WorkloadClass::Recovery);
            prop_assert_eq!(never, is_protected);
        }
    }

    proptest! {
        #[test]
        fn as_str_roundtrips(variant in proptest::sample::select(
            WorkloadClass::all_by_priority().to_vec()
        )) {
            prop_assert_eq!(WorkloadClass::parse(variant.as_str()), Ok(variant));
        }
    }

    proptest! {
        #[test]
        fn json_roundtrip(variant in proptest::sample::select(
            WorkloadClass::all_by_priority().to_vec()
        )) {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: WorkloadClass = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(parsed, variant);
        }
    }

    proptest! {
        #[test]
        fn budget_never_negative(
            reserved in 0u32..=100,
            acquires in 0u32..=100,
            releases in 0u32..=100,
        ) {
            let class = WorkloadClass::Standard;
            let budget = WorkloadBudget::new(reserved, reserved, reserved, reserved);
            for _ in 0..acquires { let _ = budget.acquire(class); }
            for _ in 0..releases { budget.release(class); }
            prop_assert!(budget.remaining(class) <= reserved);
        }
    }

    proptest! {
        #[test]
        fn can_acquire_consistent(reserved in 1u32..=50) {
            let class = WorkloadClass::ExactCritical;
            let budget = WorkloadBudget::new(reserved, 0, 0, 0);
            for _ in 0..reserved {
                let can = budget.can_acquire(class);
                let result = budget.acquire(class);
                prop_assert_eq!(can, result.is_ok());
            }
            prop_assert!(!budget.can_acquire(class));
        }
    }
}
