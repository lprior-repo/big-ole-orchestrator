mod write_class_boundary {
    use super::*;
    use crate::write_class::{self, WriteBudget, WriteClass};

    #[test]
    fn parse_substring_prefix_returns_unknown() {
        assert!(WriteClass::parse("critical_control_plane_extra").is_err());
    }

    #[test]
    fn parse_substring_suffix_returns_unknown() {
        assert!(WriteClass::parse("my_critical_control_plane").is_err());
    }

    #[test]
    fn parse_with_newline_returns_unknown() {
        assert!(WriteClass::parse("critical_control_plane\n").is_err());
    }

    #[test]
    fn parse_with_null_byte_returns_unknown() {
        assert!(WriteClass::parse("critical\0_control_plane").is_err());
    }

    #[test]
    fn budget_zero_all_can_write_zero_bytes() {
        let budget = WriteBudget::new(0, 0, 0);
        for class in [
            WriteClass::CriticalControlPlane,
            WriteClass::OperatorProjection,
            WriteClass::BulkBlob,
        ] {
            assert!(
                budget.can_write(class, 0),
                "{:?} should allow zero-byte write with zero budget",
                class
            );
            assert!(
                !budget.can_write(class, 1),
                "{:?} should deny 1-byte write with zero budget",
                class
            );
        }
    }

    #[test]
    fn budget_reserve_one_byte_on_zero_budget_fails() {
        let budget = WriteBudget::new(0, 0, 0);
        let err = budget
            .reserve(WriteClass::CriticalControlPlane, 1)
            .unwrap_err();
        assert!(matches!(
            err,
            write_class::Error::BudgetExceeded {
                class: WriteClass::CriticalControlPlane,
                requested: 1,
                available: 0,
            }
        ));
    }

    #[test]
    fn budget_reserve_exact_max_succeeds() {
        let budget = WriteBudget::new(100, 200, 300);
        assert!(budget
            .reserve(WriteClass::CriticalControlPlane, 100)
            .is_ok());
        assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 0);
    }

    #[test]
    fn budget_reserve_max_plus_one_fails() {
        let budget = WriteBudget::new(100, 200, 300);
        let err = budget
            .reserve(WriteClass::OperatorProjection, 201)
            .unwrap_err();
        assert!(matches!(
            err,
            write_class::Error::BudgetExceeded {
                class: WriteClass::OperatorProjection,
                requested: 201,
                available: 200,
            }
        ));
    }

    #[test]
    fn error_display_all_variants() {
        let errs = vec![
            write_class::Error::UnknownWriteClass("bogus".to_string()),
            write_class::Error::SerializationError("bad json".to_string()),
            write_class::Error::TaxonomyNotInitialized,
            write_class::Error::BudgetExceeded {
                class: WriteClass::BulkBlob,
                requested: 999,
                available: 0,
            },
        ];
        for err in &errs {
            let msg = err.to_string();
            assert!(
                !msg.is_empty(),
                "error display should not be empty: {:?}",
                err
            );
        }
    }
}