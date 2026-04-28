//! Validation helpers for state machine invariants.

use crate::state::lifecycle::{LifecycleState, TransitionEvent};

/// Returns true when a transition is forbidden from a terminal state.
///
/// The only exception is `Failed` + `InstanceResumed` which is a recovery path.
pub fn is_terminal_transition(current: LifecycleState, event: TransitionEvent) -> bool {
    current == LifecycleState::Completed
        || current == LifecycleState::Cancelled
        || (current == LifecycleState::Failed && event != TransitionEvent::InstanceResumed)
}
