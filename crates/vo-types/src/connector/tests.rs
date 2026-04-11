//! Unit tests for connector type derives, serde round-trips, and all_variants.

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::super::types::*;
    use rstest::rstest;

    // ========================================================================
    // ConnectorState Derive Tests
    // ========================================================================

    #[test]
    fn connector_state_debug_format_equals_variant_name_for_idle() {
        assert_eq!(format!("{:?}", ConnectorState::Idle), "Idle");
    }

    #[test]
    fn connector_state_debug_format_equals_variant_name_for_preparing() {
        assert_eq!(format!("{:?}", ConnectorState::Preparing), "Preparing");
    }

    #[test]
    fn connector_state_debug_format_equals_variant_name_for_prepared() {
        assert_eq!(format!("{:?}", ConnectorState::Prepared), "Prepared");
    }

    #[test]
    fn connector_state_debug_format_equals_variant_name_for_executing() {
        assert_eq!(format!("{:?}", ConnectorState::Executing), "Executing");
    }

    #[test]
    fn connector_state_debug_format_equals_variant_name_for_succeeded() {
        assert_eq!(format!("{:?}", ConnectorState::Succeeded), "Succeeded");
    }

    #[test]
    fn connector_state_debug_format_equals_variant_name_for_failed() {
        assert_eq!(format!("{:?}", ConnectorState::Failed), "Failed");
    }

    #[test]
    fn connector_state_debug_format_equals_variant_name_for_ambiguous() {
        assert_eq!(format!("{:?}", ConnectorState::Ambiguous), "Ambiguous");
    }

    #[test]
    fn connector_state_clone_copy_semantics_preserve_equality() {
        let state = ConnectorState::Idle;
        let copy = state;
        assert_eq!(state, copy);

        let state1 = ConnectorState::Ambiguous;
        let state2 = state1;
        assert_eq!(state1, state2);
    }

    #[test]
    fn connector_state_partial_eq_distinguishes_all_variants() {
        assert_eq!(ConnectorState::Idle, ConnectorState::Idle);
        assert_ne!(ConnectorState::Idle, ConnectorState::Preparing);
        assert_ne!(ConnectorState::Succeeded, ConnectorState::Failed);
        assert_ne!(ConnectorState::Ambiguous, ConnectorState::Succeeded);
        assert_ne!(ConnectorState::Ambiguous, ConnectorState::Failed);
    }

    #[test]
    fn connector_state_hash_consistency_for_equal_variants() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let state1 = ConnectorState::Ambiguous;
        let state2 = ConnectorState::Ambiguous;

        let mut h1 = DefaultHasher::new();
        state1.hash(&mut h1);
        let mut h2 = DefaultHasher::new();
        state2.hash(&mut h2);
        assert_eq!(
            h1.finish(),
            h2.finish(),
            "Equal states must have equal hashes"
        );
    }

    // ========================================================================
    // ConnectorState Serde Round-Trip (parameterized)
    // ========================================================================

    #[rstest]
    #[case(ConnectorState::Idle, "Idle")]
    #[case(ConnectorState::Preparing, "Preparing")]
    #[case(ConnectorState::Prepared, "Prepared")]
    #[case(ConnectorState::Executing, "Executing")]
    #[case(ConnectorState::Succeeded, "Succeeded")]
    #[case(ConnectorState::Failed, "Failed")]
    #[case(ConnectorState::Ambiguous, "Ambiguous")]
    fn connector_state_serializes_and_deserializes_for_all_variants(
        #[case] variant: ConnectorState,
        #[case] expected_json: &str,
    ) {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, format!("\"{expected_json}\""));
        let recovered: ConnectorState = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, variant);
    }

    // ========================================================================
    // ConnectorState is_terminal
    // ========================================================================

    #[rstest]
    #[case(ConnectorState::Idle, false)]
    #[case(ConnectorState::Preparing, false)]
    #[case(ConnectorState::Prepared, false)]
    #[case(ConnectorState::Executing, false)]
    #[case(ConnectorState::Succeeded, true)]
    #[case(ConnectorState::Failed, true)]
    #[case(ConnectorState::Ambiguous, false)]
    fn connector_state_is_terminal_returns_correct_value_for_all_variants(
        #[case] state: ConnectorState,
        #[case] expected: bool,
    ) {
        assert_eq!(state.is_terminal(), expected);
    }

    // ========================================================================
    // ConnectorState all_variants
    // ========================================================================

    #[test]
    fn connector_state_all_variants_returns_seven_variants_in_declaration_order() {
        let variants = ConnectorState::all_variants();
        assert_eq!(variants.len(), 7);
        assert_eq!(variants[0], ConnectorState::Idle);
        assert_eq!(variants[1], ConnectorState::Preparing);
        assert_eq!(variants[2], ConnectorState::Prepared);
        assert_eq!(variants[3], ConnectorState::Executing);
        assert_eq!(variants[4], ConnectorState::Succeeded);
        assert_eq!(variants[5], ConnectorState::Failed);
        assert_eq!(variants[6], ConnectorState::Ambiguous);
    }
}
