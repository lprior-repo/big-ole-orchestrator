//! Pure functions for effect state transitions (Calc layer).

use super::types::*;

// ============================================================================
// Calc Layer: Pure Functions
// ============================================================================

impl EffectIntent {
    /// Check if this state is terminal (Committed or RolledBack).
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, EffectIntent::Committed | EffectIntent::RolledBack)
    }

    /// Returns all EffectIntent variants.
    #[must_use]
    pub const fn all_variants() -> &'static [EffectIntent] {
        &[
            EffectIntent::Prepared,
            EffectIntent::Committed,
            EffectIntent::RolledBack,
        ]
    }
}

impl EffectKind {
    /// Returns all EffectKind variants.
    #[must_use]
    pub const fn all_variants() -> &'static [EffectKind] {
        &[
            EffectKind::HttpCall,
            EffectKind::SqlQuery,
            EffectKind::BlobWrite,
        ]
    }
}

impl CompensationPolicy {
    /// Returns all CompensationPolicy variants.
    #[must_use]
    pub const fn all_variants() -> &'static [CompensationPolicy] {
        &[
            CompensationPolicy::None,
            CompensationPolicy::Manual,
            CompensationPolicy::Automatic,
        ]
    }
}

impl EffectTransitionEvent {
    /// Returns all transition event variants.
    #[must_use]
    pub const fn all_variants() -> &'static [EffectTransitionEvent] {
        &[
            EffectTransitionEvent::Commit,
            EffectTransitionEvent::Rollback,
        ]
    }
}

/// Apply a state transition to an EffectIntent.
///
/// # Errors
///
/// Returns `EffectTransitionError::TerminalStateTransition` if the current state
/// is Committed or RolledBack (INV-EFF-002).
/// Returns `EffectTransitionError::InvalidTransition` if the event is not valid
/// for the current state.
pub fn apply_effect_transition(
    current: EffectIntent,
    event: EffectTransitionEvent,
) -> Result<EffectIntent, EffectTransitionError> {
    match (current, event) {
        // Valid transitions (INV-EFF-001): one-directional from Prepared
        (EffectIntent::Prepared, EffectTransitionEvent::Commit) => Ok(EffectIntent::Committed),
        (EffectIntent::Prepared, EffectTransitionEvent::Rollback) => Ok(EffectIntent::RolledBack),

        // Terminal states reject all transitions (INV-EFF-002)
        (EffectIntent::Committed | EffectIntent::RolledBack, _) => {
            Err(EffectTransitionError::TerminalStateTransition)
        }
    }
}
