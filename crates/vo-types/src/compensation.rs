//! Compensation types for saga-style workflow rollback (ADR-034).
//!
//! Architecture: Data (CompensationStatus, CompensationTransitionEvent, CompensationRecord)
//!             → Calc (apply_compensation_transition, is_terminal, all_variants).
//!
//! This module defines the type system for compensation actions flowing through the Engine.
//! No I/O, no engine integration — pure types and state machine logic.

// ============================================================================
// Data Layer: Type Definitions
// ============================================================================

/// Lifecycle state of a compensation action (ADR-034).
///
/// Transitions are strictly one-directional:
/// - NotNeeded → terminal (no transitions)
/// - Pending → InProgress | Failed
/// - InProgress → Succeeded | Failed
/// - Succeeded → terminal
/// - Failed → terminal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CompensationStatus {
    /// Effect has CompensationPolicy::None — no compensation possible.
    NotNeeded,
    /// Compensation needed, waiting to start (Manual or Automatic).
    Pending,
    /// Compensation action is executing.
    InProgress,
    /// Compensation completed successfully (terminal).
    Succeeded,
    /// Compensation failed (terminal).
    Failed,
}

/// Event that triggers a compensation status transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompensationTransitionEvent {
    /// Pending → InProgress
    Start,
    /// InProgress → Succeeded
    Succeed,
    /// Pending → Failed | InProgress → Failed
    Fail,
}

/// Error returned when a compensation status transition is invalid.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompensationTransitionError {
    #[error("Cannot transition from terminal compensation state")]
    TerminalStateTransition,
    #[error("Invalid compensation state transition")]
    InvalidTransition,
}

/// Persisted record of a compensation action for a committed effect (ADR-034).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct CompensationRecord {
    effect_id: String,
    policy: crate::effects::CompensationPolicy,
    status: CompensationStatus,
    compensation_effect_id: Option<String>,
    started_at: Option<crate::types::TimestampMs>,
    completed_at: Option<crate::types::TimestampMs>,
}

// ============================================================================
// Calc Layer: Pure Functions
// ============================================================================

impl CompensationStatus {
    /// Check if this state is terminal (NotNeeded, Succeeded, or Failed).
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            CompensationStatus::NotNeeded
                | CompensationStatus::Succeeded
                | CompensationStatus::Failed
        )
    }

    /// Returns all CompensationStatus variants in declaration order.
    #[must_use]
    pub const fn all_variants() -> &'static [CompensationStatus] {
        &[
            CompensationStatus::NotNeeded,
            CompensationStatus::Pending,
            CompensationStatus::InProgress,
            CompensationStatus::Succeeded,
            CompensationStatus::Failed,
        ]
    }
}

impl CompensationTransitionEvent {
    /// Returns all CompensationTransitionEvent variants in declaration order.
    #[must_use]
    pub const fn all_variants() -> &'static [CompensationTransitionEvent] {
        &[
            CompensationTransitionEvent::Start,
            CompensationTransitionEvent::Succeed,
            CompensationTransitionEvent::Fail,
        ]
    }
}

impl CompensationRecord {
    /// Construct a new CompensationRecord.
    ///
    /// Returns `None` if `effect_id` is empty (INV-COMP-003).
    #[must_use]
    pub fn new(
        effect_id: String,
        policy: crate::effects::CompensationPolicy,
        status: CompensationStatus,
        compensation_effect_id: Option<String>,
        started_at: Option<crate::types::TimestampMs>,
        completed_at: Option<crate::types::TimestampMs>,
    ) -> Option<Self> {
        if effect_id.is_empty() {
            return None;
        }
        Some(Self {
            effect_id,
            policy,
            status,
            compensation_effect_id,
            started_at,
            completed_at,
        })
    }

    #[must_use]
    pub fn effect_id(&self) -> &str {
        &self.effect_id
    }

    #[must_use]
    pub fn policy(&self) -> crate::effects::CompensationPolicy {
        self.policy
    }

    #[must_use]
    pub fn status(&self) -> CompensationStatus {
        self.status
    }

    #[must_use]
    pub fn compensation_effect_id(&self) -> Option<&str> {
        self.compensation_effect_id.as_deref()
    }

    #[must_use]
    pub fn started_at(&self) -> Option<&crate::types::TimestampMs> {
        self.started_at.as_ref()
    }

    #[must_use]
    pub fn completed_at(&self) -> Option<&crate::types::TimestampMs> {
        self.completed_at.as_ref()
    }
}

/// Apply a state transition to a CompensationStatus.
///
/// # Errors
///
/// Returns `CompensationTransitionError::TerminalStateTransition` if the current state
/// is NotNeeded, Succeeded, or Failed (INV-COMP-002).
/// Returns `CompensationTransitionError::InvalidTransition` if the event is not valid
/// for the current state.
pub fn apply_compensation_transition(
    current: CompensationStatus,
    event: CompensationTransitionEvent,
) -> Result<CompensationStatus, CompensationTransitionError> {
    match (current, event) {
        // Valid transitions (INV-COMP-001)
        (CompensationStatus::Pending, CompensationTransitionEvent::Start) => {
            Ok(CompensationStatus::InProgress)
        }
        (CompensationStatus::Pending, CompensationTransitionEvent::Fail) => {
            Ok(CompensationStatus::Failed)
        }
        (CompensationStatus::InProgress, CompensationTransitionEvent::Succeed) => {
            Ok(CompensationStatus::Succeeded)
        }
        (CompensationStatus::InProgress, CompensationTransitionEvent::Fail) => {
            Ok(CompensationStatus::Failed)
        }

        // Terminal states reject all transitions (INV-COMP-002)
        (
            CompensationStatus::NotNeeded
            | CompensationStatus::Succeeded
            | CompensationStatus::Failed,
            _,
        ) => Err(CompensationTransitionError::TerminalStateTransition),

        // Invalid transitions for non-terminal states
        (CompensationStatus::Pending, CompensationTransitionEvent::Succeed) => {
            Err(CompensationTransitionError::InvalidTransition)
        }
        (CompensationStatus::InProgress, CompensationTransitionEvent::Start) => {
            Err(CompensationTransitionError::InvalidTransition)
        }
    }
}

// ============================================================================
// Proptest Invariants
// ============================================================================

#[cfg(feature = "proptest")]
#[allow(clippy::unwrap_used)]
mod proptests {
    use super::*;
    use proptest::prop_assert;
    use proptest::prop_assert_eq;

    proptest::proptest! {
        /// INV: Serde round-trip preserves CompensationStatus equality for all variants.
        #[test]
        fn compensationstatus_serde_roundtrip_preserves_equality(
            variant in proptest::sample::select(&[
                CompensationStatus::NotNeeded,
                CompensationStatus::Pending,
                CompensationStatus::InProgress,
                CompensationStatus::Succeeded,
                CompensationStatus::Failed,
            ])
        ) {
            let json = serde_json::to_string(&variant).unwrap();
            let recovered: CompensationStatus = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(variant, recovered);
        }

        /// INV: CompensationRecord field immutability — accessors return construction values.
        #[test]
        fn compensationrecord_accessors_return_construction_values(
            id in "[a-zA-Z0-9_-]{1,100}",
            policy_idx in 0usize..3,
            status_idx in 0usize..5,
        ) {
            let policies = [
                crate::effects::CompensationPolicy::None,
                crate::effects::CompensationPolicy::Manual,
                crate::effects::CompensationPolicy::Automatic,
            ];
            let statuses = [
                CompensationStatus::NotNeeded,
                CompensationStatus::Pending,
                CompensationStatus::InProgress,
                CompensationStatus::Succeeded,
                CompensationStatus::Failed,
            ];
            let policy = policies[policy_idx];
            let status = statuses[status_idx];
            let started = crate::types::TimestampMs(42);

            let record = CompensationRecord::new(
                id.clone(),
                policy,
                status,
                Some("comp-test".to_string()),
                Some(started),
                None,
            );
            prop_assert!(record.is_some());
            let r = record.unwrap();
            prop_assert_eq!(r.effect_id(), id);
            prop_assert_eq!(r.policy(), policy);
            prop_assert_eq!(r.status(), status);
        }

        /// INV: apply_compensation_transition never panics — all 15 combinations.
        #[test]
        fn apply_compensation_transition_never_panics(
            status_idx in 0usize..5,
            event_idx in 0usize..3,
        ) {
            let statuses = CompensationStatus::all_variants();
            let events = CompensationTransitionEvent::all_variants();
            let current = statuses[status_idx];
            let event = events[event_idx];

            // Must not panic — all combinations handled
            let _ = apply_compensation_transition(current, event);
        }

        /// INV: is_terminal returns true for exactly [NotNeeded, Succeeded, Failed].
        #[test]
        fn compensationstatus_is_terminal_consistent_with_all_variants(
            variant in proptest::sample::select(CompensationStatus::all_variants())
        ) {
            let terminal_variants = [
                CompensationStatus::NotNeeded,
                CompensationStatus::Succeeded,
                CompensationStatus::Failed,
            ];
            let expected = terminal_variants.contains(&variant);
            prop_assert_eq!(variant.is_terminal(), expected);
        }
    }
}

// ============================================================================
// Kani Verification Harnesses
// ============================================================================

#[cfg(kani)]
mod verification {
    use super::*;

    /// K-01: Verify apply_compensation_transition exhaustiveness.
    /// All 5×3 = 15 combinations must be covered without panic.
    #[kani::proof]
    fn verify_compensation_transition_exhaustiveness() {
        let state: u8 = kani::any();
        let event: u8 = kani::any();
        kani::assume(state < 5);
        kani::assume(event < 3);

        let current = match state {
            0 => CompensationStatus::NotNeeded,
            1 => CompensationStatus::Pending,
            2 => CompensationStatus::InProgress,
            3 => CompensationStatus::Succeeded,
            _ => CompensationStatus::Failed,
        };
        let evt = match event {
            0 => CompensationTransitionEvent::Start,
            1 => CompensationTransitionEvent::Succeed,
            _ => CompensationTransitionEvent::Fail,
        };

        // Must not panic — all combinations handled
        let _ = apply_compensation_transition(current, evt);
    }

    /// K-02: Verify CompensationRecord::new rejects empty effect_id.
    #[kani::proof]
    fn verify_compensation_record_rejects_empty_effect_id() {
        let effect_id = String::new();
        let result = CompensationRecord::new(
            effect_id,
            crate::effects::CompensationPolicy::Automatic,
            CompensationStatus::Pending,
            None,
            None,
            None,
        );
        assert!(result.is_none());
    }
}
