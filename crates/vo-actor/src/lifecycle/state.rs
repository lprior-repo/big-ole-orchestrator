//! Lifecycle state types for the hierarchical actor model (ADR-039).
//!
//! Defines the five lifecycle states and their query methods.

// =============================================================================
// Actor Lifecycle State
// =============================================================================

/// Lifecycle states for actors in the hierarchical model (ADR-039).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActorLifecycleState {
    /// Actor is created but not yet started
    Pending,
    /// Actor is actively running
    Running,
    /// Actor is initiating graceful shutdown
    Stopping,
    /// Actor has completed shutdown
    Stopped,
    /// Actor encountered an unrecoverable error
    Failed,
}

impl ActorLifecycleState {
    /// Returns true if this is a terminal state.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Stopped | Self::Failed)
    }

    /// Returns true if shutdown is in progress.
    #[must_use]
    pub const fn is_stopping(&self) -> bool {
        matches!(self, Self::Stopping | Self::Stopped)
    }

    /// Returns true if the actor can accept new children.
    #[must_use]
    pub const fn can_spawn_child(&self) -> bool {
        matches!(self, Self::Pending | Self::Running)
    }

    /// Get valid transitions from this state.
    #[must_use]
    pub fn valid_transitions(&self) -> Vec<LifecycleTransition> {
        match self {
            Self::Pending => vec![LifecycleTransition::Start, LifecycleTransition::Fail],
            Self::Running => vec![LifecycleTransition::Stop, LifecycleTransition::Fail],
            Self::Stopping => vec![
                LifecycleTransition::ChildStopped,
                LifecycleTransition::AllChildrenStopped,
            ],
            Self::Stopped | Self::Failed => vec![],
        }
    }
}

/// Transition events for actor lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LifecycleTransition {
    Start,
    Stop,
    ChildStopped,
    AllChildrenStopped,
    Fail,
}

impl std::fmt::Display for ActorLifecycleState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Running => write!(f, "running"),
            Self::Stopping => write!(f, "stopping"),
            Self::Stopped => write!(f, "stopped"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actor_lifecycle_state_is_terminal() {
        assert!(!ActorLifecycleState::Pending.is_terminal());
        assert!(!ActorLifecycleState::Running.is_terminal());
        assert!(!ActorLifecycleState::Stopping.is_terminal());
        assert!(ActorLifecycleState::Stopped.is_terminal());
        assert!(ActorLifecycleState::Failed.is_terminal());
    }

    #[test]
    fn actor_lifecycle_state_is_stopping() {
        assert!(!ActorLifecycleState::Pending.is_stopping());
        assert!(!ActorLifecycleState::Running.is_stopping());
        assert!(ActorLifecycleState::Stopping.is_stopping());
        assert!(ActorLifecycleState::Stopped.is_stopping());
        assert!(!ActorLifecycleState::Failed.is_stopping());
    }

    #[test]
    fn actor_lifecycle_state_can_spawn_child() {
        assert!(ActorLifecycleState::Pending.can_spawn_child());
        assert!(ActorLifecycleState::Running.can_spawn_child());
        assert!(!ActorLifecycleState::Stopping.can_spawn_child());
        assert!(!ActorLifecycleState::Stopped.can_spawn_child());
        assert!(!ActorLifecycleState::Failed.can_spawn_child());
    }

    #[test]
    fn display_trait_actor_lifecycle_state() {
        assert_eq!(format!("{}", ActorLifecycleState::Pending), "pending");
        assert_eq!(format!("{}", ActorLifecycleState::Running), "running");
        assert_eq!(format!("{}", ActorLifecycleState::Stopping), "stopping");
        assert_eq!(format!("{}", ActorLifecycleState::Stopped), "stopped");
        assert_eq!(format!("{}", ActorLifecycleState::Failed), "failed");
    }
}
