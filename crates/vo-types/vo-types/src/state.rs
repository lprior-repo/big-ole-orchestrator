//! Domain state types for the vo-engine.
//!
//! Lifecycle state machine with exhaustive transition rules.
#![allow(dead_code)]

use std::hash::Hash;

// ============================================================================
// Semantic Types
// ============================================================================

/// Semantic type: Worker node identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeName(String);

impl NodeName {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Semantic type: Timer identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimerId(u64);

impl TimerId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn inner(&self) -> u64 {
        self.0
    }
}

/// Semantic type: Attempt number (1-indexed)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AttemptNumber(u32);

impl AttemptNumber {
    pub fn new(num: u32) -> Option<Self> {
        if num == 0 {
            None
        } else {
            Some(Self(num))
        }
    }

    pub fn inner(&self) -> u32 {
        self.0
    }
}

// ============================================================================
// Lifecycle State Machine
// ============================================================================

/// Lifecycle state of a bead in the workflow
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LifecycleState {
    /// Initial state: bead is queued, not yet assigned
    Pending,

    /// Decision phase: bead is evaluating which step to execute
    RunningDecision,

    /// Step is scheduled but not yet executing
    StepScheduled,

    /// Step is actively executing
    StepExecuting,

    /// Waiting for external timer/callback
    WaitingForTimer,

    /// Terminal state: bead completed successfully
    Completed,

    /// Terminal state: bead failed
    Failed,

    /// Terminal state: bead was cancelled
    Cancelled,
}

impl LifecycleState {
    /// Get the operational status for a given state
    pub fn get_operational_status(&self) -> OperationalStatus {
        match self {
            LifecycleState::Pending => OperationalStatus::Healthy,
            LifecycleState::RunningDecision => OperationalStatus::Healthy,
            LifecycleState::StepScheduled => OperationalStatus::Healthy,
            LifecycleState::StepExecuting => OperationalStatus::Healthy,
            LifecycleState::WaitingForTimer => OperationalStatus::Healthy,
            LifecycleState::Completed => OperationalStatus::Blocked(BlockedReason::ManualHold),
            LifecycleState::Failed => OperationalStatus::Recovering,
            LifecycleState::Cancelled => OperationalStatus::Blocked(BlockedReason::ManualHold),
        }
    }

    /// Check if a state is terminal
    pub fn is_terminal(&self) -> bool {
        match self {
            LifecycleState::Completed => true,
            LifecycleState::Failed => true,
            LifecycleState::Cancelled => true,
            LifecycleState::Pending => false,
            LifecycleState::RunningDecision => false,
            LifecycleState::StepScheduled => false,
            LifecycleState::StepExecuting => false,
            LifecycleState::WaitingForTimer => false,
        }
    }

    /// Get all valid transitions from a state
    pub fn get_valid_transitions(&self) -> Vec<TransitionEvent> {
        match self {
            LifecycleState::Pending => {
                vec![TransitionEvent::AssignToNode, TransitionEvent::Cancel]
            }
            LifecycleState::RunningDecision => {
                vec![
                    TransitionEvent::StepScheduled,
                    TransitionEvent::Cancel,
                    TransitionEvent::Fail,
                ]
            }
            LifecycleState::StepScheduled => {
                vec![
                    TransitionEvent::ExecuteStep,
                    TransitionEvent::Cancel,
                    TransitionEvent::Fail,
                ]
            }
            LifecycleState::StepExecuting => vec![
                TransitionEvent::WaitForTimer,
                TransitionEvent::CompleteStep,
                TransitionEvent::Cancel,
                TransitionEvent::Fail,
            ],
            LifecycleState::WaitingForTimer => vec![
                TransitionEvent::TimerFired,
                TransitionEvent::TimerExpired,
                TransitionEvent::Cancel,
                TransitionEvent::Fail,
            ],
            LifecycleState::Completed => vec![],
            LifecycleState::Failed => vec![TransitionEvent::InstanceResumed],
            LifecycleState::Cancelled => vec![],
        }
    }
}

/// Operational status of a bead instance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperationalStatus {
    /// Normal operation
    Healthy,

    /// Blocked with specific reason
    Blocked(BlockedReason),

    /// Recovering from failure
    Recovering,
}

/// Reason why a bead is blocked
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockedReason {
    /// Waiting for dependencies
    DependenciesPending,
    /// Resource contention
    ResourceContention,
    /// Manual hold
    ManualHold,
}

/// Transition event that triggers state changes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransitionEvent {
    // From Pending
    AssignToNode,
    Cancel,

    // From RunningDecision
    StepScheduled,
    Fail,

    // From StepScheduled
    ExecuteStep,

    // From StepExecuting
    WaitForTimer,
    CompleteStep,

    // From WaitingForTimer
    TimerFired,
    TimerExpired,

    // From Completed (terminal - no transitions)
    // From Failed (only InstanceResumed valid)
    InstanceResumed,
    // From Cancelled (terminal - no transitions)
}

impl TransitionEvent {
    /// Get all valid TransitionEvent variants for iteration
    pub fn all_variants() -> &'static [TransitionEvent] {
        &[
            TransitionEvent::AssignToNode,
            TransitionEvent::Cancel,
            TransitionEvent::StepScheduled,
            TransitionEvent::Fail,
            TransitionEvent::ExecuteStep,
            TransitionEvent::WaitForTimer,
            TransitionEvent::CompleteStep,
            TransitionEvent::TimerFired,
            TransitionEvent::TimerExpired,
            TransitionEvent::InstanceResumed,
        ]
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// Error returned when a state transition is invalid
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionError {
    /// Attempted transition from a terminal state
    /// INV-001 violation: terminal states reject all transitions
    TerminalStateTransition,

    /// Transition event is not valid for the current state
    /// INV-003 violation: state has no defined transition for this event
    InvalidTransition,
}

impl std::fmt::Display for TransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransitionError::TerminalStateTransition => {
                write!(f, "Cannot transition from terminal state")
            }
            TransitionError::InvalidTransition => {
                write!(f, "Invalid transition for current state")
            }
        }
    }
}

impl std::error::Error for TransitionError {}

// ============================================================================
// Core API
// ============================================================================

/// Apply a transition to the current state
///
/// # Arguments
/// * `current_state` - The current lifecycle state
/// * `event` - The transition event to apply
///
/// # Returns
/// * `Ok(NewState)` - Transition succeeded
/// * `Err(TransitionError)` - Transition rejected
///
/// # Invariants Enforced
/// * INV-001: Terminal states reject all transitions (except InstanceResumed from Failed)
/// * INV-002: No self-loops or cycles
/// * INV-004: Only Failed accepts InstanceResumed
pub fn apply(
    current_state: LifecycleState,
    event: TransitionEvent,
) -> Result<LifecycleState, TransitionError> {
    match (current_state, event) {
        // From Pending
        (LifecycleState::Pending, TransitionEvent::AssignToNode) => {
            Ok(LifecycleState::RunningDecision)
        }
        (LifecycleState::Pending, TransitionEvent::Cancel) => Ok(LifecycleState::Cancelled),
        (LifecycleState::Pending, TransitionEvent::StepScheduled) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::Pending, TransitionEvent::ExecuteStep) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::Pending, TransitionEvent::WaitForTimer) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::Pending, TransitionEvent::CompleteStep) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::Pending, TransitionEvent::TimerFired) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::Pending, TransitionEvent::TimerExpired) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::Pending, TransitionEvent::Fail) => Err(TransitionError::InvalidTransition),
        (LifecycleState::Pending, TransitionEvent::InstanceResumed) => {
            Err(TransitionError::InvalidTransition)
        }

        // From RunningDecision
        (LifecycleState::RunningDecision, TransitionEvent::StepScheduled) => {
            Ok(LifecycleState::StepScheduled)
        }
        (LifecycleState::RunningDecision, TransitionEvent::Cancel) => Ok(LifecycleState::Cancelled),
        (LifecycleState::RunningDecision, TransitionEvent::Fail) => Ok(LifecycleState::Failed),
        (LifecycleState::RunningDecision, TransitionEvent::AssignToNode) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::RunningDecision, TransitionEvent::ExecuteStep) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::RunningDecision, TransitionEvent::WaitForTimer) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::RunningDecision, TransitionEvent::CompleteStep) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::RunningDecision, TransitionEvent::TimerFired) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::RunningDecision, TransitionEvent::TimerExpired) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::RunningDecision, TransitionEvent::InstanceResumed) => {
            Err(TransitionError::InvalidTransition)
        }

        // From StepScheduled
        (LifecycleState::StepScheduled, TransitionEvent::ExecuteStep) => {
            Ok(LifecycleState::StepExecuting)
        }
        (LifecycleState::StepScheduled, TransitionEvent::Cancel) => Ok(LifecycleState::Cancelled),
        (LifecycleState::StepScheduled, TransitionEvent::Fail) => Ok(LifecycleState::Failed),
        (LifecycleState::StepScheduled, TransitionEvent::AssignToNode) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::StepScheduled, TransitionEvent::StepScheduled) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::StepScheduled, TransitionEvent::WaitForTimer) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::StepScheduled, TransitionEvent::CompleteStep) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::StepScheduled, TransitionEvent::TimerFired) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::StepScheduled, TransitionEvent::TimerExpired) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::StepScheduled, TransitionEvent::InstanceResumed) => {
            Err(TransitionError::InvalidTransition)
        }

        // From StepExecuting
        (LifecycleState::StepExecuting, TransitionEvent::WaitForTimer) => {
            Ok(LifecycleState::WaitingForTimer)
        }
        (LifecycleState::StepExecuting, TransitionEvent::CompleteStep) => {
            Ok(LifecycleState::Completed)
        }
        (LifecycleState::StepExecuting, TransitionEvent::Cancel) => Ok(LifecycleState::Cancelled),
        (LifecycleState::StepExecuting, TransitionEvent::Fail) => Ok(LifecycleState::Failed),
        (LifecycleState::StepExecuting, TransitionEvent::AssignToNode) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::StepExecuting, TransitionEvent::StepScheduled) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::StepExecuting, TransitionEvent::ExecuteStep) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::StepExecuting, TransitionEvent::TimerFired) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::StepExecuting, TransitionEvent::TimerExpired) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::StepExecuting, TransitionEvent::InstanceResumed) => {
            Err(TransitionError::InvalidTransition)
        }

        // From WaitingForTimer
        (LifecycleState::WaitingForTimer, TransitionEvent::TimerFired) => {
            Ok(LifecycleState::StepExecuting)
        }
        (LifecycleState::WaitingForTimer, TransitionEvent::TimerExpired) => {
            Ok(LifecycleState::Failed)
        }
        (LifecycleState::WaitingForTimer, TransitionEvent::Cancel) => Ok(LifecycleState::Cancelled),
        (LifecycleState::WaitingForTimer, TransitionEvent::Fail) => Ok(LifecycleState::Failed),
        (LifecycleState::WaitingForTimer, TransitionEvent::AssignToNode) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::WaitingForTimer, TransitionEvent::StepScheduled) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::WaitingForTimer, TransitionEvent::ExecuteStep) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::WaitingForTimer, TransitionEvent::CompleteStep) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::WaitingForTimer, TransitionEvent::WaitForTimer) => {
            Err(TransitionError::InvalidTransition)
        }
        (LifecycleState::WaitingForTimer, TransitionEvent::InstanceResumed) => {
            Err(TransitionError::InvalidTransition)
        }

        // From Completed (terminal - rejects all)
        (LifecycleState::Completed, _) => Err(TransitionError::TerminalStateTransition),

        // From Failed (only InstanceResumed valid)
        (LifecycleState::Failed, TransitionEvent::InstanceResumed) => {
            Ok(LifecycleState::RunningDecision)
        }
        (LifecycleState::Failed, _) => Err(TransitionError::TerminalStateTransition),

        // From Cancelled (terminal - rejects all)
        (LifecycleState::Cancelled, _) => Err(TransitionError::TerminalStateTransition),
    }
}

/// Get the operational status for a given state
pub fn get_operational_status(state: LifecycleState) -> OperationalStatus {
    state.get_operational_status()
}

/// Check if a state is terminal
pub fn is_terminal(state: LifecycleState) -> bool {
    state.is_terminal()
}

/// Get all valid transitions from a state
pub fn get_valid_transitions(state: LifecycleState) -> Vec<TransitionEvent> {
    state.get_valid_transitions()
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Derive Macro Tests
    // ========================================================================

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

    // ========================================================================
    // apply() Happy Path Transitions (17)
    // ========================================================================

    #[test]
    fn apply_returns_running_decision_when_pending_assigned_to_node() {
        let result = apply(LifecycleState::Pending, TransitionEvent::AssignToNode);
        assert_eq!(result, Ok(LifecycleState::RunningDecision));
    }

    #[test]
    fn apply_returns_cancelled_when_pending_cancels() {
        let result = apply(LifecycleState::Pending, TransitionEvent::Cancel);
        assert_eq!(result, Ok(LifecycleState::Cancelled));
    }

    #[test]
    fn apply_returns_step_scheduled_when_running_decision_step_scheduled() {
        let result = apply(
            LifecycleState::RunningDecision,
            TransitionEvent::StepScheduled,
        );
        assert_eq!(result, Ok(LifecycleState::StepScheduled));
    }

    #[test]
    fn apply_returns_cancelled_when_running_decision_cancels() {
        let result = apply(LifecycleState::RunningDecision, TransitionEvent::Cancel);
        assert_eq!(result, Ok(LifecycleState::Cancelled));
    }

    #[test]
    fn apply_returns_failed_when_running_decision_fails() {
        let result = apply(LifecycleState::RunningDecision, TransitionEvent::Fail);
        assert_eq!(result, Ok(LifecycleState::Failed));
    }

    #[test]
    fn apply_returns_step_executing_when_step_scheduled_execute_step() {
        let result = apply(LifecycleState::StepScheduled, TransitionEvent::ExecuteStep);
        assert_eq!(result, Ok(LifecycleState::StepExecuting));
    }

    #[test]
    fn apply_returns_cancelled_when_step_scheduled_cancels() {
        let result = apply(LifecycleState::StepScheduled, TransitionEvent::Cancel);
        assert_eq!(result, Ok(LifecycleState::Cancelled));
    }

    #[test]
    fn apply_returns_failed_when_step_scheduled_fails() {
        let result = apply(LifecycleState::StepScheduled, TransitionEvent::Fail);
        assert_eq!(result, Ok(LifecycleState::Failed));
    }

    #[test]
    fn apply_returns_waiting_for_timer_when_step_executing_wait_for_timer() {
        let result = apply(LifecycleState::StepExecuting, TransitionEvent::WaitForTimer);
        assert_eq!(result, Ok(LifecycleState::WaitingForTimer));
    }

    #[test]
    fn apply_returns_completed_when_step_executing_complete_step() {
        let result = apply(LifecycleState::StepExecuting, TransitionEvent::CompleteStep);
        assert_eq!(result, Ok(LifecycleState::Completed));
    }

    #[test]
    fn apply_returns_cancelled_when_step_executing_cancels() {
        let result = apply(LifecycleState::StepExecuting, TransitionEvent::Cancel);
        assert_eq!(result, Ok(LifecycleState::Cancelled));
    }

    #[test]
    fn apply_returns_failed_when_step_executing_fails() {
        let result = apply(LifecycleState::StepExecuting, TransitionEvent::Fail);
        assert_eq!(result, Ok(LifecycleState::Failed));
    }

    #[test]
    fn apply_returns_step_executing_when_waiting_for_timer_timer_fired() {
        let result = apply(LifecycleState::WaitingForTimer, TransitionEvent::TimerFired);
        assert_eq!(result, Ok(LifecycleState::StepExecuting));
    }

    #[test]
    fn apply_returns_failed_when_waiting_for_timer_timer_expired() {
        let result = apply(
            LifecycleState::WaitingForTimer,
            TransitionEvent::TimerExpired,
        );
        assert_eq!(result, Ok(LifecycleState::Failed));
    }

    #[test]
    fn apply_returns_cancelled_when_waiting_for_timer_cancels() {
        let result = apply(LifecycleState::WaitingForTimer, TransitionEvent::Cancel);
        assert_eq!(result, Ok(LifecycleState::Cancelled));
    }

    #[test]
    fn apply_returns_failed_when_waiting_for_timer_fails() {
        let result = apply(LifecycleState::WaitingForTimer, TransitionEvent::Fail);
        assert_eq!(result, Ok(LifecycleState::Failed));
    }

    #[test]
    fn apply_returns_running_decision_when_failed_instance_resumed() {
        let result = apply(LifecycleState::Failed, TransitionEvent::InstanceResumed);
        assert_eq!(result, Ok(LifecycleState::RunningDecision));
    }

    // ========================================================================
    // apply() Error Behaviors - Terminal State Rejections (20)
    // ========================================================================

    // Completed state rejections (10)
    #[test]
    fn apply_returns_terminal_state_transition_when_completed_receives_assign_to_node() {
        let result = apply(LifecycleState::Completed, TransitionEvent::AssignToNode);
        assert_eq!(result, Err(TransitionError::TerminalStateTransition));
    }

    #[test]
    fn apply_returns_terminal_state_transition_when_completed_receives_cancel() {
        let result = apply(LifecycleState::Completed, TransitionEvent::Cancel);
        assert_eq!(result, Err(TransitionError::TerminalStateTransition));
    }

    #[test]
    fn apply_returns_terminal_state_transition_when_completed_receives_step_scheduled() {
        let result = apply(LifecycleState::Completed, TransitionEvent::StepScheduled);
        assert_eq!(result, Err(TransitionError::TerminalStateTransition));
    }

    #[test]
    fn apply_returns_terminal_state_transition_when_completed_receives_execute_step() {
        let result = apply(LifecycleState::Completed, TransitionEvent::ExecuteStep);
        assert_eq!(result, Err(TransitionError::TerminalStateTransition));
    }

    #[test]
    fn apply_returns_terminal_state_transition_when_completed_receives_wait_for_timer() {
        let result = apply(LifecycleState::Completed, TransitionEvent::WaitForTimer);
        assert_eq!(result, Err(TransitionError::TerminalStateTransition));
    }

    #[test]
    fn apply_returns_terminal_state_transition_when_completed_receives_complete_step() {
        let result = apply(LifecycleState::Completed, TransitionEvent::CompleteStep);
        assert_eq!(result, Err(TransitionError::TerminalStateTransition));
    }

    #[test]
    fn apply_returns_terminal_state_transition_when_completed_receives_timer_fired() {
        let result = apply(LifecycleState::Completed, TransitionEvent::TimerFired);
        assert_eq!(result, Err(TransitionError::TerminalStateTransition));
    }

    #[test]
    fn apply_returns_terminal_state_transition_when_completed_receives_timer_expired() {
        let result = apply(LifecycleState::Completed, TransitionEvent::TimerExpired);
        assert_eq!(result, Err(TransitionError::TerminalStateTransition));
    }

    #[test]
    fn apply_returns_terminal_state_transition_when_completed_receives_fail() {
        let result = apply(LifecycleState::Completed, TransitionEvent::Fail);
        assert_eq!(result, Err(TransitionError::TerminalStateTransition));
    }

    #[test]
    fn apply_returns_terminal_state_transition_when_completed_receives_instance_resumed() {
        let result = apply(LifecycleState::Completed, TransitionEvent::InstanceResumed);
        assert_eq!(result, Err(TransitionError::TerminalStateTransition));
    }

    // Cancelled state rejections (10)
    #[test]
    fn apply_returns_terminal_state_transition_when_cancelled_receives_assign_to_node() {
        let result = apply(LifecycleState::Cancelled, TransitionEvent::AssignToNode);
        assert_eq!(result, Err(TransitionError::TerminalStateTransition));
    }

    #[test]
    fn apply_returns_terminal_state_transition_when_cancelled_receives_cancel() {
        let result = apply(LifecycleState::Cancelled, TransitionEvent::Cancel);
        assert_eq!(result, Err(TransitionError::TerminalStateTransition));
    }

    #[test]
    fn apply_returns_terminal_state_transition_when_cancelled_receives_step_scheduled() {
        let result = apply(LifecycleState::Cancelled, TransitionEvent::StepScheduled);
        assert_eq!(result, Err(TransitionError::TerminalStateTransition));
    }

    #[test]
    fn apply_returns_terminal_state_transition_when_cancelled_receives_execute_step() {
        let result = apply(LifecycleState::Cancelled, TransitionEvent::ExecuteStep);
        assert_eq!(result, Err(TransitionError::TerminalStateTransition));
    }

    #[test]
    fn apply_returns_terminal_state_transition_when_cancelled_receives_wait_for_timer() {
        let result = apply(LifecycleState::Cancelled, TransitionEvent::WaitForTimer);
        assert_eq!(result, Err(TransitionError::TerminalStateTransition));
    }

    #[test]
    fn apply_returns_terminal_state_transition_when_cancelled_receives_complete_step() {
        let result = apply(LifecycleState::Cancelled, TransitionEvent::CompleteStep);
        assert_eq!(result, Err(TransitionError::TerminalStateTransition));
    }

    #[test]
    fn apply_returns_terminal_state_transition_when_cancelled_receives_timer_fired() {
        let result = apply(LifecycleState::Cancelled, TransitionEvent::TimerFired);
        assert_eq!(result, Err(TransitionError::TerminalStateTransition));
    }

    #[test]
    fn apply_returns_terminal_state_transition_when_cancelled_receives_timer_expired() {
        let result = apply(LifecycleState::Cancelled, TransitionEvent::TimerExpired);
        assert_eq!(result, Err(TransitionError::TerminalStateTransition));
    }

    #[test]
    fn apply_returns_terminal_state_transition_when_cancelled_receives_fail() {
        let result = apply(LifecycleState::Cancelled, TransitionEvent::Fail);
        assert_eq!(result, Err(TransitionError::TerminalStateTransition));
    }

    #[test]
    fn apply_returns_terminal_state_transition_when_cancelled_receives_instance_resumed() {
        let result = apply(LifecycleState::Cancelled, TransitionEvent::InstanceResumed);
        assert_eq!(result, Err(TransitionError::TerminalStateTransition));
    }

    // ========================================================================
    // apply() Error Behaviors - InvalidTransition from Non-Terminal States (13)
    // ========================================================================

    // From Pending
    #[test]
    fn apply_returns_invalid_transition_when_pending_receives_step_scheduled() {
        let result = apply(LifecycleState::Pending, TransitionEvent::StepScheduled);
        assert_eq!(result, Err(TransitionError::InvalidTransition));
    }

    #[test]
    fn apply_returns_invalid_transition_when_pending_receives_execute_step() {
        let result = apply(LifecycleState::Pending, TransitionEvent::ExecuteStep);
        assert_eq!(result, Err(TransitionError::InvalidTransition));
    }

    #[test]
    fn apply_returns_invalid_transition_when_pending_receives_timer_fired() {
        let result = apply(LifecycleState::Pending, TransitionEvent::TimerFired);
        assert_eq!(result, Err(TransitionError::InvalidTransition));
    }

    #[test]
    fn apply_returns_invalid_transition_when_pending_receives_instance_resumed() {
        let result = apply(LifecycleState::Pending, TransitionEvent::InstanceResumed);
        assert_eq!(result, Err(TransitionError::InvalidTransition));
    }

    // From RunningDecision
    #[test]
    fn apply_returns_invalid_transition_when_running_decision_receives_execute_step() {
        let result = apply(
            LifecycleState::RunningDecision,
            TransitionEvent::ExecuteStep,
        );
        assert_eq!(result, Err(TransitionError::InvalidTransition));
    }

    #[test]
    fn apply_returns_invalid_transition_when_running_decision_receives_timer_fired() {
        let result = apply(LifecycleState::RunningDecision, TransitionEvent::TimerFired);
        assert_eq!(result, Err(TransitionError::InvalidTransition));
    }

    #[test]
    fn apply_returns_invalid_transition_when_running_decision_receives_instance_resumed() {
        let result = apply(
            LifecycleState::RunningDecision,
            TransitionEvent::InstanceResumed,
        );
        assert_eq!(result, Err(TransitionError::InvalidTransition));
    }

    // From StepScheduled
    #[test]
    fn apply_returns_invalid_transition_when_step_scheduled_receives_assign_to_node() {
        let result = apply(LifecycleState::StepScheduled, TransitionEvent::AssignToNode);
        assert_eq!(result, Err(TransitionError::InvalidTransition));
    }

    #[test]
    fn apply_returns_invalid_transition_when_step_scheduled_receives_timer_fired() {
        let result = apply(LifecycleState::StepScheduled, TransitionEvent::TimerFired);
        assert_eq!(result, Err(TransitionError::InvalidTransition));
    }

    #[test]
    fn apply_returns_invalid_transition_when_step_scheduled_receives_instance_resumed() {
        let result = apply(
            LifecycleState::StepScheduled,
            TransitionEvent::InstanceResumed,
        );
        assert_eq!(result, Err(TransitionError::InvalidTransition));
    }

    // From StepExecuting
    #[test]
    fn apply_returns_invalid_transition_when_step_executing_receives_step_scheduled() {
        let result = apply(
            LifecycleState::StepExecuting,
            TransitionEvent::StepScheduled,
        );
        assert_eq!(result, Err(TransitionError::InvalidTransition));
    }

    #[test]
    fn apply_returns_invalid_transition_when_step_executing_receives_instance_resumed() {
        let result = apply(
            LifecycleState::StepExecuting,
            TransitionEvent::InstanceResumed,
        );
        assert_eq!(result, Err(TransitionError::InvalidTransition));
    }

    // From WaitingForTimer
    #[test]
    fn apply_returns_invalid_transition_when_waiting_for_timer_receives_instance_resumed() {
        let result = apply(
            LifecycleState::WaitingForTimer,
            TransitionEvent::InstanceResumed,
        );
        assert_eq!(result, Err(TransitionError::InvalidTransition));
    }

    // ========================================================================
    // Helper Function Tests
    // ========================================================================

    #[test]
    fn get_operational_status_returns_healthy_for_pending() {
        assert_eq!(
            get_operational_status(LifecycleState::Pending),
            OperationalStatus::Healthy
        );
    }

    #[test]
    fn get_operational_status_returns_healthy_for_running_decision() {
        assert_eq!(
            get_operational_status(LifecycleState::RunningDecision),
            OperationalStatus::Healthy
        );
    }

    #[test]
    fn get_operational_status_returns_healthy_for_step_scheduled() {
        assert_eq!(
            get_operational_status(LifecycleState::StepScheduled),
            OperationalStatus::Healthy
        );
    }

    #[test]
    fn get_operational_status_returns_healthy_for_step_executing() {
        assert_eq!(
            get_operational_status(LifecycleState::StepExecuting),
            OperationalStatus::Healthy
        );
    }

    #[test]
    fn get_operational_status_returns_healthy_for_waiting_for_timer() {
        assert_eq!(
            get_operational_status(LifecycleState::WaitingForTimer),
            OperationalStatus::Healthy
        );
    }

    #[test]
    fn get_operational_status_returns_recovering_for_failed() {
        assert_eq!(
            get_operational_status(LifecycleState::Failed),
            OperationalStatus::Recovering
        );
    }

    #[test]
    fn get_operational_status_returns_blocked_manual_hold_for_completed() {
        assert_eq!(
            get_operational_status(LifecycleState::Completed),
            OperationalStatus::Blocked(BlockedReason::ManualHold)
        );
    }

    #[test]
    fn get_operational_status_returns_blocked_manual_hold_for_cancelled() {
        assert_eq!(
            get_operational_status(LifecycleState::Cancelled),
            OperationalStatus::Blocked(BlockedReason::ManualHold)
        );
    }

    #[test]
    fn is_terminal_returns_true_for_completed() {
        assert!(is_terminal(LifecycleState::Completed));
    }

    #[test]
    fn is_terminal_returns_true_for_failed() {
        assert!(is_terminal(LifecycleState::Failed));
    }

    #[test]
    fn is_terminal_returns_true_for_cancelled() {
        assert!(is_terminal(LifecycleState::Cancelled));
    }

    #[test]
    fn is_terminal_returns_false_for_pending() {
        assert!(!is_terminal(LifecycleState::Pending));
    }

    #[test]
    fn is_terminal_returns_false_for_running_decision() {
        assert!(!is_terminal(LifecycleState::RunningDecision));
    }

    #[test]
    fn is_terminal_returns_false_for_step_scheduled() {
        assert!(!is_terminal(LifecycleState::StepScheduled));
    }

    #[test]
    fn is_terminal_returns_false_for_step_executing() {
        assert!(!is_terminal(LifecycleState::StepExecuting));
    }

    #[test]
    fn is_terminal_returns_false_for_waiting_for_timer() {
        assert!(!is_terminal(LifecycleState::WaitingForTimer));
    }

    #[test]
    fn get_valid_transitions_returns_correct_events_for_pending() {
        let transitions = get_valid_transitions(LifecycleState::Pending);
        assert_eq!(transitions.len(), 2);
        assert!(transitions.contains(&TransitionEvent::AssignToNode));
        assert!(transitions.contains(&TransitionEvent::Cancel));
    }

    #[test]
    fn get_valid_transitions_returns_correct_events_for_running_decision() {
        let transitions = get_valid_transitions(LifecycleState::RunningDecision);
        assert_eq!(transitions.len(), 3);
        assert!(transitions.contains(&TransitionEvent::StepScheduled));
        assert!(transitions.contains(&TransitionEvent::Cancel));
        assert!(transitions.contains(&TransitionEvent::Fail));
    }

    #[test]
    fn get_valid_transitions_returns_correct_events_for_step_scheduled() {
        let transitions = get_valid_transitions(LifecycleState::StepScheduled);
        assert_eq!(transitions.len(), 3);
        assert!(transitions.contains(&TransitionEvent::ExecuteStep));
        assert!(transitions.contains(&TransitionEvent::Cancel));
        assert!(transitions.contains(&TransitionEvent::Fail));
    }

    #[test]
    fn get_valid_transitions_returns_correct_events_for_step_executing() {
        let transitions = get_valid_transitions(LifecycleState::StepExecuting);
        assert_eq!(transitions.len(), 4);
        assert!(transitions.contains(&TransitionEvent::WaitForTimer));
        assert!(transitions.contains(&TransitionEvent::CompleteStep));
        assert!(transitions.contains(&TransitionEvent::Cancel));
        assert!(transitions.contains(&TransitionEvent::Fail));
    }

    #[test]
    fn get_valid_transitions_returns_correct_events_for_waiting_for_timer() {
        let transitions = get_valid_transitions(LifecycleState::WaitingForTimer);
        assert_eq!(transitions.len(), 4);
        assert!(transitions.contains(&TransitionEvent::TimerFired));
        assert!(transitions.contains(&TransitionEvent::TimerExpired));
        assert!(transitions.contains(&TransitionEvent::Cancel));
        assert!(transitions.contains(&TransitionEvent::Fail));
    }

    #[test]
    fn get_valid_transitions_returns_empty_vec_when_state_has_no_valid_transitions() {
        let transitions = get_valid_transitions(LifecycleState::Completed);
        assert_eq!(transitions.len(), 0);
    }

    #[test]
    fn get_valid_transitions_returns_empty_vec_when_cancelled_has_no_valid_transitions() {
        let transitions = get_valid_transitions(LifecycleState::Cancelled);
        assert_eq!(transitions.len(), 0);
    }

    #[test]
    fn get_valid_transitions_returns_instance_resumed_for_failed() {
        let transitions = get_valid_transitions(LifecycleState::Failed);
        assert_eq!(transitions.len(), 1);
        assert!(transitions.contains(&TransitionEvent::InstanceResumed));
    }
}
