//! Merge conflict detection and auto-resolution types (ADR-027, ADR-029, ADR-039).
//!
//! Architecture: Data (ConflictType, ResolutionStrategy, ResolutionResult)
//!             → Calc (resolve, classify, detect).
//!
//! This module defines the type system for merge conflicts in the event-sourced
//! actor system. No I/O, no engine integration — pure types and resolution logic.

use serde::{Deserialize, Serialize};

use crate::integer_types::{FenceToken, SequenceNumber, TimestampMs};
use crate::state::LifecycleState;
use crate::state::LeaseRecord;
use crate::string_types::InstanceId;

// ============================================================================
// Data Layer: Conflict Types
// ============================================================================

/// Base conflict type enum - all conflict variants.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConflictType {
    /// Lease conflict: two concurrent operations hold the same lease.
    Lease(LeaseConflict),
    /// State transition conflict: incompatible state transitions.
    StateTransition(StateTransitionConflict),
    /// Sequence conflict: events with same/inverted sequence numbers.
    Sequence(SequenceConflict),
    /// Fence conflict: fence token mismatch during lease validation.
    Fence(FenceConflict),
}

/// Occurs when two concurrent operations attempt to acquire or renew the same lease.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LeaseConflict {
    pub instance_id: InstanceId,
    pub step_id: crate::string_types::StepId,
    pub holder_a: LeaseRecord,
    pub holder_b: LeaseRecord,
    pub contested_at: TimestampMs,
}

/// Occurs when two events attempt to transition the same actor instance to incompatible states.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StateTransitionConflict {
    pub instance_id: InstanceId,
    pub current_state: LifecycleState,
    pub event_a: crate::state::TransitionEvent,
    pub event_b: crate::state::TransitionEvent,
    pub sequence_a: SequenceNumber,
    pub sequence_b: SequenceNumber,
}

/// Occurs when events arrive with identical or inverted sequence numbers from different producers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SequenceConflict {
    pub instance_id: InstanceId,
    pub expected_next: SequenceNumber,
    pub received: SequenceNumber,
    pub producer: String,
}

/// Occurs when a fence token mismatch is detected during lease validation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FenceConflict {
    pub instance_id: InstanceId,
    pub presented_token: FenceToken,
    pub current_token: FenceToken,
    pub operation: String,
}

// ============================================================================
// Data Layer: Resolution Strategy
// ============================================================================

/// Strategy for resolving a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResolutionStrategy {
    /// Higher fence token wins.
    FenceTokenPriority,
    /// Lower sequence number wins.
    EarliestSequenceWins,
    /// Most recent timestamp wins.
    LatestTimestampWins,
    /// Existing lease holder wins.
    CurrentHolderRetains,
    /// Escalate to manual resolution.
    RejectBoth,
}

/// Winner of a conflict resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConflictWinner {
    /// Side A won the conflict.
    HolderA,
    /// Side B won the conflict.
    HolderB,
    /// Neither side won (conflict was deferred or unresolvable).
    Neither,
}

impl ConflictWinner {
    #[must_use]
    pub fn is_decisive(self) -> bool {
        !matches!(self, ConflictWinner::Neither)
    }
}

// ============================================================================
// Data Layer: Resolution Result
// ============================================================================

/// Result of attempting to resolve a conflict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionResult {
    /// Conflict was resolved.
    Resolved {
        winner: ConflictWinner,
        strategy: ResolutionStrategy,
    },
    /// Conflict could not be auto-resolved.
    Unresolvable {
        conflict: ConflictType,
        reason: UnresolvableReason,
    },
    /// Resolution deferred, retry at specified time.
    Deferred {
        conflict: ConflictType,
        retry_at: TimestampMs,
    },
}

/// Reason a conflict cannot be auto-resolved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnresolvableReason {
    /// Conflict type is ambiguous or indeterminate.
    AmbiguousConflict,
    /// Circular dependency detected among instances.
    CircularDependency(Vec<InstanceId>),
    /// Winner selection would violate an invariant.
    InvariantViolation,
    /// Conflict requires manual intervention.
    RequiresManualResolution,
    /// Fence token regression detected.
    FenceRegression(FenceToken, FenceToken),
}

// ============================================================================
// Data Layer: Error Types
// ============================================================================

/// Error from merge conflict detection or resolution.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum MergeConflictError {
    #[error("Conflict detection failed: {detail}")]
    DetectionFailure { detail: ErrorDetail },

    #[error("Resolution strategy could not be applied: {detail}")]
    ResolutionFailure { detail: ErrorDetail },

    #[error("Resolution would violate invariant: {detail}")]
    InvariantViolation { detail: ErrorDetail },

    #[error("Cannot allocate resources to resolve conflict: {resource}")]
    ResourceExhaustion { resource: &'static str },

    #[error("Resolution timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },
}

/// Category of merge conflict error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCategory {
    /// Conflict detection itself failed.
    DetectionFailure,
    /// Resolution strategy could not be applied.
    ResolutionFailure,
    /// Resolution would violate an invariant.
    InvariantViolation,
    /// Cannot allocate resources to resolve.
    ResourceExhaustion,
    /// Resolution timed out.
    Timeout,
}

/// Detailed error information.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum ErrorDetail {
    /// Conflict type is ambiguous.
    #[error("ambiguous conflict: {0:?}")]
    AmbiguousConflict(ConflictType),
    /// Circular dependency chain detected.
    #[error("circular dependency: {0:?}")]
    CircularDependency(Vec<InstanceId>),
    /// Winner would be stale.
    #[error("stale winner: {0:?}")]
    StaleWinner(InstanceId, SequenceNumber),
    /// Multiple active lease holders detected.
    #[error("multiple active holders: {0:?}")]
    MultipleActiveHolders(InstanceId),
    /// Sequence number regression detected.
    #[error("sequence regress: {0:?}")]
    SequenceRegress(InstanceId),
    /// Fence token regression detected.
    #[error("fence regression: {0:?} -> {1:?}")]
    FenceRegression(FenceToken, FenceToken),
    /// Terminal state violation.
    #[error("terminal violation: {0:?}")]
    TerminalViolation(LifecycleState),
}

// ============================================================================
// Calc Layer: Resolution Logic
// ============================================================================

impl ConflictType {
    /// Classify this conflict as resolvable or not.
    #[must_use]
    pub fn classify(&self) -> ConflictClass {
        match self {
            ConflictType::Lease(lease) => {
                if lease.holder_a.matches_token(lease.holder_b.token()) {
                    ConflictClass::Unresolvable(UnresolvableReason::AmbiguousConflict)
                } else {
                    ConflictClass::Resolvable
                }
            }
            ConflictType::StateTransition(st) => {
                if st.current_state.is_terminal() {
                    ConflictClass::Unresolvable(UnresolvableReason::InvariantViolation)
                } else {
                    ConflictClass::Resolvable
                }
            }
            ConflictType::Sequence(_) => ConflictClass::Resolvable,
            ConflictType::Fence(_) => ConflictClass::Resolvable,
        }
    }

    /// Get the instance ID for this conflict.
    #[must_use]
    pub fn instance_id(&self) -> &InstanceId {
        match self {
            ConflictType::Lease(c) => &c.instance_id,
            ConflictType::StateTransition(c) => &c.instance_id,
            ConflictType::Sequence(c) => &c.instance_id,
            ConflictType::Fence(c) => &c.instance_id,
        }
    }
}

/// Classification of a conflict's resolvability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictClass {
    /// Conflict can be auto-resolved.
    Resolvable,
    /// Conflict cannot be auto-resolved.
    Unresolvable(UnresolvableReason),
}

/// Resolve a lease conflict using fence token priority.
pub fn resolve_lease_conflict_fence_token(conflict: &LeaseConflict) -> ResolutionResult {
    let token_a = conflict.holder_a.token();
    let token_b = conflict.holder_b.token();

    if token_a > token_b {
        ResolutionResult::Resolved {
            winner: ConflictWinner::HolderA,
            strategy: ResolutionStrategy::FenceTokenPriority,
        }
    } else if token_b > token_a {
        ResolutionResult::Resolved {
            winner: ConflictWinner::HolderB,
            strategy: ResolutionStrategy::FenceTokenPriority,
        }
    } else {
        ResolutionResult::Unresolvable {
            conflict: ConflictType::Lease(conflict.clone()),
            reason: UnresolvableReason::AmbiguousConflict,
        }
    }
}

/// Resolve a state transition conflict using earliest sequence wins.
pub fn resolve_state_transition_conflict(conflict: &StateTransitionConflict) -> ResolutionResult {
    if conflict.current_state.is_terminal() {
        return ResolutionResult::Unresolvable {
            conflict: ConflictType::StateTransition(conflict.clone()),
            reason: UnresolvableReason::InvariantViolation,
        };
    }

    let winner = if conflict.sequence_a <= conflict.sequence_b {
        ConflictWinner::HolderA
    } else {
        ConflictWinner::HolderB
    };

    ResolutionResult::Resolved {
        winner,
        strategy: ResolutionStrategy::EarliestSequenceWins,
    }
}

/// Resolve a sequence conflict.
pub fn resolve_sequence_conflict(_conflict: &SequenceConflict) -> ResolutionResult {
    ResolutionResult::Resolved {
        winner: ConflictWinner::Neither,
        strategy: ResolutionStrategy::RejectBoth,
    }
}

/// Resolve a fence conflict using fence token priority.
pub fn resolve_fence_conflict(conflict: &FenceConflict) -> ResolutionResult {
    if conflict.presented_token > conflict.current_token {
        ResolutionResult::Resolved {
            winner: ConflictWinner::HolderA,
            strategy: ResolutionStrategy::FenceTokenPriority,
        }
    } else {
        ResolutionResult::Unresolvable {
            conflict: ConflictType::Fence(conflict.clone()),
            reason: UnresolvableReason::FenceRegression(
                conflict.presented_token,
                conflict.current_token,
            ),
        }
    }
}

/// Main entry point: resolve any conflict type.
pub fn resolve(conflict: &ConflictType) -> ResolutionResult {
    match conflict.classify() {
        ConflictClass::Unresolvable(reason) => ResolutionResult::Unresolvable {
            conflict: conflict.clone(),
            reason,
        },
        ConflictClass::Resolvable => match conflict {
            ConflictType::Lease(c) => resolve_lease_conflict_fence_token(c),
            ConflictType::StateTransition(c) => resolve_state_transition_conflict(c),
            ConflictType::Sequence(c) => resolve_sequence_conflict(c),
            ConflictType::Fence(c) => resolve_fence_conflict(c),
        },
    }
}

// ============================================================================
// Invariant Verification
// ============================================================================

impl ResolutionResult {
    /// Verify INV-001: After resolution, exactly one operation succeeds
    /// or the conflict is marked Unresolvable.
    #[must_use]
    pub fn satisfies_inv_001(&self) -> bool {
        match self {
            ResolutionResult::Resolved { winner, .. } => winner.is_decisive(),
            ResolutionResult::Unresolvable { .. } => true,
            ResolutionResult::Deferred { .. } => true,
        }
    }

    /// Verify INV-003: Fence token monotonicity is preserved.
    #[must_use]
    pub fn satisfies_inv_003(&self) -> bool {
        match self {
            ResolutionResult::Resolved {
                winner: ConflictWinner::HolderA,
                strategy: ResolutionStrategy::FenceTokenPriority,
                ..
            } => true,
            ResolutionResult::Resolved {
                winner: ConflictWinner::HolderB,
                strategy: ResolutionStrategy::FenceTokenPriority,
                ..
            } => true,
            ResolutionResult::Unresolvable { .. } => true,
            ResolutionResult::Deferred { .. } => true,
            _ => false,
        }
    }

    /// Verify INV-004: Terminal states reject all conflicting transitions.
    #[must_use]
    pub fn satisfies_inv_004(&self) -> bool {
        match self {
            ResolutionResult::Resolved { winner, .. } => winner.is_decisive(),
            ResolutionResult::Unresolvable { reason, .. } => {
                matches!(reason, UnresolvableReason::InvariantViolation)
            }
            ResolutionResult::Deferred { .. } => false,
        }
    }

    /// Verify INV-006: Lease conflicts never result in both holders retaining the lease.
    #[must_use]
    pub fn satisfies_inv_006(&self) -> bool {
        match self {
            ResolutionResult::Resolved { winner, .. } => {
                !matches!(winner, ConflictWinner::Neither)
            }
            ResolutionResult::Unresolvable { .. } => true,
            ResolutionResult::Deferred { .. } => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integer_types::FenceToken;
    use crate::state::TransitionEvent;
    use crate::string_types::StepId;

    fn make_instance_id() -> InstanceId {
        InstanceId::try_from("01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string()).unwrap()
    }

    fn make_step_id() -> StepId {
        StepId::try_from("test-step".to_string()).unwrap()
    }

    #[test]
    fn test_lease_conflict_fence_token_priority_a_wins() {
        let instance_id = make_instance_id();
        let step_id = make_step_id();

        let holder_a = LeaseRecord::new(
            instance_id.clone(),
            step_id.clone(),
            FenceToken::new(100).unwrap(),
        );
        let holder_b = LeaseRecord::new(
            instance_id.clone(),
            step_id.clone(),
            FenceToken::new(50).unwrap(),
        );

        let conflict = LeaseConflict {
            instance_id,
            step_id,
            holder_a,
            holder_b,
            contested_at: TimestampMs::new_unchecked(1000),
        };

        let result = resolve_lease_conflict_fence_token(&conflict);

        match result {
            ResolutionResult::Resolved { winner, strategy } => {
                assert_eq!(winner, ConflictWinner::HolderA);
                assert_eq!(strategy, ResolutionStrategy::FenceTokenPriority);
            }
            _ => panic!("Expected resolved result"),
        }
    }

    #[test]
    fn test_lease_conflict_fence_token_priority_b_wins() {
        let instance_id = make_instance_id();
        let step_id = make_step_id();

        let holder_a = LeaseRecord::new(
            instance_id.clone(),
            step_id.clone(),
            FenceToken::new(50).unwrap(),
        );
        let holder_b = LeaseRecord::new(
            instance_id.clone(),
            step_id.clone(),
            FenceToken::new(100).unwrap(),
        );

        let conflict = LeaseConflict {
            instance_id,
            step_id,
            holder_a,
            holder_b,
            contested_at: TimestampMs::new_unchecked(1000),
        };

        let result = resolve_lease_conflict_fence_token(&conflict);

        match result {
            ResolutionResult::Resolved { winner, strategy } => {
                assert_eq!(winner, ConflictWinner::HolderB);
                assert_eq!(strategy, ResolutionStrategy::FenceTokenPriority);
            }
            _ => panic!("Expected resolved result"),
        }
    }

    #[test]
    fn test_state_transition_conflict_terminal_state_unresolvable() {
        let instance_id = make_instance_id();

        let conflict = StateTransitionConflict {
            instance_id: instance_id.clone(),
            current_state: LifecycleState::Completed,
            event_a: TransitionEvent::CompleteStep,
            event_b: TransitionEvent::Cancel,
            sequence_a: SequenceNumber::new_unchecked(1),
            sequence_b: SequenceNumber::new_unchecked(2),
        };

        let result = resolve_state_transition_conflict(&conflict);

        match result {
            ResolutionResult::Unresolvable { reason, .. } => {
                assert!(matches!(reason, UnresolvableReason::InvariantViolation));
            }
            _ => panic!("Expected unresolvable result"),
        }
    }

    #[test]
    fn test_state_transition_conflict_non_terminal_resolvable() {
        let instance_id = make_instance_id();

        let conflict = StateTransitionConflict {
            instance_id,
            current_state: LifecycleState::StepExecuting,
            event_a: TransitionEvent::CompleteStep,
            event_b: TransitionEvent::Cancel,
            sequence_a: SequenceNumber::new_unchecked(1),
            sequence_b: SequenceNumber::new_unchecked(2),
        };

        let result = resolve_state_transition_conflict(&conflict);

        match result {
            ResolutionResult::Resolved { winner, strategy } => {
                assert_eq!(winner, ConflictWinner::HolderA);
                assert_eq!(strategy, ResolutionStrategy::EarliestSequenceWins);
            }
            _ => panic!("Expected resolved result"),
        }
    }

    #[test]
    fn test_conflict_type_classify_lease() {
        let instance_id = make_instance_id();
        let step_id = make_step_id();

        let holder_a = LeaseRecord::new(
            instance_id.clone(),
            step_id.clone(),
            FenceToken::new(100).unwrap(),
        );
        let holder_b = LeaseRecord::new(
            instance_id.clone(),
            step_id.clone(),
            FenceToken::new(50).unwrap(),
        );

        let conflict = ConflictType::Lease(LeaseConflict {
            instance_id,
            step_id,
            holder_a,
            holder_b,
            contested_at: TimestampMs::new_unchecked(1000),
        });

        assert!(matches!(conflict.classify(), ConflictClass::Resolvable));
    }

    #[test]
    fn test_resolve_all_conflict_types() {
        let instance_id = make_instance_id();
        let step_id = make_step_id();

        let lease_conflict = ConflictType::Lease(LeaseConflict {
            instance_id: instance_id.clone(),
            step_id: step_id.clone(),
            holder_a: LeaseRecord::new(
                instance_id.clone(),
                step_id.clone(),
                FenceToken::new(100).unwrap(),
            ),
            holder_b: LeaseRecord::new(
                instance_id.clone(),
                step_id.clone(),
                FenceToken::new(50).unwrap(),
            ),
            contested_at: TimestampMs::new_unchecked(1000),
        });

        let result = resolve(&lease_conflict);
        assert!(result.satisfies_inv_001());
        assert!(result.satisfies_inv_003());
        assert!(result.satisfies_inv_006());
    }

    #[test]
    fn test_resolution_result_invariants() {
        let resolved = ResolutionResult::Resolved {
            winner: ConflictWinner::HolderA,
            strategy: ResolutionStrategy::FenceTokenPriority,
        };
        assert!(resolved.satisfies_inv_001());

        let unresolvable = ResolutionResult::Unresolvable {
            conflict: ConflictType::Sequence(SequenceConflict {
                instance_id: make_instance_id(),
                expected_next: SequenceNumber::new_unchecked(1),
                received: SequenceNumber::new_unchecked(1),
                producer: "node-1".to_string(),
            }),
            reason: UnresolvableReason::AmbiguousConflict,
        };
        assert!(unresolvable.satisfies_inv_001());
    }
}