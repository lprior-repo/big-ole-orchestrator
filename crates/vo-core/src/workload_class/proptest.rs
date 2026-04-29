//! Proptest invariants for workload classification and budget tracking.

#[cfg(test)]
#[allow(unused_doc_comments)]
mod proptest_workload_invariants {
    use super::super::budget::WorkloadBudget;
    use super::super::classification::WorkloadClass;

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
