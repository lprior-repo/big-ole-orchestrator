//! Integration tests for actor lifecycle transitions.
//!
//! These tests verify the hierarchical lifecycle model for vo-actor (ADR-039).
//! Tests cover state transitions, parent-child registry, and shutdown propagation.

use vo_actor::lifecycle::{
    compute_next_state, is_valid_transition, ActorLifecycleState, ChildInfo,
    LifecycleTransition, ParentChildRegistry, ShutdownPropagator, ShutdownResult,
};
use vo_types::InstanceId;

fn test_instance_id() -> InstanceId {
    use ulid::Ulid;
    let ulid = Ulid::new();
    InstanceId::from_bytes(ulid.to_bytes())
}

// =============================================================================
// Lifecycle State Transition Tests
// =============================================================================

#[test]
fn compute_next_state_pending_start() {
    let next =
        compute_next_state(ActorLifecycleState::Pending, LifecycleTransition::Start);
    assert_eq!(next, Some(ActorLifecycleState::Running));
}

#[test]
fn compute_next_state_pending_fail() {
    let next =
        compute_next_state(ActorLifecycleState::Pending, LifecycleTransition::Fail);
    assert_eq!(next, Some(ActorLifecycleState::Failed));
}

#[test]
fn compute_next_state_running_stop() {
    let next =
        compute_next_state(ActorLifecycleState::Running, LifecycleTransition::Stop);
    assert_eq!(next, Some(ActorLifecycleState::Stopping));
}

#[test]
fn compute_next_state_running_fail() {
    let next =
        compute_next_state(ActorLifecycleState::Running, LifecycleTransition::Fail);
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
fn compute_next_state_invalid_from_stopped() {
    let next = compute_next_state(
        ActorLifecycleState::Stopped,
        LifecycleTransition::Start,
    );
    assert_eq!(next, None);
}

#[test]
fn compute_next_state_invalid_from_failed() {
    let next = compute_next_state(
        ActorLifecycleState::Failed,
        LifecycleTransition::Start,
    );
    assert_eq!(next, None);
}

#[test]
fn compute_next_state_invalid_pending_stop() {
    let next = compute_next_state(
        ActorLifecycleState::Pending,
        LifecycleTransition::Stop,
    );
    assert_eq!(next, None);
}

#[test]
fn compute_next_state_invalid_running_start() {
    let next = compute_next_state(
        ActorLifecycleState::Running,
        LifecycleTransition::Start,
    );
    assert_eq!(next, None);
}

#[test]
fn compute_next_state_invalid_stopping_start() {
    let next = compute_next_state(
        ActorLifecycleState::Stopping,
        LifecycleTransition::Start,
    );
    assert_eq!(next, None);
}

// =============================================================================
// Valid Transition Tests
// =============================================================================

#[test]
fn is_valid_transition_pending_start() {
    assert!(is_valid_transition(
        ActorLifecycleState::Pending,
        LifecycleTransition::Start
    ));
}

#[test]
fn is_valid_transition_pending_fail() {
    assert!(is_valid_transition(
        ActorLifecycleState::Pending,
        LifecycleTransition::Fail
    ));
}

#[test]
fn is_valid_transition_running_stop() {
    assert!(is_valid_transition(
        ActorLifecycleState::Running,
        LifecycleTransition::Stop
    ));
}

#[test]
fn is_valid_transition_running_fail() {
    assert!(is_valid_transition(
        ActorLifecycleState::Running,
        LifecycleTransition::Fail
    ));
}

#[test]
fn is_valid_transition_stopping_all_children_stopped() {
    assert!(is_valid_transition(
        ActorLifecycleState::Stopping,
        LifecycleTransition::AllChildrenStopped
    ));
}

#[test]
fn is_valid_transition_invalid_from_stopped() {
    assert!(!is_valid_transition(
        ActorLifecycleState::Stopped,
        LifecycleTransition::Start
    ));
}

#[test]
fn is_valid_transition_invalid_from_failed() {
    assert!(!is_valid_transition(
        ActorLifecycleState::Failed,
        LifecycleTransition::Start
    ));
}

#[test]
fn is_valid_transition_invalid_pending_stop() {
    assert!(!is_valid_transition(
        ActorLifecycleState::Pending,
        LifecycleTransition::Stop
    ));
}

// =============================================================================
// Lifecycle State Properties Tests
// =============================================================================

#[test]
fn actor_lifecycle_state_is_terminal_stopped() {
    assert!(ActorLifecycleState::Stopped.is_terminal());
}

#[test]
fn actor_lifecycle_state_is_terminal_failed() {
    assert!(ActorLifecycleState::Failed.is_terminal());
}

#[test]
fn actor_lifecycle_state_is_not_terminal_pending() {
    assert!(!ActorLifecycleState::Pending.is_terminal());
}

#[test]
fn actor_lifecycle_state_is_not_terminal_running() {
    assert!(!ActorLifecycleState::Running.is_terminal());
}

#[test]
fn actor_lifecycle_state_is_not_terminal_stopping() {
    assert!(!ActorLifecycleState::Stopping.is_terminal());
}

#[test]
fn actor_lifecycle_state_is_stopping_stopping() {
    assert!(ActorLifecycleState::Stopping.is_stopping());
}

#[test]
fn actor_lifecycle_state_is_stopping_stopped() {
    assert!(ActorLifecycleState::Stopped.is_stopping());
}

#[test]
fn actor_lifecycle_state_is_not_stopping_running() {
    assert!(!ActorLifecycleState::Running.is_stopping());
}

#[test]
fn actor_lifecycle_state_is_not_stopping_failed() {
    assert!(!ActorLifecycleState::Failed.is_stopping());
}

#[test]
fn actor_lifecycle_state_can_spawn_child_pending() {
    assert!(ActorLifecycleState::Pending.can_spawn_child());
}

#[test]
fn actor_lifecycle_state_can_spawn_child_running() {
    assert!(ActorLifecycleState::Running.can_spawn_child());
}

#[test]
fn actor_lifecycle_state_cannot_spawn_child_stopping() {
    assert!(!ActorLifecycleState::Stopping.can_spawn_child());
}

#[test]
fn actor_lifecycle_state_cannot_spawn_child_stopped() {
    assert!(!ActorLifecycleState::Stopped.can_spawn_child());
}

#[test]
fn actor_lifecycle_state_cannot_spawn_child_failed() {
    assert!(!ActorLifecycleState::Failed.can_spawn_child());
}

// =============================================================================
// Parent-Child Registry Tests
// =============================================================================

#[tokio::test]
async fn parent_child_registry_add_child() {
    let registry = ParentChildRegistry::new();
    let id = test_instance_id();

    registry.add_child(id.clone()).await;
    let children = registry.get_children().await;

    assert!(children.contains_key(&id));
    assert_eq!(children.get(&id).unwrap().state, ActorLifecycleState::Pending);
}

#[tokio::test]
async fn parent_child_registry_add_multiple_children() {
    let registry = ParentChildRegistry::new();
    let id1 = test_instance_id();
    let id2 = test_instance_id();

    registry.add_child(id1.clone()).await;
    registry.add_child(id2.clone()).await;

    let children = registry.get_children().await;
    assert_eq!(children.len(), 2);
    assert!(children.contains_key(&id1));
    assert!(children.contains_key(&id2));
}

#[tokio::test]
async fn parent_child_registry_remove_child() {
    let registry = ParentChildRegistry::new();
    let id = test_instance_id();

    registry.add_child(id.clone()).await;
    registry.remove_child(&id).await;

    let children = registry.get_children().await;
    assert!(!children.contains_key(&id));
}

#[tokio::test]
async fn parent_child_registry_remove_nonexistent() {
    let registry = ParentChildRegistry::new();
    let id = test_instance_id();

    registry.remove_child(&id).await;

    let children = registry.get_children().await;
    assert!(children.is_empty());
}

#[tokio::test]
async fn parent_child_registry_update_state() {
    let registry = ParentChildRegistry::new();
    let id = test_instance_id();

    registry.add_child(id.clone()).await;

    let info = registry
        .update_child_state(&id, ActorLifecycleState::Running)
        .await;

    assert!(info.is_some());
    assert_eq!(info.unwrap().state, ActorLifecycleState::Running);
}

#[tokio::test]
async fn parent_child_registry_update_nonexistent() {
    let registry = ParentChildRegistry::new();
    let id = test_instance_id();

    let result = registry
        .update_child_state(&id, ActorLifecycleState::Running)
        .await;

    assert!(result.is_none());
}

#[tokio::test]
async fn parent_child_registry_get_children_by_state() {
    let registry = ParentChildRegistry::new();
    let id1 = test_instance_id();
    let id2 = test_instance_id();
    let id3 = test_instance_id();

    registry.add_child(id1.clone()).await;
    registry.add_child(id2.clone()).await;
    registry.add_child(id3.clone()).await;

    registry
        .update_child_state(&id1, ActorLifecycleState::Running)
        .await;
    registry
        .update_child_state(&id2, ActorLifecycleState::Running)
        .await;

    let running = registry
        .get_children_by_state(ActorLifecycleState::Running)
        .await;
    assert_eq!(running.len(), 2);
    assert!(running.contains(&id1));
    assert!(running.contains(&id2));

    let pending = registry
        .get_children_by_state(ActorLifecycleState::Pending)
        .await;
    assert_eq!(pending.len(), 1);
    assert!(pending.contains(&id3));
}

#[tokio::test]
async fn parent_child_registry_all_children_terminal_false() {
    let registry = ParentChildRegistry::new();
    let id1 = test_instance_id();
    let id2 = test_instance_id();

    registry.add_child(id1.clone()).await;
    registry.add_child(id2.clone()).await;

    registry
        .update_child_state(&id1, ActorLifecycleState::Stopped)
        .await;

    assert!(!registry.all_children_terminal().await);
}

#[tokio::test]
async fn parent_child_registry_all_children_terminal_true() {
    let registry = ParentChildRegistry::new();
    let id1 = test_instance_id();
    let id2 = test_instance_id();

    registry.add_child(id1.clone()).await;
    registry.add_child(id2.clone()).await;

    registry
        .update_child_state(&id1, ActorLifecycleState::Stopped)
        .await;
    registry
        .update_child_state(&id2, ActorLifecycleState::Failed)
        .await;

    assert!(registry.all_children_terminal().await);
}

#[tokio::test]
async fn parent_child_registry_all_children_stopped_true() {
    let registry = ParentChildRegistry::new();
    let id1 = test_instance_id();
    let id2 = test_instance_id();

    registry.add_child(id1.clone()).await;
    registry.add_child(id2.clone()).await;

    registry
        .update_child_state(&id1, ActorLifecycleState::Stopping)
        .await;
    registry
        .update_child_state(&id2, ActorLifecycleState::Stopped)
        .await;

    assert!(registry.all_children_stopped().await);
}

#[tokio::test]
async fn parent_child_registry_active_count_empty() {
    let registry = ParentChildRegistry::new();
    assert_eq!(registry.active_children_count().await, 0);
}

#[tokio::test]
async fn parent_child_registry_active_count_mixed() {
    let registry = ParentChildRegistry::new();
    let id1 = test_instance_id();
    let id2 = test_instance_id();
    let id3 = test_instance_id();

    registry.add_child(id1.clone()).await;
    registry.add_child(id2.clone()).await;
    registry.add_child(id3.clone()).await;

    registry
        .update_child_state(&id1, ActorLifecycleState::Running)
        .await;
    registry
        .update_child_state(&id2, ActorLifecycleState::Stopped)
        .await;

    assert_eq!(registry.active_children_count().await, 2);

    registry
        .update_child_state(&id3, ActorLifecycleState::Failed)
        .await;

    assert_eq!(registry.active_children_count().await, 1);
}

#[tokio::test]
async fn parent_child_registry_all_terminal_empty() {
    let registry = ParentChildRegistry::new();
    assert!(registry.all_children_terminal().await);
}

#[tokio::test]
async fn parent_child_registry_all_stopped_empty() {
    let registry = ParentChildRegistry::new();
    assert!(registry.all_children_stopped().await);
}

// =============================================================================
// Shutdown Propagator Tests
// =============================================================================

#[test]
fn shutdown_propagator_default_timeouts() {
    let propagator = ShutdownPropagator::default_propagator();
    assert_eq!(
        propagator.graceful_timeout(),
        std::time::Duration::from_secs(30)
    );
    assert_eq!(
        propagator.force_kill_timeout(),
        std::time::Duration::from_secs(10)
    );
}

#[test]
fn shutdown_propagator_custom_timeouts() {
    let propagator = ShutdownPropagator::new(
        std::time::Duration::from_secs(60),
        std::time::Duration::from_secs(15),
    );
    assert_eq!(
        propagator.graceful_timeout(),
        std::time::Duration::from_secs(60)
    );
    assert_eq!(
        propagator.force_kill_timeout(),
        std::time::Duration::from_secs(15)
    );
}

// =============================================================================
// Lifecycle Display and Error Tests
// =============================================================================

#[test]
fn display_trait_actor_lifecycle_state() {
    assert_eq!(format!("{}", ActorLifecycleState::Pending), "pending");
    assert_eq!(format!("{}", ActorLifecycleState::Running), "running");
    assert_eq!(format!("{}", ActorLifecycleState::Stopping), "stopping");
    assert_eq!(format!("{}", ActorLifecycleState::Stopped), "stopped");
    assert_eq!(format!("{}", ActorLifecycleState::Failed), "failed");
}

#[test]
fn child_info_contains_instance_id_and_state() {
    let id = test_instance_id();
    let info = ChildInfo {
        instance_id: id.clone(),
        state: ActorLifecycleState::Pending,
        added_at: std::time::Instant::now(),
    };

    assert_eq!(info.instance_id, id);
    assert_eq!(info.state, ActorLifecycleState::Pending);
}

// =============================================================================
// Valid Transitions Enumeration Tests
// =============================================================================

#[test]
fn valid_transitions_from_pending() {
    let transitions = ActorLifecycleState::Pending.valid_transitions();
    assert_eq!(transitions.len(), 2);
    assert!(transitions.contains(&LifecycleTransition::Start));
    assert!(transitions.contains(&LifecycleTransition::Fail));
}

#[test]
fn valid_transitions_from_running() {
    let transitions = ActorLifecycleState::Running.valid_transitions();
    assert_eq!(transitions.len(), 2);
    assert!(transitions.contains(&LifecycleTransition::Stop));
    assert!(transitions.contains(&LifecycleTransition::Fail));
}

#[test]
fn valid_transitions_from_stopping() {
    let transitions = ActorLifecycleState::Stopping.valid_transitions();
    assert_eq!(transitions.len(), 2);
    assert!(transitions.contains(&LifecycleTransition::ChildStopped));
    assert!(transitions.contains(&LifecycleTransition::AllChildrenStopped));
}

#[test]
fn valid_transitions_from_stopped() {
    let transitions = ActorLifecycleState::Stopped.valid_transitions();
    assert!(transitions.is_empty());
}

#[test]
fn valid_transitions_from_failed() {
    let transitions = ActorLifecycleState::Failed.valid_transitions();
    assert!(transitions.is_empty());
}
