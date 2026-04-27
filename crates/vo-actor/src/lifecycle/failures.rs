//! Failure scope handling (ADR-042 Section 5).
//!
//! Computes failure outcomes based on current state and failure scope,
//! supporting both epoch-scoped and lineage-scoped failures.

use vo_types::signal::FailureScope;
use vo_types::LineageStatus;

use super::state::ActorLifecycleState;

// =============================================================================
// Failure Scope Handling (ADR-042 Section 5)
// =============================================================================

/// Outcome of a failure transition, combining actor state with lineage status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureOutcome {
    /// Actor failed and lineage remains active (epoch-scoped failure).
    /// A new epoch may be spawned via continue-as-new.
    EpochFailure {
        actor_state: ActorLifecycleState,
        lineage_status: LineageStatus,
    },
    /// Actor failed and lineage is permanently tombstoned (lineage-scoped failure).
    /// No more epochs can be spawned for this lineage.
    LineageFailure {
        actor_state: ActorLifecycleState,
        lineage_status: LineageStatus,
    },
}

impl FailureOutcome {
    /// Returns the actor lifecycle state after the failure.
    #[must_use]
    pub const fn actor_state(&self) -> ActorLifecycleState {
        match self {
            Self::EpochFailure { actor_state, .. } | Self::LineageFailure { actor_state, .. } => {
                *actor_state
            }
        }
    }

    /// Returns the lineage status after the failure.
    #[must_use]
    pub const fn lineage_status(&self) -> LineageStatus {
        match self {
            Self::EpochFailure { lineage_status, .. }
            | Self::LineageFailure { lineage_status, .. } => *lineage_status,
        }
    }

    /// Returns `true` if this was an epoch-scoped failure.
    #[must_use]
    pub const fn is_epoch_failure(&self) -> bool {
        matches!(self, Self::EpochFailure { .. })
    }

    /// Returns `true` if this was a lineage-scoped failure.
    #[must_use]
    pub const fn is_lineage_failure(&self) -> bool {
        matches!(self, Self::LineageFailure { .. })
    }

    /// Returns `true` if the lineage can spawn new epochs after this failure.
    #[must_use]
    pub const fn can_lineage_spawn_epoch(&self) -> bool {
        match self {
            Self::EpochFailure { lineage_status, .. } => lineage_status.is_active(),
            Self::LineageFailure { lineage_status, .. } => lineage_status.is_active(),
        }
    }
}

/// Compute the failure outcome based on current state and failure scope.
///
/// Per ADR-042 Section 5:
/// - Epoch-scoped failures allow retry/continue-as-new within the lineage
/// - Lineage-scoped failures permanently tombstone the lineage
#[must_use]
pub fn compute_failure_outcome(
    _current: ActorLifecycleState,
    scope: FailureScope,
) -> FailureOutcome {
    match scope {
        FailureScope::Epoch => FailureOutcome::EpochFailure {
            actor_state: ActorLifecycleState::Failed,
            lineage_status: LineageStatus::Active,
        },
        FailureScope::Lineage => FailureOutcome::LineageFailure {
            actor_state: ActorLifecycleState::Failed,
            lineage_status: LineageStatus::Tombstoned,
        },
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_failure_outcome_epoch_scope_allows_lineage_continue() {
        let outcome = compute_failure_outcome(ActorLifecycleState::Running, FailureScope::Epoch);
        assert!(outcome.is_epoch_failure());
        assert!(!outcome.is_lineage_failure());
        assert_eq!(outcome.actor_state(), ActorLifecycleState::Failed);
        assert_eq!(outcome.lineage_status(), LineageStatus::Active);
        assert!(outcome.can_lineage_spawn_epoch());
    }

    #[test]
    fn compute_failure_outcome_lineage_scope_tombstones_lineage() {
        let outcome = compute_failure_outcome(ActorLifecycleState::Running, FailureScope::Lineage);
        assert!(!outcome.is_epoch_failure());
        assert!(outcome.is_lineage_failure());
        assert_eq!(outcome.actor_state(), ActorLifecycleState::Failed);
        assert_eq!(outcome.lineage_status(), LineageStatus::Tombstoned);
        assert!(!outcome.can_lineage_spawn_epoch());
    }

    #[test]
    fn failure_outcome_epoch_failure_has_active_lineage() {
        let outcome = FailureOutcome::EpochFailure {
            actor_state: ActorLifecycleState::Failed,
            lineage_status: LineageStatus::Active,
        };
        assert!(outcome.is_epoch_failure());
        assert!(!outcome.is_lineage_failure());
        assert!(outcome.can_lineage_spawn_epoch());
    }

    #[test]
    fn failure_outcome_lineage_failure_blocks_scheduling() {
        let outcome = FailureOutcome::LineageFailure {
            actor_state: ActorLifecycleState::Failed,
            lineage_status: LineageStatus::Tombstoned,
        };
        assert!(!outcome.is_epoch_failure());
        assert!(outcome.is_lineage_failure());
        assert!(!outcome.can_lineage_spawn_epoch());
    }
}
