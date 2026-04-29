mod workload_class_boundary {
    use super::*;
    use crate::workload_class::{
        RejectionDetail, WorkloadBudget, WorkloadClass, WorkloadClassError,
    };

    #[test]
    fn parse_whitespace_string_returns_unknown() {
        assert!(WorkloadClass::parse(" ").is_err());
    }

    #[test]
    fn parse_trailing_space_returns_unknown() {
        assert!(WorkloadClass::parse("exact_critical ").is_err());
    }

    #[test]
    fn parse_leading_space_returns_unknown() {
        assert!(WorkloadClass::parse(" exact_critical").is_err());
    }

    #[test]
    fn parse_tab_returns_unknown() {
        assert!(WorkloadClass::parse("\tstandard").is_err());
    }

    #[test]
    fn parse_uppercase_returns_unknown() {
        assert!(WorkloadClass::parse("EXACT_CRITICAL").is_err());
    }

    #[test]
    fn parse_mixed_case_returns_unknown() {
        assert!(WorkloadClass::parse("Standard").is_err());
    }

    #[test]
    fn budget_all_zero_acquire_fails_immediately() {
        let budget = WorkloadBudget::new(0, 0, 0, 0);
        for class in WorkloadClass::all_by_priority() {
            assert!(
                budget.acquire(*class).is_err(),
                "{:?} should fail with zero budget",
                class
            );
            assert!(
                !budget.can_acquire(*class),
                "{:?} should not be acquirable",
                class
            );
        }
    }

    #[test]
    fn budget_error_contains_class_and_amounts() {
        let budget = WorkloadBudget::new(0, 0, 0, 0);
        let err = budget.acquire(WorkloadClass::Standard).unwrap_err();
        assert!(matches!(
            err,
            WorkloadClassError::BudgetExceeded {
                class: WorkloadClass::Standard,
                requested: 1,
                available: 0,
            }
        ));
        let msg = err.to_string();
        assert!(msg.contains("budget exceeded"));
        assert!(msg.contains("Standard"));
    }

    #[test]
    fn budget_release_below_zero_saturates() {
        let budget = WorkloadBudget::new(5, 5, 5, 5);
        budget.release(WorkloadClass::Standard);
        assert_eq!(budget.remaining(WorkloadClass::Standard), 5);
    }

    #[test]
    fn rejection_detail_display_all_reasons() {
        let details = vec![
            RejectionDetail::budget_exhausted(WorkloadClass::ExactCritical),
            RejectionDetail::workflow_cap_exceeded(WorkloadClass::Standard),
            RejectionDetail::global_limit(WorkloadClass::UnsafeBulk),
        ];
        for detail in &details {
            let msg = detail.to_string();
            assert!(!msg.is_empty());
            assert!(msg.contains("rejected"));
        }
    }
}
