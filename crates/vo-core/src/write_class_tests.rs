#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_class_returns_tier_1_when_critical_control_plane() {
        let wc = WriteClass::CriticalControlPlane;
        assert_eq!(wc.tier(), 1);
    }

    #[test]
    fn write_class_returns_tier_2_when_operator_projection() {
        let wc = WriteClass::OperatorProjection;
        assert_eq!(wc.tier(), 2);
    }

    #[test]
    fn write_class_returns_tier_3_when_bulk_blob() {
        let wc = WriteClass::BulkBlob;
        assert_eq!(wc.tier(), 3);
    }

    #[test]
    fn write_class_never_drops_returns_true_when_critical_control_plane() {
        let wc = WriteClass::CriticalControlPlane;
        assert!(wc.never_drops());
    }

    #[test]
    fn write_class_never_drops_returns_false_when_operator_projection() {
        let wc = WriteClass::OperatorProjection;
        assert!(!wc.never_drops());
    }

    #[test]
    fn write_class_never_drops_returns_false_when_bulk_blob() {
        let wc = WriteClass::BulkBlob;
        assert!(!wc.never_drops());
    }

    #[test]
    fn write_class_parses_critical_control_plane_from_str() {
        let result = WriteClass::parse("critical_control_plane");
        assert_eq!(result, Ok(WriteClass::CriticalControlPlane));
    }

    #[test]
    fn write_class_parses_operator_projection_from_str() {
        let result = WriteClass::parse("operator_projection");
        assert_eq!(result, Ok(WriteClass::OperatorProjection));
    }

    #[test]
    fn write_class_parses_bulk_blob_from_str() {
        let result = WriteClass::parse("bulk_blob");
        assert_eq!(result, Ok(WriteClass::BulkBlob));
    }

    #[test]
    fn write_class_returns_unknown_write_class_error_when_parsing_invalid_string() {
        let result = WriteClass::parse("invalid_class_name");
        assert_eq!(
            result,
            Err(Error::UnknownWriteClass("invalid_class_name".to_string()))
        );
    }

    #[test]
    fn write_class_returns_unknown_write_class_error_when_parsing_empty_string() {
        let result = WriteClass::parse("");
        assert_eq!(result, Err(Error::UnknownWriteClass("".to_string())));
    }

    #[test]
    fn write_class_returns_unknown_write_class_error_when_parsing_case_mismatch() {
        let result = WriteClass::parse("CRITICAL_CONTROL_PLANE");
        assert_eq!(
            result,
            Err(Error::UnknownWriteClass(
                "CRITICAL_CONTROL_PLANE".to_string()
            ))
        );
    }

    #[test]
    fn write_class_as_str_returns_critical_control_plane_when_critical_control_plane() {
        let wc = WriteClass::CriticalControlPlane;
        assert_eq!(wc.as_str(), "critical_control_plane");
    }

    #[test]
    fn write_class_as_str_returns_operator_projection_when_operator_projection() {
        let wc = WriteClass::OperatorProjection;
        assert_eq!(wc.as_str(), "operator_projection");
    }

    #[test]
    fn write_class_as_str_returns_bulk_blob_when_bulk_blob() {
        let wc = WriteClass::BulkBlob;
        assert_eq!(wc.as_str(), "bulk_blob");
    }

    #[test]
    fn write_class_returns_serialization_error_when_deserializing_malformed_json() {
        let json = "{ invalid }";
        let result: Result<WriteClass, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn write_class_returns_serialization_error_when_deserializing_truncated_json() {
        let json = "\"critical_cont";
        let result: Result<WriteClass, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn taxonomy_returns_not_initialized_error_when_accessed_before_init() {
        let err = Error::TaxonomyNotInitialized;
        assert_eq!(err.to_string(), "taxonomy not initialized");
    }

    #[test]
    fn write_budget_creates_with_given_limits() {
        let budget = WriteBudget::new(100, 200, 300);
        assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 100);
        assert_eq!(budget.remaining(WriteClass::OperatorProjection), 200);
        assert_eq!(budget.remaining(WriteClass::BulkBlob), 300);
    }

    #[test]
    fn write_budget_creates_with_zero_limits() {
        let budget = WriteBudget::new(0, 0, 0);
        assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 0);
        assert_eq!(budget.remaining(WriteClass::OperatorProjection), 0);
        assert_eq!(budget.remaining(WriteClass::BulkBlob), 0);
    }

    #[test]
    fn write_budget_creates_with_max_limits() {
        let budget = WriteBudget::new(u64::MAX, u64::MAX, u64::MAX);
        assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), u64::MAX);
        assert_eq!(budget.remaining(WriteClass::OperatorProjection), u64::MAX);
        assert_eq!(budget.remaining(WriteClass::BulkBlob), u64::MAX);
    }

    #[test]
    fn write_budget_can_write_returns_true_when_under_limit() {
        let budget = WriteBudget::new(100, 200, 300);
        assert!(budget.can_write(WriteClass::CriticalControlPlane, 50));
    }

    #[test]
    fn write_budget_can_write_returns_true_when_at_exact_limit() {
        let budget = WriteBudget::new(100, 200, 300);
        assert!(budget.can_write(WriteClass::CriticalControlPlane, 100));
    }

    #[test]
    fn write_budget_can_write_returns_false_when_over_limit() {
        let budget = WriteBudget::new(100, 200, 300);
        assert!(!budget.can_write(WriteClass::CriticalControlPlane, 150));
    }

    #[test]
    fn write_budget_can_write_returns_true_when_zero_bytes() {
        let budget = WriteBudget::new(100, 200, 300);
        assert!(budget.can_write(WriteClass::CriticalControlPlane, 0));
    }

    #[test]
    fn write_budget_can_write_returns_false_when_exhausted() {
        let budget = WriteBudget::new(100, 200, 300);
        let _ = budget.reserve(WriteClass::CriticalControlPlane, 100);
        assert!(!budget.can_write(WriteClass::CriticalControlPlane, 1));
    }

    #[test]
    fn write_budget_reserve_deducts_bytes_on_success() {
        let budget = WriteBudget::new(100, 200, 300);
        let result = budget.reserve(WriteClass::CriticalControlPlane, 30);
        assert_eq!(result, Ok(()));
        assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 70);
    }

    #[test]
    fn write_budget_reserve_succeeds_when_at_exact_limit() {
        let budget = WriteBudget::new(100, 200, 300);
        let result = budget.reserve(WriteClass::CriticalControlPlane, 100);
        assert_eq!(result, Ok(()));
        assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 0);
    }

    #[test]
    fn write_budget_reserve_returns_budget_exceeded_when_over_limit() {
        let budget = WriteBudget::new(100, 200, 300);
        let result = budget.reserve(WriteClass::CriticalControlPlane, 150);
        assert_eq!(
            result,
            Err(Error::BudgetExceeded {
                class: WriteClass::CriticalControlPlane,
                requested: 150,
                available: 100,
            })
        );
    }

    #[test]
    fn write_budget_reserve_returns_budget_exceeded_when_exhausted_plus_one() {
        let budget = WriteBudget::new(100, 200, 300);
        let _ = budget.reserve(WriteClass::CriticalControlPlane, 100);
        let result = budget.reserve(WriteClass::CriticalControlPlane, 1);
        assert_eq!(
            result,
            Err(Error::BudgetExceeded {
                class: WriteClass::CriticalControlPlane,
                requested: 1,
                available: 0,
            })
        );
    }

    #[test]
    fn write_budget_reserve_zero_bytes_succeeds() {
        let budget = WriteBudget::new(100, 200, 300);
        let result = budget.reserve(WriteClass::CriticalControlPlane, 0);
        assert_eq!(result, Ok(()));
        assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 100);
    }

    #[test]
    fn write_budget_remaining_returns_correct_initial_values() {
        let budget = WriteBudget::new(100, 200, 300);
        assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 100);
        assert_eq!(budget.remaining(WriteClass::OperatorProjection), 200);
        assert_eq!(budget.remaining(WriteClass::BulkBlob), 300);
    }

    #[test]
    fn write_budget_remaining_returns_zero_after_exhaustion() {
        let budget = WriteBudget::new(100, 200, 300);
        let _ = budget.reserve(WriteClass::CriticalControlPlane, 100);
        assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 0);
    }

    #[test]
    fn write_budget_remaining_unchanged_after_failed_reserve() {
        let budget = WriteBudget::new(100, 200, 300);
        let _ = budget.reserve(WriteClass::CriticalControlPlane, 150);
        assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 100);
    }

    #[test]
    fn error_unknown_write_class_displays_class_name() {
        let err = Error::UnknownWriteClass("test_class".to_string());
        assert_eq!(err.to_string(), "unknown write class: test_class");
    }

    #[test]
    fn error_serialization_error_displays_message() {
        let err = Error::SerializationError("test error".to_string());
        assert_eq!(err.to_string(), "serialization error: test error");
    }

    #[test]
    fn error_taxonomy_not_initialized_displays_message() {
        let err = Error::TaxonomyNotInitialized;
        assert_eq!(err.to_string(), "taxonomy not initialized");
    }

    #[test]
    fn error_budget_exceeded_displays_details() {
        let err = Error::BudgetExceeded {
            class: WriteClass::CriticalControlPlane,
            requested: 150,
            available: 100,
        };
        assert!(err.to_string().contains("budget exceeded"));
        assert!(err.to_string().contains("CriticalControlPlane"));
        assert!(err.to_string().contains("150"));
        assert!(err.to_string().contains("100"));
    }
}
