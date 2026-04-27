//! Compensation lifecycle state machine for saga patterns (ADR-034).
//!
//! This module implements the pure state machine for compensation actions.
//! States: Pending → Executing → Completed/Failed
//!
//! ## Architecture
//!
//! Data (CompensationState, CompensationTransitionEvent) → Calc (apply_transition)
//!
//! This is a pure state machine with no I/O dependencies.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CompensationState {
    Pending,
    Executing,
    Completed,
    Failed,
}

impl CompensationState {
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            CompensationState::Completed | CompensationState::Failed
        )
    }

    pub const fn all_variants() -> &'static [CompensationState] {
        &[
            CompensationState::Pending,
            CompensationState::Executing,
            CompensationState::Completed,
            CompensationState::Failed,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CompensationTransitionEvent {
    Start,
    Complete,
    Fail,
}

impl CompensationTransitionEvent {
    pub const fn all_variants() -> &'static [CompensationTransitionEvent] {
        &[
            CompensationTransitionEvent::Start,
            CompensationTransitionEvent::Complete,
            CompensationTransitionEvent::Fail,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CompensationTransitionError {
    #[error("cannot transition from terminal state {state:?}")]
    TerminalState { state: CompensationState },
    #[error("invalid transition from {from:?} with event {event:?}")]
    InvalidTransition {
        from: CompensationState,
        event: CompensationTransitionEvent,
    },
}

pub const fn apply_transition(
    current: CompensationState,
    event: CompensationTransitionEvent,
) -> Result<CompensationState, CompensationTransitionError> {
    match (current, event) {
        (CompensationState::Pending, CompensationTransitionEvent::Start) => {
            Ok(CompensationState::Executing)
        }
        (CompensationState::Executing, CompensationTransitionEvent::Complete) => {
            Ok(CompensationState::Completed)
        }
        (CompensationState::Executing, CompensationTransitionEvent::Fail) => {
            Ok(CompensationState::Failed)
        }
        (CompensationState::Pending, CompensationTransitionEvent::Fail) => {
            Ok(CompensationState::Failed)
        }
        (CompensationState::Pending, CompensationTransitionEvent::Complete) => {
            Err(CompensationTransitionError::InvalidTransition {
                from: CompensationState::Pending,
                event: CompensationTransitionEvent::Complete,
            })
        }

        // Terminal states reject all transitions
        (CompensationState::Completed, _) => Err(CompensationTransitionError::TerminalState {
            state: CompensationState::Completed,
        }),
        (CompensationState::Failed, _) => Err(CompensationTransitionError::TerminalState {
            state: CompensationState::Failed,
        }),
        (CompensationState::Executing, CompensationTransitionEvent::Start) => {
            Err(CompensationTransitionError::InvalidTransition {
                from: CompensationState::Executing,
                event: CompensationTransitionEvent::Start,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transition_from_pending_to_executing_to_completed() {
        let state = CompensationState::Pending;
        let state = apply_transition(state, CompensationTransitionEvent::Start).unwrap();
        assert_eq!(state, CompensationState::Executing);

        let state = apply_transition(state, CompensationTransitionEvent::Complete).unwrap();
        assert_eq!(state, CompensationState::Completed);
    }

    #[test]
    fn test_transition_from_pending_to_executing_to_failed() {
        let state = CompensationState::Pending;
        let state = apply_transition(state, CompensationTransitionEvent::Start).unwrap();
        assert_eq!(state, CompensationState::Executing);

        let state = apply_transition(state, CompensationTransitionEvent::Fail).unwrap();
        assert_eq!(state, CompensationState::Failed);
    }

    #[test]
    fn test_transition_from_pending_to_failed_directly() {
        let state = CompensationState::Pending;
        let state = apply_transition(state, CompensationTransitionEvent::Fail).unwrap();
        assert_eq!(state, CompensationState::Failed);
    }

    #[test]
    fn test_transition_from_completed_to_executing_returns_error() {
        let state = CompensationState::Completed;
        let result = apply_transition(state, CompensationTransitionEvent::Start);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CompensationTransitionError::TerminalState {
                state: CompensationState::Completed
            }
        ));
    }

    #[test]
    fn test_transition_from_completed_to_complete_returns_error() {
        let state = CompensationState::Completed;
        let result = apply_transition(state, CompensationTransitionEvent::Complete);
        assert!(result.is_err());
    }

    #[test]
    fn test_transition_from_failed_to_start_returns_error() {
        let state = CompensationState::Failed;
        let result = apply_transition(state, CompensationTransitionEvent::Start);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CompensationTransitionError::TerminalState {
                state: CompensationState::Failed
            }
        ));
    }

    #[test]
    fn test_transition_from_pending_to_complete_is_invalid() {
        let state = CompensationState::Pending;
        let result = apply_transition(state, CompensationTransitionEvent::Complete);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CompensationTransitionError::InvalidTransition { .. }
        ));
    }

    #[test]
    fn test_transition_from_executing_to_start_is_invalid() {
        let state = CompensationState::Executing;
        let result = apply_transition(state, CompensationTransitionEvent::Start);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CompensationTransitionError::InvalidTransition { .. }
        ));
    }

    #[test]
    fn test_completed_is_terminal() {
        assert!(CompensationState::Completed.is_terminal());
        assert!(!CompensationState::Pending.is_terminal());
        assert!(!CompensationState::Executing.is_terminal());
        assert!(CompensationState::Failed.is_terminal());
    }

    #[test]
    fn test_all_states_reachable() {
        let mut states_reached = vec![];

        let mut state = CompensationState::Pending;
        states_reached.push(state);
        state = apply_transition(state, CompensationTransitionEvent::Start).unwrap();
        states_reached.push(state);
        state = apply_transition(state, CompensationTransitionEvent::Complete).unwrap();
        states_reached.push(state);

        assert!(states_reached.contains(&CompensationState::Pending));
        assert!(states_reached.contains(&CompensationState::Executing));
        assert!(states_reached.contains(&CompensationState::Completed));
    }

    #[test]
    fn test_serde_roundtrip_pending() {
        let state = CompensationState::Pending;
        let encoded = serde_json::to_string(&state).unwrap();
        let decoded: CompensationState = serde_json::from_str(&encoded).unwrap();
        assert_eq!(state, decoded);
    }

    #[test]
    fn test_serde_roundtrip_executing() {
        let state = CompensationState::Executing;
        let encoded = serde_json::to_string(&state).unwrap();
        let decoded: CompensationState = serde_json::from_str(&encoded).unwrap();
        assert_eq!(state, decoded);
    }

    #[test]
    fn test_serde_roundtrip_completed() {
        let state = CompensationState::Completed;
        let encoded = serde_json::to_string(&state).unwrap();
        let decoded: CompensationState = serde_json::from_str(&encoded).unwrap();
        assert_eq!(state, decoded);
    }

    #[test]
    fn test_serde_roundtrip_failed() {
        let state = CompensationState::Failed;
        let encoded = serde_json::to_string(&state).unwrap();
        let decoded: CompensationState = serde_json::from_str(&encoded).unwrap();
        assert_eq!(state, decoded);
    }
}
