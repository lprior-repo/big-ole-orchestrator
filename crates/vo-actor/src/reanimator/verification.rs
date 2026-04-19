//! KANI verification for Reanimator Loop invariants.
//!
//! This module contains formal verification proofs using the KANI model checker.

#[cfg(kani)]
mod verification {
    use super::types::FairnessBudget;
    use vo_types::InstanceId;

    /// Creates a deterministic InstanceId for KANI testing.
    fn make_kani_id(n: u8) -> InstanceId {
        InstanceId::parse(&format!("01H5JYV4XHGSR2F8KZ9BWNRF{:0>5}", n))
            .expect("valid test instance id")
    }

    // =========================================================================
    // INV-BQ1: Bounded Queue - instance counts never exceed max_per_instance
    // =========================================================================

    #[kani::proof]
    fn verify_bounded_instance_counts() {
        let max_per_instance = kani::any();
        kani::assume(max_per_instance > 0);
        let max_per_workflow = max_per_instance * 10;

        let mut budget = FairnessBudget::with_limits(max_per_instance, max_per_workflow);

        let id1 = make_kani_id(1);
        let id2 = make_kani_id(2);

        // Perform up to 10 resume operations
        for _ in 0..10 {
            let choice: bool = kani::any();
            let id = if choice { id1.clone() } else { id2.clone() };

            let _ = budget.record_resume(id);
        }

        // INV-BQ1: Each instance count must be <= max_per_instance
        for (instance_id, count) in &budget.instance_counts {
            assert!(
                *count <= max_per_instance,
                "Instance {:?} count {} exceeds max_per_instance {}",
                instance_id,
                count,
                max_per_instance
            );
        }
    }

    // =========================================================================
    // INV-BQ2: can_resume is consistent with record_resume
    // =========================================================================

    #[kani::proof]
    fn verify_can_resume_consistency() {
        let max_per_instance = kani::any();
        kani::assume(max_per_instance > 0 && max_per_instance <= 100);
        let max_per_workflow = max_per_instance * 10;

        let mut budget = FairnessBudget::with_limits(max_per_instance, max_per_workflow);
        let id = make_kani_id(1);

        // Record up to max_per_instance resumes
        for _ in 0..max_per_instance {
            let can_before = budget.can_resume(&id);
            let recorded = budget.record_resume(id.clone());
            let can_after = budget.can_resume(&id);

            // If record_resume returned true, can_resume must have been true before
            if recorded {
                assert!(
                    can_before,
                    "record_resume returned true but can_resume was false"
                );
            }

            // After recording, can_resume should be false if we've hit the limit
            if !can_after {
                assert!(
                    !recorded,
                    "can_resume is false but record_resume returned true"
                );
            }
        }
    }

    // =========================================================================
    // INV-BQ3: Reset clears all counts
    // =========================================================================

    #[kani::proof]
    fn verify_reset_clears_counts() {
        let max_per_instance = 5;
        let max_per_workflow = 50;

        let mut budget = FairnessBudget::with_limits(max_per_instance, max_per_workflow);
        let id = make_kani_id(1);

        // Record some resumes
        for _ in 0..3 {
            let _ = budget.record_resume(id.clone());
        }

        assert!(
            !budget.instance_counts.is_empty(),
            "Budget should have counts before reset"
        );

        budget.reset();

        assert!(
            budget.instance_counts.is_empty(),
            "Budget should be empty after reset"
        );
        assert!(
            budget.can_resume(&id),
            "Instance should be resumable after reset"
        );
    }

    // =========================================================================
    // INV-BQ4: Zero max_per_instance means no resumes allowed
    // =========================================================================

    #[kani::proof]
    fn verify_zero_max_prevents_resume() {
        let max_per_instance = 0u32;
        let max_per_workflow = 0u32;

        let budget = FairnessBudget::with_limits(max_per_instance, max_per_workflow);
        let id = make_kani_id(1);

        assert!(
            !budget.can_resume(&id),
            "can_resume must be false when max_per_instance is 0"
        );
    }
}
