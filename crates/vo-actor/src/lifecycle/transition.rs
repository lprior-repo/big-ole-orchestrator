//! Lifecycle state machine logic: error types, transition computation, and
//! validation.

use vo_types::InstanceId;

use super::state::{ActorLifecycleState, LifecycleTransition};

// =============================================================================
// Actor Lifecycle State Machine
// =============================================================================

/// Errors that can occur during lifecycle transitions.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LifecycleError {
    #[error("invalid transition {attempted:?} from {from}")]
    InvalidTransition {
        from: ActorLifecycleState,
        attempted: LifecycleTransition,
    },
    #[error("child not found: {0}")]
    ChildNotFound(InstanceId),
    #[error("cannot spawn child in state {0}")]
    CannotSpawnChild(ActorLifecycleState),
    #[error("shutdown timeout with {children_remaining} children remaining")]
    ShutdownTimeout { children_remaining: usize },
}

/// Pure calculation function to determine next state.
#[must_use]
pub fn compute_next_state(
    current: ActorLifecycleState,
    transition: LifecycleTransition,
) -> Option<ActorLifecycleState> {
    match (current, transition) {
        (ActorLifecycleState::Pending, LifecycleTransition::Start) => {
            Some(ActorLifecycleState::Running)
        }
        (ActorLifecycleState::Pending, LifecycleTransition::Fail) => {
            Some(ActorLifecycleState::Failed)
        }
        (ActorLifecycleState::Running, LifecycleTransition::Stop) => {
            Some(ActorLifecycleState::Stopping)
        }
        (ActorLifecycleState::Running, LifecycleTransition::Fail) => {
            Some(ActorLifecycleState::Failed)
        }
        (ActorLifecycleState::Stopping, LifecycleTransition::ChildStopped) => {
            Some(ActorLifecycleState::Stopping)
        }
        (ActorLifecycleState::Stopping, LifecycleTransition::AllChildrenStopped) => {
            Some(ActorLifecycleState::Stopped)
        }
        _ => None,
    }
}

/// Check if a transition is valid for the given state.
#[must_use]
pub fn is_valid_transition(current: ActorLifecycleState, transition: LifecycleTransition) -> bool {
    compute_next_state(current, transition).is_some()
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_next_state_pending_start() {
        let next = compute_next_state(ActorLifecycleState::Pending, LifecycleTransition::Start);
        assert_eq!(next, Some(ActorLifecycleState::Running));
    }

    #[test]
    fn compute_next_state_pending_fail() {
        let next = compute_next_state(ActorLifecycleState::Pending, LifecycleTransition::Fail);
        assert_eq!(next, Some(ActorLifecycleState::Failed));
    }

    #[test]
    fn compute_next_state_running_stop() {
        let next = compute_next_state(ActorLifecycleState::Running, LifecycleTransition::Stop);
        assert_eq!(next, Some(ActorLifecycleState::Stopping));
    }

    #[test]
    fn compute_next_state_running_fail() {
        let next = compute_next_state(ActorLifecycleState::Running, LifecycleTransition::Fail);
        assert_eq!(next, Some(ActorLifecycleState::Failed));
    }

    #[test]
    fn compute_next_state_stopping_all_children_stopped() {
        let next = compute_next_state(
            ActorLifecycleState::Stopping,
            LifecycleTransition::AllChildrenStopped,
        );
        assert_eq!(next, Some(ActorLifecycleState::Stopped));
    }

    #[test]
    fn compute_next_state_stopping_child_stopped() {
        let next = compute_next_state(
            ActorLifecycleState::Stopping,
            LifecycleTransition::ChildStopped,
        );
        assert_eq!(next, Some(ActorLifecycleState::Stopping));
    }

    #[test]
    fn compute_next_state_invalid_transition() {
        let next = compute_next_state(ActorLifecycleState::Stopped, LifecycleTransition::Start);
        assert_eq!(next, None);
    }

    #[test]
    fn compute_next_state_terminal_states_reject_all_transitions() {
        let terminal_states = [ActorLifecycleState::Stopped, ActorLifecycleState::Failed];
        let transitions = [
            LifecycleTransition::Start,
            LifecycleTransition::Stop,
            LifecycleTransition::ChildStopped,
            LifecycleTransition::AllChildrenStopped,
            LifecycleTransition::Fail,
        ];

        for state in terminal_states {
            for transition in transitions {
                let next = compute_next_state(state, transition);
                assert_eq!(
                    next, None,
                    "terminal state {state:?} should reject {transition:?}"
                );
            }
        }
    }

    #[test]
    fn compute_next_state_pending_rejects_stop_and_child_transitions() {
        assert_eq!(
            compute_next_state(ActorLifecycleState::Pending, LifecycleTransition::Stop),
            None
        );
        assert_eq!(
            compute_next_state(
                ActorLifecycleState::Pending,
                LifecycleTransition::ChildStopped
            ),
            None
        );
        assert_eq!(
            compute_next_state(
                ActorLifecycleState::Pending,
                LifecycleTransition::AllChildrenStopped
            ),
            None
        );
    }

    #[test]
    fn compute_next_state_running_rejects_start_and_child_transitions() {
        assert_eq!(
            compute_next_state(ActorLifecycleState::Running, LifecycleTransition::Start),
            None
        );
        assert_eq!(
            compute_next_state(
                ActorLifecycleState::Running,
                LifecycleTransition::ChildStopped
            ),
            None
        );
        assert_eq!(
            compute_next_state(
                ActorLifecycleState::Running,
                LifecycleTransition::AllChildrenStopped
            ),
            None
        );
    }

    #[test]
    fn compute_next_state_stopping_rejects_start_and_stop() {
        assert_eq!(
            compute_next_state(ActorLifecycleState::Stopping, LifecycleTransition::Start),
            None
        );
        assert_eq!(
            compute_next_state(ActorLifecycleState::Stopping, LifecycleTransition::Stop),
            None
        );
        assert_eq!(
            compute_next_state(ActorLifecycleState::Stopping, LifecycleTransition::Fail),
            None
        );
    }

    #[test]
    fn is_valid_transition_terminal_states_always_false() {
        for state in [ActorLifecycleState::Stopped, ActorLifecycleState::Failed] {
            for transition in [
                LifecycleTransition::Start,
                LifecycleTransition::Stop,
                LifecycleTransition::ChildStopped,
                LifecycleTransition::AllChildrenStopped,
                LifecycleTransition::Fail,
            ] {
                assert!(
                    !is_valid_transition(state, transition),
                    "{state:?} should reject {transition:?}"
                );
            }
        }
    }

    #[test]
    fn is_valid_transition_returns_correctly() {
        assert!(is_valid_transition(
            ActorLifecycleState::Pending,
            LifecycleTransition::Start
        ));
        assert!(!is_valid_transition(
            ActorLifecycleState::Stopped,
            LifecycleTransition::Start
        ));
    }

    #[test]
    fn lifecycle_error_display() {
        let err = LifecycleError::InvalidTransition {
            from: ActorLifecycleState::Running,
            attempted: LifecycleTransition::Start,
        };
        assert!(format!("{}", err).contains("invalid transition"));

        let err =
            LifecycleError::ChildNotFound(InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap());
        assert!(format!("{}", err).contains("child not found"));

        let err = LifecycleError::CannotSpawnChild(ActorLifecycleState::Stopped);
        assert!(format!("{}", err).contains("cannot spawn child"));

        let err = LifecycleError::ShutdownTimeout {
            children_remaining: 3,
        };
        assert!(format!("{}", err).contains("shutdown timeout"));
    }
}
