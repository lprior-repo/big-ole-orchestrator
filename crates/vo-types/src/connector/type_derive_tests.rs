//! Unit tests for ConnectorResult, ReconcileAction, ConnectorTransition, and
//! ConnectorTransitionError derives, serde round-trips, and all_variants.

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod type_derive_tests {
    use super::super::types::*;
    use rstest::rstest;

    // ========================================================================
    // ConnectorResult Derive Tests
    // ========================================================================

    #[test]
    fn connector_result_debug_format_equals_variant_name_for_success() {
        assert_eq!(format!("{:?}", ConnectorResult::Success), "Success");
    }

    #[test]
    fn connector_result_debug_format_equals_variant_name_for_failure() {
        assert_eq!(format!("{:?}", ConnectorResult::Failure), "Failure");
    }

    #[test]
    fn connector_result_debug_format_equals_variant_name_for_ambiguous() {
        assert_eq!(format!("{:?}", ConnectorResult::Ambiguous), "Ambiguous");
    }

    #[test]
    fn connector_result_clone_copy_semantics_preserve_equality() {
        let result = ConnectorResult::Success;
        let copy = result;
        assert_eq!(result, copy);
    }

    #[test]
    fn connector_result_partial_eq_distinguishes_all_variants() {
        assert_eq!(ConnectorResult::Success, ConnectorResult::Success);
        assert_ne!(ConnectorResult::Success, ConnectorResult::Failure);
        assert_ne!(ConnectorResult::Ambiguous, ConnectorResult::Success);
        assert_ne!(ConnectorResult::Ambiguous, ConnectorResult::Failure);
    }

    #[test]
    fn connector_result_hash_consistency_for_equal_variants() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let r1 = ConnectorResult::Ambiguous;
        let r2 = ConnectorResult::Ambiguous;
        let mut h1 = DefaultHasher::new();
        r1.hash(&mut h1);
        let mut h2 = DefaultHasher::new();
        r2.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
    }

    // ========================================================================
    // ConnectorResult Serde Round-Trip (parameterized)
    // ========================================================================

    #[rstest]
    #[case(ConnectorResult::Success, "Success")]
    #[case(ConnectorResult::Failure, "Failure")]
    #[case(ConnectorResult::Ambiguous, "Ambiguous")]
    fn connector_result_serializes_and_deserializes_for_all_variants(
        #[case] variant: ConnectorResult,
        #[case] expected_json: &str,
    ) {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, format!("\"{expected_json}\""));
        let recovered: ConnectorResult = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, variant);
    }

    // ========================================================================
    // ConnectorResult all_variants
    // ========================================================================

    #[test]
    fn connector_result_all_variants_returns_three_variants_in_declaration_order() {
        let variants = ConnectorResult::all_variants();
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0], ConnectorResult::Success);
        assert_eq!(variants[1], ConnectorResult::Failure);
        assert_eq!(variants[2], ConnectorResult::Ambiguous);
    }

    // ========================================================================
    // ReconcileAction Derive Tests
    // ========================================================================

    #[test]
    fn reconcile_action_debug_format_equals_variant_name_for_commit() {
        assert_eq!(format!("{:?}", ReconcileAction::Commit), "Commit");
    }

    #[test]
    fn reconcile_action_debug_format_equals_variant_name_for_rollback() {
        assert_eq!(format!("{:?}", ReconcileAction::Rollback), "Rollback");
    }

    #[test]
    fn reconcile_action_debug_format_equals_variant_name_for_retry() {
        assert_eq!(format!("{:?}", ReconcileAction::Retry), "Retry");
    }

    #[test]
    fn reconcile_action_clone_copy_semantics_preserve_equality() {
        let action = ReconcileAction::Retry;
        let copy = action;
        assert_eq!(action, copy);
    }

    #[test]
    fn reconcile_action_partial_eq_distinguishes_all_variants() {
        assert_eq!(ReconcileAction::Commit, ReconcileAction::Commit);
        assert_ne!(ReconcileAction::Commit, ReconcileAction::Rollback);
        assert_ne!(ReconcileAction::Retry, ReconcileAction::Commit);
    }

    #[test]
    fn reconcile_action_hash_consistency_for_equal_variants() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let a1 = ReconcileAction::Commit;
        let a2 = ReconcileAction::Commit;
        let mut h1 = DefaultHasher::new();
        a1.hash(&mut h1);
        let mut h2 = DefaultHasher::new();
        a2.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
    }

    // ========================================================================
    // ReconcileAction Serde Round-Trip (parameterized)
    // ========================================================================

    #[rstest]
    #[case(ReconcileAction::Commit, "Commit")]
    #[case(ReconcileAction::Rollback, "Rollback")]
    #[case(ReconcileAction::Retry, "Retry")]
    fn reconcile_action_serializes_and_deserializes_for_all_variants(
        #[case] variant: ReconcileAction,
        #[case] expected_json: &str,
    ) {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, format!("\"{expected_json}\""));
        let recovered: ReconcileAction = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, variant);
    }

    // ========================================================================
    // ReconcileAction all_variants
    // ========================================================================

    #[test]
    fn reconcile_action_all_variants_returns_three_variants_in_declaration_order() {
        let variants = ReconcileAction::all_variants();
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0], ReconcileAction::Commit);
        assert_eq!(variants[1], ReconcileAction::Rollback);
        assert_eq!(variants[2], ReconcileAction::Retry);
    }

    // ========================================================================
    // ConnectorTransition all_variants
    // ========================================================================

    #[test]
    fn connector_transition_all_variants_returns_nine_variants_in_declaration_order() {
        let variants = ConnectorTransition::all_variants();
        assert_eq!(variants.len(), 9);
        assert_eq!(variants[0], ConnectorTransition::Prepare);
        assert_eq!(variants[1], ConnectorTransition::Prepared);
        assert_eq!(variants[2], ConnectorTransition::Commit);
        assert_eq!(variants[3], ConnectorTransition::Succeed);
        assert_eq!(variants[4], ConnectorTransition::Fail);
        assert_eq!(variants[5], ConnectorTransition::Ambiguate);
        assert_eq!(variants[6], ConnectorTransition::ReconcileSucceeded);
        assert_eq!(variants[7], ConnectorTransition::ReconcileFailed);
        assert_eq!(variants[8], ConnectorTransition::ReconcileRetry);
    }

    // ========================================================================
    // ConnectorTransitionError Tests
    // ========================================================================

    #[test]
    fn connector_transition_error_terminal_state_transition_displays_correct_message() {
        let err = ConnectorTransitionError::TerminalStateTransition;
        assert_eq!(
            err.to_string(),
            "Cannot transition from terminal connector state"
        );
    }

    #[test]
    fn connector_transition_error_invalid_transition_displays_correct_message() {
        let err = ConnectorTransitionError::InvalidTransition;
        assert_eq!(err.to_string(), "Invalid connector state transition");
    }

    #[test]
    fn connector_transition_error_implements_std_error_error() {
        let err: Box<dyn std::error::Error> =
            Box::new(ConnectorTransitionError::TerminalStateTransition);
        assert_eq!(
            err.to_string(),
            "Cannot transition from terminal connector state"
        );
    }
}
