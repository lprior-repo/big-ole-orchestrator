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

impl EffectRecord {
    /// Construct a new EffectRecord.
    ///
    /// Returns `None` if `intent_id` is empty (INV-EFF-003).
    #[must_use]
    pub fn new(
        intent_id: String,
        kind: EffectKind,
        params_json: serde_json::Value,
        status: EffectIntent,
        committed_at: Option<crate::types::TimestampMs>,
    ) -> Option<Self> {
        if intent_id.is_empty() {
            return None;
        }
        Some(Self {
            intent_id,
            kind,
            params_json,
            status,
            committed_at,
        })
    }

    #[must_use]
    pub fn intent_id(&self) -> &str {
        &self.intent_id
    }

    #[must_use]
    pub fn kind(&self) -> EffectKind {
        self.kind
    }

    #[must_use]
    pub fn params_json(&self) -> &serde_json::Value {
        &self.params_json
    }

    #[must_use]
    pub fn status(&self) -> EffectIntent {
        self.status
    }

    #[must_use]
    pub fn committed_at(&self) -> Option<&crate::types::TimestampMs> {
        self.committed_at.as_ref()
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
