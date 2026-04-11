//! Tests for derive macros, operational status, semantic types, and transition errors.

use super::*;
use std::hash::Hash;

#[test]
fn lifecycle_state_debug_format_equals_variant_name() {
    assert_eq!(format!("{:?}", LifecycleState::Pending), "Pending");
    assert_eq!(
        format!("{:?}", LifecycleState::RunningDecision),
        "RunningDecision"
    );
    assert_eq!(
        format!("{:?}", LifecycleState::StepScheduled),
        "StepScheduled"
    );
    assert_eq!(
        format!("{:?}", LifecycleState::StepExecuting),
        "StepExecuting"
    );
    assert_eq!(
        format!("{:?}", LifecycleState::WaitingForTimer),
        "WaitingForTimer"
    );
    assert_eq!(format!("{:?}", LifecycleState::Completed), "Completed");
    assert_eq!(format!("{:?}", LifecycleState::Failed), "Failed");
    assert_eq!(format!("{:?}", LifecycleState::Cancelled), "Cancelled");
}

#[test]
fn lifecycle_state_clone_copy_semantics() {
    let state = LifecycleState::Pending;
    let clone = state; // Copy semantics
    assert_eq!(state, clone);

    let state1 = LifecycleState::RunningDecision;
    let state2 = state1; // Copy semantics
    assert_eq!(state1, state2);
}

#[test]
fn lifecycle_state_partial_eq_and_eq() {
    assert_eq!(LifecycleState::Pending, LifecycleState::Pending);
    assert_ne!(LifecycleState::Pending, LifecycleState::Completed);
    assert_eq!(LifecycleState::Failed, LifecycleState::Failed);
    assert_ne!(LifecycleState::Failed, LifecycleState::Cancelled);
}

#[test]
fn lifecycle_state_hash_consistency() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hasher;

    let state1 = LifecycleState::Pending;
    let state2 = LifecycleState::Pending;

    let mut hasher1 = DefaultHasher::new();
    state1.hash(&mut hasher1);
    let hash1 = hasher1.finish();

    let mut hasher2 = DefaultHasher::new();
    state2.hash(&mut hasher2);
    let hash2 = hasher2.finish();

    assert_eq!(hash1, hash2, "Equal states must have equal hashes");
}

// ========================================================================
// OperationalStatus Tests
// ========================================================================

#[test]
fn operational_status_healthy() {
    assert_eq!(OperationalStatus::Healthy, OperationalStatus::Healthy);
}

#[test]
fn operational_status_blocked_variants() {
    assert_eq!(
        OperationalStatus::Blocked(BlockedReason::DependenciesPending),
        OperationalStatus::Blocked(BlockedReason::DependenciesPending)
    );
    assert_eq!(
        OperationalStatus::Blocked(BlockedReason::ResourceContention),
        OperationalStatus::Blocked(BlockedReason::ResourceContention)
    );
    assert_eq!(
        OperationalStatus::Blocked(BlockedReason::ManualHold),
        OperationalStatus::Blocked(BlockedReason::ManualHold)
    );
}

#[test]
fn operational_status_recovering() {
    assert_eq!(OperationalStatus::Recovering, OperationalStatus::Recovering);
}

#[test]
fn blocked_reason_variants() {
    assert_eq!(
        BlockedReason::DependenciesPending,
        BlockedReason::DependenciesPending
    );
    assert_eq!(
        BlockedReason::ResourceContention,
        BlockedReason::ResourceContention
    );
    assert_eq!(BlockedReason::ManualHold, BlockedReason::ManualHold);
}

// ========================================================================
// TransitionEvent Tests
// ========================================================================

#[test]
fn transition_event_all_variants() {
    let variants = TransitionEvent::all_variants();
    assert_eq!(variants.len(), 10);
}

// ========================================================================
// Semantic Type Tests
// ========================================================================

#[test]
fn node_name_creation() {
    let name = NodeName::new("test-node");
    assert_eq!(name.as_str(), "test-node");
}

#[test]
fn timer_id_creation() {
    let id = TimerId::new(42);
    assert_eq!(id.inner(), 42);
}

#[test]
fn attempt_number_creation_valid() {
    let attempt = AttemptNumber::new(1).unwrap();
    assert_eq!(attempt.inner(), 1);
}

#[test]
fn attempt_number_creation_zero_invalid() {
    assert!(AttemptNumber::new(0).is_none());
}

#[test]
fn attempt_number_creation_positive() {
    let attempt = AttemptNumber::new(5).unwrap();
    assert_eq!(attempt.inner(), 5);
}

// ========================================================================
// TransitionError Tests
// ========================================================================

#[test]
fn transition_error_terminal_state_transition() {
    let err = TransitionError::TerminalStateTransition;
    assert_eq!(err.to_string(), "Cannot transition from terminal state");
}

#[test]
fn transition_error_invalid_transition() {
    let err = TransitionError::InvalidTransition;
    assert_eq!(err.to_string(), "Invalid transition for current state");
}

#[test]
fn transition_error_display() {
    use std::fmt::Write;
    let mut output = String::new();
    write!(output, "{:?}", TransitionError::TerminalStateTransition).unwrap();
    assert_eq!(output, "TerminalStateTransition");
}
