#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::super::transitions::*;
    use super::super::types::*;
    use rstest::rstest;
    use serde_json::json;

    // ========================================================================
    // EffectIntent Derive Tests
    // ========================================================================

    #[test]
    fn effectintent_debug_format_equals_variant_name() {
        assert_eq!(format!("{:?}", EffectIntent::Prepared), "Prepared");
        assert_eq!(format!("{:?}", EffectIntent::Committed), "Committed");
        assert_eq!(format!("{:?}", EffectIntent::RolledBack), "RolledBack");
    }

    #[test]
    fn effectintent_clone_copy_semantics() {
        let state = EffectIntent::Prepared;
        let copy = state;
        assert_eq!(state, copy);

        let state1 = EffectIntent::Committed;
        let state2 = state1;
        assert_eq!(state1, state2);
    }

    #[test]
    fn effectintent_partial_eq_and_hash() {
        assert_eq!(EffectIntent::Prepared, EffectIntent::Prepared);
        assert_ne!(EffectIntent::Prepared, EffectIntent::Committed);
        assert_ne!(EffectIntent::Committed, EffectIntent::RolledBack);

        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut h1 = DefaultHasher::new();
        EffectIntent::Prepared.hash(&mut h1);
        let mut h2 = DefaultHasher::new();
        EffectIntent::Prepared.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
    }

    // ========================================================================
    // EffectIntent Serde Round-Trip
    // ========================================================================

    #[rstest]
    #[case(EffectIntent::Prepared, "Prepared")]
    #[case(EffectIntent::Committed, "Committed")]
    #[case(EffectIntent::RolledBack, "RolledBack")]
    fn effectintent_serializes_and_deserializes_for_all_variants(
        #[case] variant: EffectIntent,
        #[case] expected_json: &str,
    ) {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, format!("\"{expected_json}\""));
        let recovered: EffectIntent = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, variant);
    }

    // ========================================================================
    // apply_effect_transition — Happy Paths
    // ========================================================================

    #[test]
    fn apply_effect_transition_returns_committed_when_prepared_commit() {
        let result = apply_effect_transition(EffectIntent::Prepared, EffectTransitionEvent::Commit);
        assert_eq!(result, Ok(EffectIntent::Committed));
    }

    #[test]
    fn apply_effect_transition_returns_rolledback_when_prepared_rollback() {
        let result =
            apply_effect_transition(EffectIntent::Prepared, EffectTransitionEvent::Rollback);
        assert_eq!(result, Ok(EffectIntent::RolledBack));
    }

    // ========================================================================
    // apply_effect_transition — Terminal Rejections (INV-EFF-002)
    // ========================================================================

    #[test]
    fn apply_effect_transition_returns_terminal_error_when_committed_commit() {
        let result =
            apply_effect_transition(EffectIntent::Committed, EffectTransitionEvent::Commit);
        assert_eq!(result, Err(EffectTransitionError::TerminalStateTransition));
    }

    #[test]
    fn apply_effect_transition_returns_terminal_error_when_committed_rollback() {
        let result =
            apply_effect_transition(EffectIntent::Committed, EffectTransitionEvent::Rollback);
        assert_eq!(result, Err(EffectTransitionError::TerminalStateTransition));
    }

    #[test]
    fn apply_effect_transition_returns_terminal_error_when_rolledback_commit() {
        let result =
            apply_effect_transition(EffectIntent::RolledBack, EffectTransitionEvent::Commit);
        assert_eq!(result, Err(EffectTransitionError::TerminalStateTransition));
    }

    #[test]
    fn apply_effect_transition_returns_terminal_error_when_rolledback_rollback() {
        let result =
            apply_effect_transition(EffectIntent::RolledBack, EffectTransitionEvent::Rollback);
        assert_eq!(result, Err(EffectTransitionError::TerminalStateTransition));
    }

    // ========================================================================
    // EffectIntent::is_terminal
    // ========================================================================

    #[test]
    fn effectintent_is_terminal_returns_false_when_prepared() {
        assert!(!EffectIntent::Prepared.is_terminal());
    }

    #[test]
    fn effectintent_is_terminal_returns_true_when_committed() {
        assert!(EffectIntent::Committed.is_terminal());
    }

    #[test]
    fn effectintent_is_terminal_returns_true_when_rolledback() {
        assert!(EffectIntent::RolledBack.is_terminal());
    }

    // ========================================================================
    // EffectIntent::all_variants
    // ========================================================================

    #[test]
    fn effectintent_all_variants_returns_three_variants_in_declaration_order() {
        let variants = EffectIntent::all_variants();
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0], EffectIntent::Prepared);
        assert_eq!(variants[1], EffectIntent::Committed);
        assert_eq!(variants[2], EffectIntent::RolledBack);
    }

    // ========================================================================
    // EffectTransitionEvent Tests
    // ========================================================================

    #[test]
    fn effect_transition_event_all_variants_returns_two_events() {
        let variants = EffectTransitionEvent::all_variants();
        assert_eq!(variants.len(), 2);
        assert_eq!(variants[0], EffectTransitionEvent::Commit);
        assert_eq!(variants[1], EffectTransitionEvent::Rollback);
    }

    // ========================================================================
    // EffectKind Derive + Serde Tests
    // ========================================================================

    #[test]
    fn effectkind_debug_format_equals_variant_name() {
        assert_eq!(format!("{:?}", EffectKind::HttpCall), "HttpCall");
        assert_eq!(format!("{:?}", EffectKind::SqlQuery), "SqlQuery");
        assert_eq!(format!("{:?}", EffectKind::BlobWrite), "BlobWrite");
    }

    #[rstest]
    #[case(EffectKind::HttpCall, "HttpCall")]
    #[case(EffectKind::SqlQuery, "SqlQuery")]
    #[case(EffectKind::BlobWrite, "BlobWrite")]
    fn effectkind_serializes_and_deserializes_for_all_variants(
        #[case] variant: EffectKind,
        #[case] expected_json: &str,
    ) {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, format!("\"{expected_json}\""));
        let recovered: EffectKind = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, variant);
    }

    #[test]
    fn effectkind_all_variants_returns_three_variants_in_declaration_order() {
        let variants = EffectKind::all_variants();
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0], EffectKind::HttpCall);
        assert_eq!(variants[1], EffectKind::SqlQuery);
        assert_eq!(variants[2], EffectKind::BlobWrite);
    }

    // ========================================================================
    // CompensationPolicy Derive + Serde Tests
    // ========================================================================

    #[test]
    fn compensationpolicy_debug_format_equals_variant_name() {
        assert_eq!(format!("{:?}", CompensationPolicy::None), "None");
        assert_eq!(format!("{:?}", CompensationPolicy::Manual), "Manual");
        assert_eq!(format!("{:?}", CompensationPolicy::Automatic), "Automatic");
    }

    #[rstest]
    #[case(CompensationPolicy::None, "None")]
    #[case(CompensationPolicy::Manual, "Manual")]
    #[case(CompensationPolicy::Automatic, "Automatic")]
    fn compensationpolicy_serializes_and_deserializes_for_all_variants(
        #[case] variant: CompensationPolicy,
        #[case] expected_json: &str,
    ) {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, format!("\"{expected_json}\""));
        let recovered: CompensationPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, variant);
    }

    #[test]
    fn compensationpolicy_all_variants_returns_three_variants_in_declaration_order() {
        let variants = CompensationPolicy::all_variants();
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0], CompensationPolicy::None);
        assert_eq!(variants[1], CompensationPolicy::Manual);
        assert_eq!(variants[2], CompensationPolicy::Automatic);
    }

    // ========================================================================
    // EffectRecord Construction
    // ========================================================================

    #[test]
    fn effectrecord_returns_some_when_constructed_with_typical_components() {
        let record = EffectRecord::new(
            "fx-123".to_string(),
            EffectKind::HttpCall,
            json!({"url": "https://api.stripe.com/v1/charges"}),
            EffectIntent::Prepared,
            None,
        );
        assert!(record.is_some());
        let r = record.unwrap();
        assert_eq!(r.intent_id(), "fx-123");
        assert_eq!(r.kind(), EffectKind::HttpCall);
        assert_eq!(
            r.params_json(),
            &json!({"url": "https://api.stripe.com/v1/charges"})
        );
        assert_eq!(r.status(), EffectIntent::Prepared);
        assert_eq!(r.committed_at(), None);
    }

    #[test]
    fn effectrecord_returns_some_when_constructed_with_single_char_intent_id() {
        let record = EffectRecord::new(
            "a".to_string(),
            EffectKind::SqlQuery,
            json!({"query": "SELECT 1"}),
            EffectIntent::Prepared,
            None,
        );
        assert!(record.is_some());
        assert_eq!(record.unwrap().intent_id(), "a");
    }

    #[test]
    fn effectrecord_returns_none_when_intent_id_is_empty() {
        let result = EffectRecord::new(
            "".to_string(),
            EffectKind::HttpCall,
            json!({}),
            EffectIntent::Prepared,
            None,
        );
        assert_eq!(result, None);
    }

    #[test]
    fn effectrecord_returns_some_when_constructed_with_committed_status_and_timestamp() {
        let ts = crate::types::TimestampMs(1234);
        let record = EffectRecord::new(
            "fx-456".to_string(),
            EffectKind::BlobWrite,
            json!({"bucket": "my-bucket", "key": "obj"}),
            EffectIntent::Committed,
            Some(ts),
        );
        assert!(record.is_some());
        let r = record.unwrap();
        assert_eq!(r.status(), EffectIntent::Committed);
        assert_eq!(r.committed_at(), Some(&ts));
    }

    #[test]
    fn effectrecord_serializes_and_deserializes_via_json_round_trip() {
        let record = EffectRecord::new(
            "fx-789".to_string(),
            EffectKind::HttpCall,
            json!({"method": "POST", "url": "https://example.com"}),
            EffectIntent::Prepared,
            None,
        );
        let r = record.unwrap();
        let json = serde_json::to_string(&r).unwrap();
        let recovered: EffectRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, r);
    }

    // ========================================================================
    // EffectTransitionError Tests
    // ========================================================================

    #[test]
    fn effect_transition_error_terminal_state_transition_displays_correct_message() {
        let err = EffectTransitionError::TerminalStateTransition;
        assert_eq!(
            err.to_string(),
            "Cannot transition from terminal effect state"
        );
    }

    #[test]
    fn effect_transition_error_invalid_transition_displays_correct_message() {
        let err = EffectTransitionError::InvalidTransition;
        assert_eq!(err.to_string(), "Invalid effect state transition");
    }

    #[test]
    fn effect_transition_error_implements_std_error_error() {
        let err: Box<dyn std::error::Error> =
            Box::new(EffectTransitionError::TerminalStateTransition);
        assert_eq!(
            err.to_string(),
            "Cannot transition from terminal effect state"
        );
    }
}
