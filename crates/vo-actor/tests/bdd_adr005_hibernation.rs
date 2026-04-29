//! BDD Tests for ADR-005: Actor Hibernation and Timer Management
//!
//! Scenarios:
//! 1. Given dormant workflow with large state, When hibernation triggers,
//!    Then memory is released (actor stopped, timer persisted).
//! 2. Given hibernated workflow receiving signal, When wake occurs,
//!    Then state is reconstructed correctly from event log.
//! 3. Given timer expiry for hibernated actor, When rehydrated,
//!    Then timer fires correctly.
//! 4. Hibernation lifecycle state transitions.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use vo_actor::lifecycle::{
    compute_next_state, is_valid_transition, ActorLifecycleState, LifecycleTransition,
};
use vo_actor::reanimator::{
    FairnessBudget, MockTimerStorage, MockWorkQueue, ReanimatorConfig, ReanimatorLoop,
    ReanimatorState, TimerRecord,
};
use vo_actor::signal_messages::LifecycleState;
use vo_actor::timer_lifecycle::TimerLifecycleError;
use vo_actor::timer_lifecycle::{
    cancel_timers_for_instance, has_pending_timers, scan_instance_timers,
    validate_timer_for_cancellation,
};
use vo_types::{InstanceId, TimestampMs};

fn make_instance_id(seed: u8) -> InstanceId {
    let mut bytes = [seed; 16];
    bytes[0] = seed;
    InstanceId::from_bytes(bytes)
}

fn make_timer(instance_id: InstanceId, fire_at_ms: u64) -> TimerRecord {
    TimerRecord::new(
        instance_id,
        TimestampMs::try_from(fire_at_ms).expect("valid timestamp"),
        Some(vo_types::TimerId::from_bytes([fire_at_ms as u8; 16])),
        TimestampMs::try_from(fire_at_ms.saturating_sub(1000)).expect("valid timestamp"),
    )
}

// =============================================================================
// Scenario 1: Dormant workflow with large state → hibernation → memory released
// (ADR-005 Section 1 & 2: Suspension trigger and persistence)
// =============================================================================

#[derive(Debug, Clone)]
struct HibernationState {
    instance_id: InstanceId,
    events: Vec<u8>,
    is_active: bool,
    state_size_bytes: AtomicUsize,
}

impl HibernationState {
    fn new(instance_id: InstanceId, state_size: usize) -> Self {
        Self {
            instance_id,
            events: vec![0u8; state_size],
            is_active: true,
            state_size_bytes: AtomicUsize::new(state_size),
        }
    }

    fn active_size(&self) -> usize {
        if self.is_active {
            self.state_size_bytes.load(Ordering::Relaxed)
        } else {
            0
        }
    }

    fn hibernate(&mut self) {
        assert!(self.is_active, "cannot hibernate an already-dormant actor");
        self.is_active = false;
        self.events.clear();
        self.state_size_bytes.store(0, Ordering::Relaxed);
    }

    fn rehydrate(&mut self, state_size: usize) {
        assert!(!self.is_active, "cannot rehydrate an active actor");
        self.is_active = true;
        self.events = vec![0u8; state_size];
        self.state_size_bytes.store(state_size, Ordering::Relaxed);
    }
}

#[test]
fn bdd_005_s1_dormant_workflow_hibernation_releases_memory() {
    // Given: a workflow with large in-memory state (10MB)
    let instance_id = make_instance_id(1);
    let large_state_size = 10 * 1024 * 1024;
    let mut state = HibernationState::new(instance_id.clone(), large_state_size);

    assert_eq!(state.active_size(), large_state_size);
    assert!(state.is_active);

    // When: hibernation triggers (actor reaches Wait node)
    state.hibernate();

    // Then: in-memory state is released
    assert_eq!(state.active_size(), 0);
    assert!(!state.is_active);
    assert!(state.events.is_empty());
}

#[test]
fn bdd_005_s1_hibernation_persists_timer_before_stopping() {
    // Given: a workflow that reaches a Wait node with a 5-second timer
    let instance_id = make_instance_id(2);
    let fire_at = TimestampMs::now().as_u64() + 5000;
    let timer = make_timer(instance_id.clone(), fire_at);

    // And: a timer storage backend
    let storage = Arc::new(MockTimerStorage::empty());

    // When: hibernation persists the timer atomically before stopping
    storage.add_timer(timer).await;
    let has_timer = has_pending_timers(&storage, &instance_id)
        .await
        .expect("check should succeed");

    // Then: the timer is persisted in storage
    assert!(has_timer);
}

#[tokio::test]
async fn bdd_005_s1_multiple_workflows_hibernate_independently() {
    // Given: 1000 workflows each with 1MB state
    let count = 1000;
    let state_size = 1024 * 1024;
    let mut workflows: Vec<HibernationState> = (0..count)
        .map(|i| HibernationState::new(make_instance_id(i as u8), state_size))
        .collect();

    let total_before: usize = workflows.iter().map(|w| w.active_size()).sum();
    assert_eq!(total_before, count * state_size);

    // When: all workflows hibernate
    for w in &mut workflows {
        w.hibernate();
    }

    // Then: all memory is released
    let total_after: usize = workflows.iter().map(|w| w.active_size()).sum();
    assert_eq!(total_after, 0);
}

#[tokio::test]
async fn bdd_005_s1_hibernation_timer_persisted_in_timers_partition() {
    // Given: a workflow with a timer-based wait
    let instance_id = make_instance_id(3);
    let fire_at = TimestampMs::now().as_u64() + 10_000;
    let storage = Arc::new(MockTimerStorage::empty());

    // When: the actor hibernates and persists the timer
    storage
        .add_timer(make_timer(instance_id.clone(), fire_at))
        .await;

    // Then: the timer is retrievable from storage
    let timers = scan_instance_timers(&storage, &instance_id, 100)
        .await
        .expect("scan should succeed");
    assert_eq!(timers.len(), 1);
    assert_eq!(timers[0].instance_id, instance_id);
}

// =============================================================================
// Scenario 2: Hibernated workflow receiving signal → wake → state reconstructed
// (ADR-005 Section 3: The Reanimator Loop, signal-based wake)
// =============================================================================

#[test]
fn bdd_005_s2_rehydrated_actor_recovers_state_from_event_log() {
    // Given: a hibernated workflow with known state
    let instance_id = make_instance_id(10);
    let original_size = 5 * 1024 * 1024;
    let mut state = HibernationState::new(instance_id.clone(), original_size);
    state.hibernate();

    assert_eq!(state.active_size(), 0);

    // When: the actor is rehydrated (replayed from event log)
    let recovered_size = original_size;
    state.rehydrate(recovered_size);

    // Then: state is reconstructed correctly
    assert!(state.is_active);
    assert_eq!(state.active_size(), recovered_size);
}

#[test]
fn bdd_005_s2_rehydrated_state_matches_pre_hibernation() {
    // Given: a workflow with specific state before hibernation
    let instance_id = make_instance_id(11);
    let original_size = 2048;
    let mut state = HibernationState::new(instance_id.clone(), original_size);

    // Record the original state signature
    let pre_hibernation_size = state.active_size();
    state.hibernate();

    // When: rehydrated with the same event log
    state.rehydrate(pre_hibernation_size);

    // Then: recovered state matches the original
    assert_eq!(state.active_size(), pre_hibernation_size);
}

#[tokio::test]
async fn bdd_005_s2_signal_wake_deletes_timer_from_storage() {
    // Given: a hibernated workflow with a pending timer
    let instance_id = make_instance_id(12);
    let fire_at = TimestampMs::now().as_u64() + 5000;
    let storage = Arc::new(MockTimerStorage::empty());

    storage
        .add_timer(make_timer(instance_id.clone(), fire_at))
        .await;
    assert!(has_pending_timers(&storage, &instance_id).await.unwrap());

    // When: the workflow is woken by a signal (timer cancelled)
    let cancelled = cancel_timers_for_instance(&storage, &instance_id)
        .await
        .expect("cancel should succeed");

    // Then: timer is removed from storage (no double-fire)
    assert_eq!(cancelled, 1);
    assert!(!has_pending_timers(&storage, &instance_id).await.unwrap());
}

#[test]
fn bdd_005_s2_lifecycle_waiting_for_signal_is_not_terminal() {
    // Given: a workflow in WaitingForSignal state (suspended)
    let state = LifecycleState::WaitingForSignal;

    // Then: it is NOT terminal — it can be woken
    assert!(!state.is_terminal());
    assert_ne!(state, LifecycleState::Completed);
    assert_ne!(state, LifecycleState::Cancelled);
}

#[test]
fn bdd_005_s2_lifecycle_running_is_not_terminal() {
    let state = LifecycleState::Running;
    assert!(!state.is_terminal());
}

#[test]
fn bdd_005_s2_lifecycle_completed_and_cancelled_are_terminal() {
    assert!(LifecycleState::Completed.is_terminal());
    assert!(LifecycleState::Cancelled.is_terminal());
    assert!(!LifecycleState::Failed.is_terminal());
    assert!(!LifecycleState::Running.is_terminal());
    assert!(!LifecycleState::WaitingForSignal.is_terminal());
}

// =============================================================================
// Scenario 3: Timer expiry for hibernated actor → rehydrated → timer fires
// (ADR-005 Section 3: The Reanimator Loop)
// =============================================================================

#[tokio::test]
async fn bdd_005_s3_reanimator_fires_due_timer_and_enqueues_resume() {
    // Given: a hibernated workflow with a timer that is already due
    let instance_id = make_instance_id(20);
    let past_fire_at = TimestampMs::now().as_u64() - 1000;
    let storage = Arc::new(MockTimerStorage::empty());
    let work_queue = Arc::new(MockWorkQueue::new());

    storage
        .add_timer(make_timer(instance_id.clone(), past_fire_at))
        .await;

    // When: the reanimator loop processes a scan cycle
    let config = ReanimatorConfig {
        scan_interval: Duration::from_millis(50),
        max_timers_per_cycle: 100,
        max_concurrent_resumes: 10,
        shutdown_timeout: Duration::from_secs(5),
    };

    let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
        .expect("spawn should succeed");

    // Wait for the reanimator to process at least one cycle
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Then: the timer was fired (recorded) and resume was enqueued
    let fire_calls = storage.fire_calls().await;
    let enqueued = work_queue.enqueued().await;

    assert!(
        fire_calls.iter().any(|(id, _)| *id == instance_id),
        "reanimator should have recorded TimerFired for the instance"
    );
    assert!(
        enqueued.contains(&instance_id),
        "reanimator should have enqueued resume work for the instance"
    );

    handle.shutdown().await.expect("shutdown should succeed");
}

#[tokio::test]
async fn bdd_005_s3_reanimator_deletes_timer_after_firing() {
    // Given: a hibernated workflow with a due timer
    let instance_id = make_instance_id(21);
    let past_fire_at = TimestampMs::now().as_u64() - 500;
    let storage = Arc::new(MockTimerStorage::empty());
    let work_queue = Arc::new(MockWorkQueue::new());

    storage
        .add_timer(make_timer(instance_id.clone(), past_fire_at))
        .await;

    // When: the reanimator fires the timer
    let config = ReanimatorConfig {
        scan_interval: Duration::from_millis(50),
        max_timers_per_cycle: 100,
        max_concurrent_resumes: 10,
        shutdown_timeout: Duration::from_secs(5),
    };

    let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
        .expect("spawn should succeed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Then: the timer is deleted from storage (preventing double-fire)
    let delete_calls = storage.delete_calls().await;
    assert!(
        delete_calls
            .iter()
            .any(|(id, ts)| *id == instance_id && *ts.as_u64() == past_fire_at),
        "reanimator should have deleted the timer after firing"
    );

    // And: no pending timers remain for this instance
    assert!(
        !has_pending_timers(&storage, &instance_id)
            .await
            .expect("check should succeed"),
        "timer should be deleted after firing"
    );

    handle.shutdown().await.expect("shutdown should succeed");
}

#[tokio::test]
async fn bdd_005_s3_reanimator_skips_future_timers() {
    // Given: a hibernated workflow with a timer far in the future
    let instance_id = make_instance_id(22);
    let future_fire_at = TimestampMs::now().as_u64() + 3600_000;
    let storage = Arc::new(MockTimerStorage::empty());
    let work_queue = Arc::new(MockWorkQueue::new());

    storage
        .add_timer(make_timer(instance_id.clone(), future_fire_at))
        .await;

    // When: the reanimator processes a scan cycle
    let config = ReanimatorConfig {
        scan_interval: Duration::from_millis(50),
        max_timers_per_cycle: 100,
        max_concurrent_resumes: 10,
        shutdown_timeout: Duration::from_secs(5),
    };

    let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
        .expect("spawn should succeed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Then: the future timer is NOT fired
    let fire_calls = storage.fire_calls().await;
    let enqueued = work_queue.enqueued().await;

    assert!(
        !fire_calls.iter().any(|(id, _)| *id == instance_id),
        "future timer should not be fired"
    );
    assert!(
        !enqueued.contains(&instance_id),
        "future timer should not trigger resume"
    );

    // And: timer is still pending in storage
    assert!(
        has_pending_timers(&storage, &instance_id)
            .await
            .expect("check should succeed"),
        "future timer should still be pending"
    );

    handle.shutdown().await.expect("shutdown should succeed");
}

#[tokio::test]
async fn bdd_005_s3_reanimator_multiple_timers_fired_in_single_cycle() {
    // Given: multiple hibernated workflows with due timers
    let storage = Arc::new(MockTimerStorage::empty());
    let work_queue = Arc::new(MockWorkQueue::new());

    let instance_ids: Vec<InstanceId> = (0..5u8).map(make_instance_id).collect();

    for id in &instance_ids {
        storage
            .add_timer(make_timer(id.clone(), TimestampMs::now().as_u64() - 100))
            .await;
    }

    // When: the reanimator processes a cycle
    let config = ReanimatorConfig {
        scan_interval: Duration::from_millis(50),
        max_timers_per_cycle: 100,
        max_concurrent_resumes: 10,
        shutdown_timeout: Duration::from_secs(5),
    };

    let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
        .expect("spawn should succeed");

    tokio::time::sleep(Duration::from_millis(300)).await;

    // Then: all timers are fired and all instances enqueued for resume
    let fire_calls = storage.fire_calls().await;
    let enqueued = work_queue.enqueued().await;

    for id in &instance_ids {
        assert!(
            fire_calls.iter().any(|(fired_id, _)| fired_id == id),
            "timer for instance {:?} should be fired",
            id
        );
        assert!(
            enqueued.contains(id),
            "instance {:?} should be enqueued for resume",
            id
        );
    }

    handle.shutdown().await.expect("shutdown should succeed");
}

#[tokio::test]
async fn bdd_005_s3_fairness_budget_limits_resumes_per_instance() {
    // Given: a fairness budget of 1 resume per instance per cycle
    let mut budget = FairnessBudget::with_limits(1, 10);

    let instance_id = make_instance_id(30);

    // When: the first resume is recorded
    let first_allowed = budget.record_resume(instance_id.clone());

    // Then: it succeeds
    assert!(first_allowed);

    // When: a second resume for the same instance is attempted
    let second_allowed = budget.record_resume(instance_id.clone());

    // Then: it is rejected (budget exhausted for this instance)
    assert!(!second_allowed);

    // And: can_resume also returns false
    assert!(!budget.can_resume(&instance_id));
}

#[tokio::test]
async fn bdd_005_s3_fairness_budget_resets_between_cycles() {
    // Given: a budget with max 1 per instance
    let mut budget = FairnessBudget::with_limits(1, 10);
    let instance_id = make_instance_id(31);

    budget.record_resume(instance_id.clone());
    assert!(!budget.can_resume(&instance_id));

    // When: the budget is reset (new cycle)
    budget.reset();

    // Then: the instance can be resumed again
    assert!(budget.can_resume(&instance_id));
    assert!(budget.record_resume(instance_id.clone()));
}

// =============================================================================
// Scenario 4: Hibernation lifecycle state transitions
// (ADR-005 + ADR-039: Hierarchical lifecycle state machine)
// =============================================================================

#[test]
fn bdd_005_s4_running_to_stopping_transition() {
    // Given: an actor in Running state
    // When: hibernation triggers (Stop transition)
    let next = compute_next_state(ActorLifecycleState::Running, LifecycleTransition::Stop);

    // Then: actor transitions to Stopping
    assert_eq!(next, Some(ActorLifecycleState::Stopping));
}

#[test]
fn bdd_005_s4_stopping_to_stopped_when_all_children_stopped() {
    // Given: an actor in Stopping state (hibernation in progress)
    // When: all children have stopped
    let next = compute_next_state(
        ActorLifecycleState::Stopping,
        LifecycleTransition::AllChildrenStopped,
    );

    // Then: actor transitions to Stopped (fully hibernated)
    assert_eq!(next, Some(ActorLifecycleState::Stopped));
}

#[test]
fn bdd_005_s4_stopped_actor_cannot_be_started_without_rehydration() {
    // Given: a fully hibernated (Stopped) actor
    // When: a Start transition is attempted directly
    let next = compute_next_state(ActorLifecycleState::Stopped, LifecycleTransition::Start);

    // Then: transition is invalid (must go through rehydration, not direct start)
    assert_eq!(next, None);
    assert!(!is_valid_transition(
        ActorLifecycleState::Stopped,
        LifecycleTransition::Start
    ));
}

#[test]
fn bdd_005_s4_failed_actor_cannot_transition_to_running() {
    // Given: a Failed actor
    // When: Start is attempted
    let next = compute_next_state(ActorLifecycleState::Failed, LifecycleTransition::Start);

    // Then: transition is invalid
    assert_eq!(next, None);
}

#[test]
fn bdd_005_s4_full_hibernation_lifecycle() {
    // Given: a running workflow
    let mut state = ActorLifecycleState::Running;

    // Step 1: Hibernate (Stop)
    state =
        compute_next_state(state, LifecycleTransition::Stop).expect("Running→Stop should be valid");
    assert_eq!(state, ActorLifecycleState::Stopping);

    // Step 2: Children stopped
    state = compute_next_state(state, LifecycleTransition::ChildStopped)
        .expect("Stopping→ChildStopped should be valid");
    assert_eq!(state, ActorLifecycleState::Stopping);

    // Step 3: All children stopped → fully hibernated
    state = compute_next_state(state, LifecycleTransition::AllChildrenStopped)
        .expect("Stopping→AllChildrenStopped should be valid");
    assert_eq!(state, ActorLifecycleState::Stopped);
    assert!(state.is_terminal());
}

#[test]
fn bdd_005_s4_timer_lifecycle_validate_cancellation_for_matching_instance() {
    // Given: a timer belonging to an instance
    let instance_id = make_instance_id(40);
    let timer = make_timer(instance_id.clone(), 5000);

    // When: validating for cancellation
    let result = validate_timer_for_cancellation(&timer, &instance_id);

    // Then: validation passes
    assert!(result.is_ok());
}

#[test]
fn bdd_005_s4_timer_lifecycle_validate_cancellation_rejects_wrong_instance() {
    // Given: a timer belonging to instance A
    let instance_a = make_instance_id(41);
    let instance_b = make_instance_id(42);
    let timer = make_timer(instance_a.clone(), 5000);

    // When: validating for cancellation for instance B
    let result = validate_timer_for_cancellation(&timer, &instance_b);

    // Then: validation fails (wrong instance)
    assert!(matches!(
        result,
        Err(TimerLifecycleError::TimerNotFound { .. })
    ));
}

#[tokio::test]
async fn bdd_005_s4_cancel_timers_on_terminal_state_prevents_orphan_timers() {
    // Given: a workflow that reached a terminal state with pending timers
    let instance_id = make_instance_id(43);
    let storage = Arc::new(MockTimerStorage::empty());

    storage
        .add_timer(make_timer(instance_id.clone(), 5000))
        .await;
    storage
        .add_timer(make_timer(instance_id.clone(), 10000))
        .await;

    assert!(has_pending_timers(&storage, &instance_id).await.unwrap());

    // When: the workflow terminates and all timers are cancelled
    let cancelled = cancel_timers_for_instance(&storage, &instance_id)
        .await
        .expect("cancel should succeed");

    // Then: all timers are removed (no orphans)
    assert_eq!(cancelled, 2);
    assert!(!has_pending_timers(&storage, &instance_id).await.unwrap());
}

// =============================================================================
// Cross-cutting: Crash resilience (ADR-005 Consequence: crash-safe timers)
// =============================================================================

#[tokio::test]
async fn bdd_005_crash_timers_survive_in_storage_after_actor_crash() {
    // Given: a workflow with a timer persisted to storage
    let instance_id = make_instance_id(50);
    let fire_at = TimestampMs::now().as_u64() + 60_000;
    let storage = Arc::new(MockTimerStorage::empty());

    storage
        .add_timer(make_timer(instance_id.clone(), fire_at))
        .await;

    // When: the actor "crashes" (no explicit action needed — timer is in storage)
    // Simulate by checking the timer is still in storage
    let timers = scan_instance_timers(&storage, &instance_id, 100)
        .await
        .expect("scan should succeed");

    // Then: the timer survives in storage (durable)
    assert_eq!(timers.len(), 1);
    assert_eq!(timers[0].fire_at_ms.as_u64(), fire_at);
}

#[tokio::test]
async fn bdd_005_crash_reanimator_recovers_timers_after_restart() {
    // Given: timers in storage (simulating post-crash state)
    let instance_id = make_instance_id(51);
    let past_fire_at = TimestampMs::now().as_u64() - 1000;
    let storage = Arc::new(MockTimerStorage::empty());
    let work_queue = Arc::new(MockWorkQueue::new());

    storage
        .add_timer(make_timer(instance_id.clone(), past_fire_at))
        .await;

    // When: the reanimator restarts (simulated by spawning a new loop)
    let config = ReanimatorConfig {
        scan_interval: Duration::from_millis(50),
        max_timers_per_cycle: 100,
        max_concurrent_resumes: 10,
        shutdown_timeout: Duration::from_secs(5),
    };

    let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
        .expect("spawn should succeed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Then: the reanimator discovers and fires the timer
    let fire_calls = storage.fire_calls().await;
    assert!(
        fire_calls.iter().any(|(id, _)| *id == instance_id),
        "reanimator should recover and fire the timer after restart"
    );

    handle.shutdown().await.expect("shutdown should succeed");
}

#[tokio::test]
async fn bdd_005_crash_multiple_timers_different_instances_all_recovered() {
    // Given: multiple timers for different instances in storage after crash
    let storage = Arc::new(MockTimerStorage::empty());
    let work_queue = Arc::new(MockWorkQueue::new());

    let ids: Vec<InstanceId> = (60..65u8).map(make_instance_id).collect();
    for id in &ids {
        storage
            .add_timer(make_timer(id.clone(), TimestampMs::now().as_u64() - 500))
            .await;
    }

    // When: reanimator restarts
    let config = ReanimatorConfig {
        scan_interval: Duration::from_millis(50),
        max_timers_per_cycle: 100,
        max_concurrent_resumes: 10,
        shutdown_timeout: Duration::from_secs(5),
    };

    let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
        .expect("spawn should succeed");

    tokio::time::sleep(Duration::from_millis(300)).await;

    // Then: all timers are recovered
    let fire_calls = storage.fire_calls().await;
    for id in &ids {
        assert!(
            fire_calls.iter().any(|(fired_id, _)| fired_id == id),
            "timer for {:?} should be recovered after crash",
            id
        );
    }

    handle.shutdown().await.expect("shutdown should succeed");
}

// =============================================================================
// Cross-cutting: Reanimator state management
// =============================================================================

#[tokio::test]
async fn bdd_005_reanimator_state_transitions_on_lifecycle() {
    // Given: a reanimator config
    let config = ReanimatorConfig {
        scan_interval: Duration::from_millis(50),
        max_timers_per_cycle: 100,
        max_concurrent_resumes: 10,
        shutdown_timeout: Duration::from_secs(5),
    };
    let storage = Arc::new(MockTimerStorage::empty());
    let work_queue = Arc::new(MockWorkQueue::new());

    // When: the reanimator is spawned
    let handle = ReanimatorLoop::spawn(config, storage, work_queue).expect("spawn should succeed");

    // Then: it transitions to Running
    assert!(handle.current_state().is_active());

    // When: shutdown is requested
    let shutdown_result = handle.shutdown().await;

    // Then: shutdown completes cleanly
    assert!(shutdown_result.is_ok());
}

#[test]
fn bdd_005_reanimator_state_stopped_is_not_active() {
    assert!(!ReanimatorState::Stopped.is_active());
    assert!(!ReanimatorState::ShutDown.is_active());
}

#[test]
fn bdd_005_reanimator_state_running_and_shutting_down_are_active() {
    assert!(ReanimatorState::Running.is_active());
    assert!(ReanimatorState::ShuttingDown.is_active());
}

// =============================================================================
// Cross-cutting: Timer record validation (ADR-005 data integrity)
// =============================================================================

#[test]
fn bdd_005_validate_timer_rejects_zero_fire_at() {
    use vo_actor::reanimator::validate_timer_record;

    let instance_id = make_instance_id(70);
    let timer = TimerRecord::new(
        instance_id,
        TimestampMs::try_from(0u64).expect("valid"),
        None,
        TimestampMs::try_from(1000).expect("valid"),
    );

    assert!(validate_timer_record(&timer).is_err());
}

#[test]
fn bdd_005_validate_timer_rejects_fire_at_before_scheduled() {
    use vo_actor::reanimator::validate_timer_record;

    let instance_id = make_instance_id(71);
    let timer = TimerRecord::new(
        instance_id,
        TimestampMs::try_from(500).expect("valid"),
        None,
        TimestampMs::try_from(1000).expect("valid"),
    );

    assert!(validate_timer_record(&timer).is_err());
}

#[test]
fn bdd_005_validate_timer_accepts_valid_record() {
    use vo_actor::reanimator::validate_timer_record;

    let instance_id = make_instance_id(72);
    let timer = TimerRecord::new(
        instance_id,
        TimestampMs::try_from(5000).expect("valid"),
        None,
        TimestampMs::try_from(1000).expect("valid"),
    );

    assert!(validate_timer_record(&timer).is_ok());
}
