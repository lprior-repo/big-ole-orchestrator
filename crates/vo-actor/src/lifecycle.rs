//! Hierarchical lifecycle model for vo-actor (ADR-039).
//!
//! Provides lifecycle states, parent-child relationships, and graceful shutdown
//! propagation for the actor hierarchy.
//!
//! # Lifecycle States
//!
//! - `Pending`: Actor created but not yet started
//! - `Running`: Actor is actively processing
//! - `Stopping`: Actor is initiating graceful shutdown
//! - `Stopped`: Actor has completed shutdown
//! - `Failed`: Actor encountered an unrecoverable error
//!
//! # Hierarchy
//!
//! Actors exist in a parent-child relationship where:
//! - Parent actors supervise child actors
//! - Shutdown propagates hierarchically from parent to children
//! - Children must stop before parent can complete shutdown

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use vo_types::InstanceId;

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
            Self::Pending => vec![
                LifecycleTransition::Start,
                LifecycleTransition::Fail,
            ],
            Self::Running => vec![
                LifecycleTransition::Stop,
                LifecycleTransition::Fail,
            ],
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

// =============================================================================
// Parent-Child Relationships
// =============================================================================

/// Information about a child actor.
#[derive(Debug, Clone)]
pub struct ChildInfo {
    pub instance_id: InstanceId,
    pub state: ActorLifecycleState,
    pub added_at: std::time::Instant,
}

/// Parent-child relationship tracker for hierarchical lifecycle.
///
/// Maintains the actor hierarchy and tracks child states for
/// graceful shutdown propagation.
#[derive(Debug)]
pub struct ParentChildRegistry {
    children: Arc<RwLock<HashMap<InstanceId, ChildInfo>>>,
}

impl Default for ParentChildRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentChildRegistry {
    /// Creates a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            children: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Adds a child to this parent's registry.
    pub async fn add_child(&self, instance_id: InstanceId) {
        let mut children = self.children.write().await;
        let info = ChildInfo {
            instance_id: instance_id.clone(),
            state: ActorLifecycleState::Pending,
            added_at: std::time::Instant::now(),
        };
        children.insert(instance_id, info);
    }

    /// Removes a child from this parent's registry.
    pub async fn remove_child(&self, instance_id: &InstanceId) {
        let mut children = self.children.write().await;
        children.remove(instance_id);
    }

    /// Updates a child's lifecycle state.
    pub async fn update_child_state(
        &self,
        instance_id: &InstanceId,
        new_state: ActorLifecycleState,
    ) -> Option<ChildInfo> {
        let mut children = self.children.write().await;
        children.get_mut(instance_id).map(|info| {
            info.state = new_state;
            info.clone()
        })
    }

    /// Gets all children with their current states.
    pub async fn get_children(&self) -> HashMap<InstanceId, ChildInfo> {
        let children = self.children.read().await;
        children.clone()
    }

    /// Gets children in a specific state.
    pub async fn get_children_by_state(
        &self,
        state: ActorLifecycleState,
    ) -> Vec<InstanceId> {
        let children = self.children.read().await;
        children
            .iter()
            .filter(|(_, info)| info.state == state)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Returns true if all children are in terminal state.
    pub async fn all_children_terminal(&self) -> bool {
        let children = self.children.read().await;
        children.values().all(|info| info.state.is_terminal())
    }

    /// Returns true if all children have stopped (including Failed).
    pub async fn all_children_stopped(&self) -> bool {
        let children = self.children.read().await;
        children.values().all(|info| info.state.is_stopping())
    }

    /// Returns the count of non-terminal children.
    pub async fn active_children_count(&self) -> usize {
        let children = self.children.read().await;
        children.values().filter(|info| !info.state.is_terminal()).count()
    }
}

// =============================================================================
// Graceful Shutdown Propagation
// =============================================================================

/// Result of a shutdown propagation operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShutdownResult {
    /// All children shut down successfully.
    Success,
    /// Some children are still running.
    ChildrenRunning {
        pending: usize,
    },
    /// Shutdown timed out.
    Timeout {
        remaining: usize,
    },
}

/// Controls graceful shutdown propagation through the actor hierarchy.
#[derive(Debug)]
pub struct ShutdownPropagator {
    graceful_timeout: std::time::Duration,
    force_kill_timeout: std::time::Duration,
}

impl ShutdownPropagator {
    /// Creates a new propagator with the given timeouts.
    #[must_use]
    pub fn new(graceful_timeout: std::time::Duration, force_kill_timeout: std::time::Duration) -> Self {
        Self {
            graceful_timeout,
            force_kill_timeout,
        }
    }

    /// Default propagator with 30s graceful, 10s force kill.
    #[must_use]
    pub fn default_propagator() -> Self {
        Self {
            graceful_timeout: std::time::Duration::from_secs(30),
            force_kill_timeout: std::time::Duration::from_secs(10),
        }
    }

    /// Returns the graceful shutdown timeout.
    #[must_use]
    pub const fn graceful_timeout(&self) -> std::time::Duration {
        self.graceful_timeout
    }

    /// Returns the force kill timeout.
    #[must_use]
    pub const fn force_kill_timeout(&self) -> std::time::Duration {
        self.force_kill_timeout
    }
}

// =============================================================================
// Actor Lifecycle State Machine
// =============================================================================

/// Errors that can occur during lifecycle transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleError {
    /// Invalid transition attempted.
    InvalidTransition {
        from: ActorLifecycleState,
        attempted: LifecycleTransition,
    },
    /// Child not found in registry.
    ChildNotFound(InstanceId),
    /// Actor cannot accept children in current state.
    CannotSpawnChild(ActorLifecycleState),
    /// Shutdown timeout exceeded.
    ShutdownTimeout {
        children_remaining: usize,
    },
}

impl std::fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTransition { from, attempted } => {
                write!(f, "invalid transition {attempted:?} from {from}")
            }
            Self::ChildNotFound(id) => write!(f, "child not found: {id}"),
            Self::CannotSpawnChild(state) => {
                write!(f, "cannot spawn child in state {state}")
            }
            Self::ShutdownTimeout { children_remaining } => {
                write!(f, "shutdown timeout with {children_remaining} children remaining")
            }
        }
    }
}

impl std::error::Error for LifecycleError {}

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
pub fn is_valid_transition(
    current: ActorLifecycleState,
    transition: LifecycleTransition,
) -> bool {
    compute_next_state(current, transition).is_some()
}

// =============================================================================
// Tests
// =============================================================================

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
        let terminal_states = [
            ActorLifecycleState::Stopped,
            ActorLifecycleState::Failed,
        ];
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
                assert_eq!(next, None, "terminal state {state:?} should reject {transition:?}");
            }
        }
    }

    #[test]
    fn compute_next_state_pending_rejects_stop_and_child_transitions() {
        assert_eq!(compute_next_state(ActorLifecycleState::Pending, LifecycleTransition::Stop), None);
        assert_eq!(compute_next_state(ActorLifecycleState::Pending, LifecycleTransition::ChildStopped), None);
        assert_eq!(compute_next_state(ActorLifecycleState::Pending, LifecycleTransition::AllChildrenStopped), None);
    }

    #[test]
    fn compute_next_state_running_rejects_start_and_child_transitions() {
        assert_eq!(compute_next_state(ActorLifecycleState::Running, LifecycleTransition::Start), None);
        assert_eq!(compute_next_state(ActorLifecycleState::Running, LifecycleTransition::ChildStopped), None);
        assert_eq!(compute_next_state(ActorLifecycleState::Running, LifecycleTransition::AllChildrenStopped), None);
    }

    #[test]
    fn compute_next_state_stopping_rejects_start_and_stop() {
        assert_eq!(compute_next_state(ActorLifecycleState::Stopping, LifecycleTransition::Start), None);
        assert_eq!(compute_next_state(ActorLifecycleState::Stopping, LifecycleTransition::Stop), None);
        assert_eq!(compute_next_state(ActorLifecycleState::Stopping, LifecycleTransition::Fail), None);
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

    #[tokio::test]
    async fn parent_child_registry_add_remove() {
        let registry = ParentChildRegistry::new();
        let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        registry.add_child(id.clone()).await;
        let children = registry.get_children().await;
        assert!(children.contains_key(&id));

        registry.remove_child(&id).await;
        let children = registry.get_children().await;
        assert!(!children.contains_key(&id));
    }

    #[tokio::test]
    async fn parent_child_registry_update_state() {
        let registry = ParentChildRegistry::new();
        let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        registry.add_child(id.clone()).await;
        let info = registry
            .update_child_state(&id, ActorLifecycleState::Running)
            .await;
        assert_eq!(info.unwrap().state, ActorLifecycleState::Running);
    }

    #[tokio::test]
    async fn parent_child_registry_get_children_by_state() {
        let registry = ParentChildRegistry::new();
        let id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();

        registry.add_child(id1.clone()).await;
        registry.add_child(id2.clone()).await;
        registry
            .update_child_state(&id1, ActorLifecycleState::Running)
            .await;

        let running = registry.get_children_by_state(ActorLifecycleState::Running).await;
        assert_eq!(running, vec![id1]);

        let pending = registry.get_children_by_state(ActorLifecycleState::Pending).await;
        assert_eq!(pending, vec![id2]);
    }

    #[tokio::test]
    async fn parent_child_registry_all_children_terminal() {
        let registry = ParentChildRegistry::new();
        let id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();

        registry.add_child(id1.clone()).await;
        registry.add_child(id2.clone()).await;
        registry
            .update_child_state(&id1, ActorLifecycleState::Stopped)
            .await;

        assert!(!registry.all_children_terminal().await);

        registry
            .update_child_state(&id2, ActorLifecycleState::Failed)
            .await;

        assert!(registry.all_children_terminal().await);
    }

    #[tokio::test]
    async fn parent_child_registry_active_count() {
        let registry = ParentChildRegistry::new();
        let id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();

        registry.add_child(id1.clone()).await;
        registry.add_child(id2.clone()).await;
        assert_eq!(registry.active_children_count().await, 2);

        registry
            .update_child_state(&id1, ActorLifecycleState::Stopped)
            .await;
        assert_eq!(registry.active_children_count().await, 1);

        registry
            .update_child_state(&id2, ActorLifecycleState::Failed)
            .await;
        assert_eq!(registry.active_children_count().await, 0);
    }

    #[test]
    fn shutdown_propagator_default() {
        let propagator = ShutdownPropagator::default_propagator();
        assert_eq!(propagator.graceful_timeout(), std::time::Duration::from_secs(30));
        assert_eq!(propagator.force_kill_timeout(), std::time::Duration::from_secs(10));
    }

    #[test]
    fn display_trait_actor_lifecycle_state() {
        assert_eq!(format!("{}", ActorLifecycleState::Pending), "pending");
        assert_eq!(format!("{}", ActorLifecycleState::Running), "running");
        assert_eq!(format!("{}", ActorLifecycleState::Stopping), "stopping");
        assert_eq!(format!("{}", ActorLifecycleState::Stopped), "stopped");
        assert_eq!(format!("{}", ActorLifecycleState::Failed), "failed");
    }

    #[test]
    fn lifecycle_error_display() {
        let err = LifecycleError::InvalidTransition {
            from: ActorLifecycleState::Running,
            attempted: LifecycleTransition::Start,
        };
        assert!(format!("{}", err).contains("invalid transition"));

        let err = LifecycleError::ChildNotFound(
            InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
        );
        assert!(format!("{}", err).contains("child not found"));

        let err = LifecycleError::CannotSpawnChild(ActorLifecycleState::Stopped);
        assert!(format!("{}", err).contains("cannot spawn child"));

        let err = LifecycleError::ShutdownTimeout { children_remaining: 3 };
        assert!(format!("{}", err).contains("shutdown timeout"));
    }
}
