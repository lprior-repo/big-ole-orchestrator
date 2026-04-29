//! Effect intent state machine types (ADR-030).
//!
//! Defines EffectIntent lifecycle, transition events, transition errors,
//! and the pure state machine function `apply_effect_transition`.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum EffectIntent {
    Prepared,
    Committed,
    RolledBack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectTransitionEvent {
    Commit,
    Rollback,
}

impl EffectTransitionEvent {
    #[must_use]
    pub const fn all_variants() -> &'static [EffectTransitionEvent] {
        &[
            EffectTransitionEvent::Commit,
            EffectTransitionEvent::Rollback,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EffectTransitionError {
    #[error("Cannot transition from terminal effect state")]
    TerminalStateTransition,
    #[error("Invalid effect state transition")]
    InvalidTransition,
}

impl EffectIntent {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, EffectIntent::Committed | EffectIntent::RolledBack)
    }

    #[must_use]
    pub const fn all_variants() -> &'static [EffectIntent] {
        &[
            EffectIntent::Prepared,
            EffectIntent::Committed,
            EffectIntent::RolledBack,
        ]
    }
}

pub fn apply_effect_transition(
    current: EffectIntent,
    event: EffectTransitionEvent,
) -> Result<EffectIntent, EffectTransitionError> {
    match (current, event) {
        (EffectIntent::Prepared, EffectTransitionEvent::Commit) => Ok(EffectIntent::Committed),
        (EffectIntent::Prepared, EffectTransitionEvent::Rollback) => Ok(EffectIntent::RolledBack),
        (EffectIntent::Committed | EffectIntent::RolledBack, _) => {
            Err(EffectTransitionError::TerminalStateTransition)
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use rstest::rstest;

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

    #[test]
    fn effectintent_all_variants_returns_three_variants_in_declaration_order() {
        let variants = EffectIntent::all_variants();
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0], EffectIntent::Prepared);
        assert_eq!(variants[1], EffectIntent::Committed);
        assert_eq!(variants[2], EffectIntent::RolledBack);
    }

    #[test]
    fn effect_transition_event_all_variants_returns_two_events() {
        let variants = EffectTransitionEvent::all_variants();
        assert_eq!(variants.len(), 2);
        assert_eq!(variants[0], EffectTransitionEvent::Commit);
        assert_eq!(variants[1], EffectTransitionEvent::Rollback);
    }

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
