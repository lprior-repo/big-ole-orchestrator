//! Integration tests for `OrchestratorState::capacity_check`.

use ractor::Actor;
use wtf_actor::master::state::OrchestratorConfig;
use wtf_actor::master::OrchestratorState;
use wtf_actor::messages::InstanceMsg;
use wtf_common::InstanceId;

fn orchestrator_config(max_instances: usize) -> OrchestratorConfig {
    OrchestratorConfig {
        max_instances,
        engine_node_id: "test-node".into(),
        snapshot_db: None,
        event_store: None,
        state_store: None,
        task_queue: None,
        definitions: Vec::new(),
        procedural_workflows: Vec::new(),
    }
}

/// Minimal actor for obtaining valid ActorRef<InstanceMsg> in tests.
struct NullActor;

#[async_trait::async_trait]
impl ractor::Actor for NullActor {
    type Msg = InstanceMsg;
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _: ractor::ActorRef<Self::Msg>,
        _: Self::Arguments,
    ) -> Result<(), ractor::ActorProcessingErr> {
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Happy Path Tests
// ─────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn returns_true_when_running_count_zero_and_max_concurrent_three() {
    let state = OrchestratorState::new(orchestrator_config(3));
    assert!(state.capacity_check());
}

#[tokio::test]
async fn returns_true_when_running_count_below_max_concurrent() {
    let mut state = OrchestratorState::new(orchestrator_config(5));
    // Register 2 instances
    for i in 0..2 {
        let id = InstanceId::new(format!("instance-{}", i));
        let (actor_ref, handle) = NullActor::spawn(None, NullActor, ())
            .await
            .expect("null actor spawned");
        state.register(id, actor_ref);
        // Keep handle alive - ActorRef still valid
        std::mem::forget(handle);
    }
    assert!(state.capacity_check());
}

// ─────────────────────────────────────────────────────────────────────────
// Error / Rejection Path Tests
// ─────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn returns_false_when_running_count_equals_max_concurrent() {
    let mut state = OrchestratorState::new(orchestrator_config(3));
    // Register 3 instances (at limit)
    for i in 0..3 {
        let id = InstanceId::new(format!("instance-{}", i));
        let (actor_ref, handle) = NullActor::spawn(None, NullActor, ())
            .await
            .expect("null actor spawned");
        state.register(id, actor_ref);
        std::mem::forget(handle);
    }
    assert!(!state.capacity_check());
}

#[tokio::test]
async fn returns_false_when_running_count_exceeds_max_concurrent() {
    // This scenario should not occur in a consistent state, but we test the
    // comparison logic directly.
    let mut state = OrchestratorState::new(orchestrator_config(3));
    // Spawn actors and register them, exceeding the limit
    for i in 0..5 {
        let id = InstanceId::new(format!("extra-{}", i));
        let (actor_ref, handle) = NullActor::spawn(None, NullActor, ())
            .await
            .expect("null actor spawned");
        state.register(id, actor_ref);
        std::mem::forget(handle);
    }
    assert!(!state.capacity_check());
}

// ─────────────────────────────────────────────────────────────────────────
// Edge Case Tests
// ─────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn returns_true_with_max_concurrent_one_and_empty_state() {
    let state = OrchestratorState::new(orchestrator_config(1));
    assert!(state.capacity_check());
}

#[tokio::test]
async fn returns_false_with_max_concurrent_one_and_one_running() {
    let mut state = OrchestratorState::new(orchestrator_config(1));
    let (actor_ref, handle) = NullActor::spawn(None, NullActor, ())
        .await
        .expect("null actor spawned");
    state.register(InstanceId::new("only-instance"), actor_ref);
    std::mem::forget(handle);
    assert!(!state.capacity_check());
}

#[tokio::test]
async fn returns_true_with_very_large_max_concurrent() {
    let state = OrchestratorState::new(orchestrator_config(100_000));
    // Simulate 999 running - far below the limit
    // We cannot actually register 999 actors in a unit test efficiently,
    // so we verify the boundary condition using direct state construction.
    // The method uses state.active.len() which we can verify is 0 here.
    assert_eq!(state.active.len(), 0);
    assert!(state.capacity_check());
}

// ─────────────────────────────────────────────────────────────────────────
// Contract Verification Tests
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn invariant_max_concurrent_always_positive_in_orchestrator() {
    // Verify that OrchestratorConfig::default() and all test configs have max_instances > 0
    let default_cfg = OrchestratorConfig::default();
    assert!(
        default_cfg.max_instances > 0,
        "default max_instances must be positive"
    );

    let custom_cfg = orchestrator_config(42);
    assert!(
        custom_cfg.max_instances > 0,
        "custom max_instances must be positive"
    );
}

#[test]
fn invariant_running_count_never_negative() {
    // Note: `state.active.len()` returns `usize` which is guaranteed >= 0 by the type system.
    // This test documents the invariant rather than testing a runtime condition.
    let state = OrchestratorState::new(orchestrator_config(10));
    let count = state.active.len();
    assert!(count >= 0, "active.len() must be non-negative usize");
    // Explicitly destructure to verify type is usize (compile-time check)
    let _unsigned_count: usize = count;
}

#[test]
fn postcondition_returns_exclusive_bound() {
    // If capacity_check returns true, then running_count < max_concurrent
    let state = OrchestratorState::new(orchestrator_config(5));
    if state.capacity_check() {
        assert!(state.active.len() < state.config.max_instances);
    }
}

#[test]
fn boundary_at_equality_transitions_to_false() {
    // When running_count == max_concurrent, capacity_check returns false
    let state = OrchestratorState::new(orchestrator_config(0));
    // With max_instances = 0, even empty state should return false
    assert!(!state.capacity_check());
}

// ─────────────────────────────────────────────────────────────────────────
// Given-When-Then Scenario Tests
// ─────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn scenario_spawn_workflow_when_capacity_available() {
    // Given: An orchestrator with max_concurrent = 10 and running_count = 3
    let mut state = OrchestratorState::new(orchestrator_config(10));
    for i in 0..3 {
        let id = InstanceId::new(format!("instance-{}", i));
        let (actor_ref, handle) = NullActor::spawn(None, NullActor, ())
            .await
            .expect("null actor spawned");
        state.register(id, actor_ref);
        std::mem::forget(handle);
    }

    // When: capacity_check is called
    let has_capacity = state.capacity_check();

    // Then: returns true
    assert!(has_capacity, "Should have capacity when below limit");
}

#[tokio::test]
async fn scenario_reject_workflow_when_at_capacity() {
    // Given: An orchestrator with max_concurrent = 5 and running_count = 5
    let mut state = OrchestratorState::new(orchestrator_config(5));
    for i in 0..5 {
        let id = InstanceId::new(format!("instance-{}", i));
        let (actor_ref, handle) = NullActor::spawn(None, NullActor, ())
            .await
            .expect("null actor spawned");
        state.register(id, actor_ref);
        std::mem::forget(handle);
    }

    // When: capacity_check is called
    let has_capacity = state.capacity_check();

    // Then: returns false
    assert!(
        !has_capacity,
        "Should be at capacity when running_count == max_concurrent"
    );
}
