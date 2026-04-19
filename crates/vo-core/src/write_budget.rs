use std::cell::RefCell;

use crate::write_class::{Error, WriteClass};

#[derive(Clone, Debug)]
pub struct WriteBudget {
    critical_limit: u64,
    projection_limit: u64,
    blob_limit: u64,
    critical_used: RefCell<u64>,
    projection_used: RefCell<u64>,
    blob_used: RefCell<u64>,
}

impl WriteBudget {
    pub fn new(critical_limit: u64, projection_limit: u64, blob_limit: u64) -> Self {
        Self {
            critical_limit,
            projection_limit,
            blob_limit,
            critical_used: RefCell::new(0),
            projection_used: RefCell::new(0),
            blob_used: RefCell::new(0),
        }
    }

    pub fn remaining(&self, class: WriteClass) -> u64 {
        match class {
            WriteClass::CriticalControlPlane => self
                .critical_limit
                .saturating_sub(*self.critical_used.borrow()),
            WriteClass::OperatorProjection => self
                .projection_limit
                .saturating_sub(*self.projection_used.borrow()),
            WriteClass::BulkBlob => self.blob_limit.saturating_sub(*self.blob_used.borrow()),
        }
    }

    pub fn can_write(&self, class: WriteClass, size_bytes: u64) -> bool {
        self.remaining(class) >= size_bytes
    }

    pub fn reserve(&self, class: WriteClass, size_bytes: u64) -> Result<(), Error> {
        let remaining = self.remaining(class);
        if remaining < size_bytes {
            return Err(Error::BudgetExceeded {
                class,
                requested: size_bytes,
                available: remaining,
            });
        }

        match class {
            WriteClass::CriticalControlPlane => {
                *self.critical_used.borrow_mut() += size_bytes;
            }
            WriteClass::OperatorProjection => {
                *self.projection_used.borrow_mut() += size_bytes;
            }
            WriteClass::BulkBlob => {
                *self.blob_used.borrow_mut() += size_bytes;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
#[allow(unused_doc_comments)]
mod proptest_write_class_invariants {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn write_class_tier_always_returns_1_2_or_3(variant in proptest::sample::select(&[
            WriteClass::CriticalControlPlane,
            WriteClass::OperatorProjection,
            WriteClass::BulkBlob,
        ])) {
            let tier = variant.tier();
            prop_assert!((1..=3).contains(&tier), "tier() must be 1, 2, or 3, got {}", tier);
        }
    }

    proptest! {
        #[test]
        fn write_class_never_drops_true_only_for_critical_control_plane(variant in proptest::sample::select(&[
            WriteClass::CriticalControlPlane,
            WriteClass::OperatorProjection,
            WriteClass::BulkBlob,
        ])) {
            let never_drops = variant.never_drops();
            let is_critical = matches!(variant, WriteClass::CriticalControlPlane);
            prop_assert_eq!(never_drops, is_critical,
                "never_drops() should be {} for {:?}, got {}",
                is_critical, variant, never_drops);
        }
    }

    proptest! {
        #[test]
        fn write_class_as_str_roundtrips_through_from_str(variant in proptest::sample::select(&[
            WriteClass::CriticalControlPlane,
            WriteClass::OperatorProjection,
            WriteClass::BulkBlob,
        ])) {
            let s = variant.as_str();
            let parsed = WriteClass::parse(s);
            prop_assert_eq!(parsed.clone(), Ok(variant));
            prop_assert_eq!(parsed.as_ref().ok(), Some(&variant),
                "from_str({}) should return Some({:?}), got {:?}", s, variant, parsed);
        }
    }

    proptest! {
        #[test]
        fn write_class_json_roundtrip_preserves_variant(variant in proptest::sample::select(&[
            WriteClass::CriticalControlPlane,
            WriteClass::OperatorProjection,
            WriteClass::BulkBlob,
        ])) {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: WriteClass = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(parsed, variant,
                "JSON round-trip failed for {:?}: serialized to {}, parsed back to {:?}",
                variant, json, parsed);
        }
    }

    proptest! {
        #[test]
        fn write_budget_reserve_never_produces_negative_remaining(
            critical in 0u64..=1000,
            _projection in 0u64..=1000,
            _blob in 0u64..=1000,
            reserve_size in 0u64..=1000,
        ) {
            let budget = WriteBudget::new(critical, 1000, 1000);
            let class = WriteClass::CriticalControlPlane;

            let initial = budget.remaining(class);
            let result = budget.reserve(class, reserve_size);

            if result.is_ok() {
                let remaining = budget.remaining(class);
                prop_assert!(remaining <= initial,
                    "remaining() should be <= {} after successful reserve of {}, was {}",
                    initial, reserve_size, remaining);
            }
        }
    }

    proptest! {
        #[test]
        fn write_budget_can_write_and_reserve_are_consistent(
            critical in 1u64..=1000,
            _projection in 1u64..=1000,
            _blob in 1u64..=1000,
            size in 0u64..=2000,
        ) {
            let budget = WriteBudget::new(critical, 1000, 1000);
            let class = WriteClass::CriticalControlPlane;

            let can_write = budget.can_write(class, size);
            let reserve_result = budget.reserve(class, size);

            prop_assert_eq!(can_write, reserve_result.is_ok(),
                "can_write returned {} but reserve returned {:?}",
                can_write, reserve_result);
        }
    }
}
