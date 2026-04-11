//! State transition engine and LeaseRecord.
//!
//! Pure functions that apply the state machine rules (INV-001 through INV-004)
//! and the fence-token lease record type.

use crate::integer_types::FenceToken;
use crate::string_types::{InstanceId, StepId};

use super::lifecycle::{LifecycleState, OperationalStatus, TransitionEvent};

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
/// # Errors
///
/// Returns `TransitionError::TerminalStateTransition` if the current state is terminal
/// (except `InstanceResumed` from `Failed`), or `TransitionError::InvalidTransition`
/// if the event is not valid for the current state.
///
/// # Invariants Enforced
/// * INV-001: Terminal states reject all transitions (except `InstanceResumed` from Failed)
/// * INV-002: No self-loops or cycles
/// * INV-004: Only Failed accepts `InstanceResumed`
pub fn apply(
    current_state: LifecycleState,
    event: TransitionEvent,
) -> Result<LifecycleState, TransitionError> {
    match (current_state, event) {
        // Valid transitions from non-terminal states
        (LifecycleState::Pending, TransitionEvent::AssignToNode)
        | (LifecycleState::Failed, TransitionEvent::InstanceResumed) => {
            Ok(LifecycleState::RunningDecision)
        }
        (LifecycleState::RunningDecision, TransitionEvent::StepScheduled) => {
            Ok(LifecycleState::StepScheduled)
        }
        (LifecycleState::StepScheduled, TransitionEvent::ExecuteStep)
        | (LifecycleState::WaitingForTimer, TransitionEvent::TimerFired) => {
            Ok(LifecycleState::StepExecuting)
        }
        (LifecycleState::StepExecuting, TransitionEvent::WaitForTimer) => {
            Ok(LifecycleState::WaitingForTimer)
        }
        (LifecycleState::StepExecuting, TransitionEvent::CompleteStep) => {
            Ok(LifecycleState::Completed)
        }
        (LifecycleState::WaitingForTimer, TransitionEvent::TimerExpired) => {
            Ok(LifecycleState::Failed)
        }

        // Cancel from any non-terminal state
        (
            LifecycleState::Pending
            | LifecycleState::RunningDecision
            | LifecycleState::StepScheduled
            | LifecycleState::StepExecuting
            | LifecycleState::WaitingForTimer,
            TransitionEvent::Cancel,
        ) => Ok(LifecycleState::Cancelled),

        // Fail from eligible non-terminal states
        (
            LifecycleState::RunningDecision
            | LifecycleState::StepScheduled
            | LifecycleState::StepExecuting
            | LifecycleState::WaitingForTimer,
            TransitionEvent::Fail,
        ) => Ok(LifecycleState::Failed),

        // Terminal states reject all other transitions
        (LifecycleState::Completed | LifecycleState::Failed | LifecycleState::Cancelled, _) => {
            Err(TransitionError::TerminalStateTransition)
        }

        // All other combinations are invalid
        _ => Err(TransitionError::InvalidTransition),
    }
}

/// Get the operational status for a given state
#[must_use]
pub fn get_operational_status(state: LifecycleState) -> OperationalStatus {
    state.get_operational_status()
}

/// Check if a state is terminal
#[must_use]
pub fn is_terminal(state: LifecycleState) -> bool {
    state.is_terminal()
}

/// Get all valid transitions from a state
#[must_use]
pub fn get_valid_transitions(state: LifecycleState) -> Vec<TransitionEvent> {
    state.get_valid_transitions()
}

// ============================================================================
// LeaseRecord
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct LeaseRecord {
    instance_id: InstanceId,
    step_id: StepId,
    token: FenceToken,
}

impl LeaseRecord {
    #[must_use]
    pub fn new(instance_id: InstanceId, step_id: StepId, token: FenceToken) -> Self {
        Self {
            instance_id,
            step_id,
            token,
        }
    }
    #[must_use]
    pub fn instance_id(&self) -> &InstanceId {
        &self.instance_id
    }
    #[must_use]
    pub fn step_id(&self) -> &StepId {
        &self.step_id
    }
    #[must_use]
    pub fn token(&self) -> &FenceToken {
        &self.token
    }
    #[must_use]
    pub fn matches_token(&self, other: &FenceToken) -> bool {
        &self.token == other
    }
}
