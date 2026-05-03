//! RED-QUEEN coevolutionary adversarial tests for vo-actor supervision tree crash recovery.
//!
//! ## Bead: ve-lmp
//!
//! Adversarial tests targeting the actor supervisor crash recovery infrastructure:
//! 1. Kill child actor mid-message — verify parent receives termination signal
//! 2. Cascading failure propagation through the hierarchy
//! 3. Restart policy enforcement (max restarts, exponential backoff, isolation)
//! 4. State recovery after supervisor restart
//!
//! ## Contracts
//!
//! **Invariant 1:** Terminal states (Stopped, Failed) are irrevocable.
//! **Invariant 2:** Isolation occurs after exactly `max_restart_attempts` panics.
//! **Invariant 3:** Backoff is monotonically non-decreasing across restart attempts.
//! **Invariant 4:** ParentChildRegistry always reflects child state within bounded time.
//! **Invariant 5:** Shutdown propagation executes in LIFO order (children before parents).
//! **Invariant 6:** PanicCatcher never suppresses errors — every panic surfaces as ActorPanic.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

use vo_actor::actor_supervisor::{
    ActorSupervisorAuditEntry, ActorSupervisorAuditDetails, ActorSupervisorConfig,
    ActorSupervisorError, ActorSupervisorEventType, ActorSupervisorMetrics,
    ActorSupervisorState, PanicCatcher, PanicInfo, RestartDecision,
    compute_restart_decision,
};
use vo_actor::lifecycle::{
    ActorLifecycleState, FailureOutcome, LifecycleTransition, ParentChildRegistry,
    ShutdownPropagator, ShutdownResult, compute_failure_outcome, compute_next_state,
};
use vo_types::InstanceId;

fn test_instance_id(n: u8) -> InstanceId {
    use ulid::Ulid;
    let ulid = Ulid::new();
    let mut bytes = ulid.to_bytes();
    bytes[15] = n;
    InstanceId::from_bytes(bytes)
}

// =============================================================================
// SCENARIO 1: Kill child actor mid-message — parent receives termination signal
// =============================================================================

mod kill_child_mid_message {
    use super::*;

    #[test]
    fn rq_panic_catcher_surfaces_error_on_mid_message_crash() {
        let metrics = ActorSupervisorMetrics::new();
        let id = test_instance_id(1);
        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        let result = PanicCatcher::catch_panic(id.clone(), move || {
            call_count_clone.fetch_add(1, SeqCst);
            panic!("mid-message processing crash");
        }, &metrics);

        assert!(result.is_err(), "panicking closure must produce an error");
        assert_eq!(call_count.load(SeqCst), 1, "closure must have been invoked exactly once");

        match result.unwrap_err() {
            ActorSupervisorError::ActorPanic { instance_id, panic_message, .. } => {
                assert_eq!(instance_id, id);
                assert!(panic_message.contains("mid-message processing crash"));
            }
            other => panic!("expected ActorPanic, got {:?}", other),
        }
        assert_eq!(metrics.get_panic_count(), 1);
    }

    #[tokio::test]
    async fn rq_parent_registry_detects_child_failure_after_panic() {
        let registry = ParentChildRegistry::new();
        let child_id = test_instance_id(1);

        registry.add_child(child_id.clone()).await;
        registry.update_child_state(&child_id, ActorLifecycleState::Running).await;

        let metrics = ActorSupervisorMetrics::new();
        let result = PanicCatcher::catch_panic(child_id.clone(), || {
            panic!("child actor died");
        }, &metrics);

        assert!(result.is_err());

        registry.update_child_state(&child_id, ActorLifecycleState::Failed).await;

        let children = registry.get_children().await;
        let child_info = children.get(&child_id).expect("child must still exist in registry");
        assert_eq!(child_info.state, ActorLifecycleState::Failed);

        assert!(registry.all_children_terminal().await);
    }

    #[tokio::test]
    async fn rq_parent_sees_failed_child_as_terminal_not_active() {
        let registry = ParentChildRegistry::new();
        let c1 = test_instance_id(1);
        let c2 = test_instance_id(2);

        registry.add_child(c1.clone()).await;
        registry.add_child(c2.clone()).await;
        registry.update_child_state(&c1, ActorLifecycleState::Running).await;
        registry.update_child_state(&c2, ActorLifecycleState::Running).await;

        assert_eq!(registry.active_children_count().await, 2);

        registry.update_child_state(&c1, ActorLifecycleState::Failed).await;
        assert_eq!(registry.active_children_count().await, 1);

        let running = registry.get_children_by_state(ActorLifecycleState::Running).await;
        assert_eq!(running, vec![c2]);

        let failed = registry.get_children_by_state(ActorLifecycleState::Failed).await;
        assert_eq!(failed, vec![c1]);
    }

    #[test]
    fn rq_panic_catcher_with_backtrace_captures_state_on_crash() {
        let metrics = ActorSupervisorMetrics::new();
        let id = test_instance_id(1);

        let result = PanicCatcher::catch_panic_with_backtrace(id.clone(), || {
            panic!("crash with backtrace capture");
        }, &metrics);

        let (error, backtrace) = result.unwrap_err();
        assert!(matches!(error, ActorSupervisorError::ActorPanic { .. }));
        assert!(backtrace.is_some(), "backtrace must be captured on panic");
        assert_eq!(metrics.get_panic_count(), 1);
    }

    #[tokio::test]
    async fn rq_parent_child_registry_tracks_multiple_concurrent_failures() {
        let registry = ParentChildRegistry::new();
        let children: Vec<InstanceId> = (0..5).map(|i| test_instance_id(i)).collect();

        for child in &children {
            registry.add_child(child.clone()).await;
            registry.update_child_state(child, ActorLifecycleState::Running).await;
        }

        assert_eq!(registry.active_children_count().await, 5);

        for child in &children {
            registry.update_child_state(child, ActorLifecycleState::Failed).await;
        }

        assert!(registry.all_children_terminal().await);
        assert_eq!(registry.active_children_count().await, 0);
        let failed = registry.get_children_by_state(ActorLifecycleState::Failed).await;
        assert_eq!(failed.len(), 5);
    }

    #[test]
    fn rq_panic_info_captures_instance_id_for_parent_notification() {
        let id = test_instance_id(42);
        let info = PanicInfo::new(
            id.clone(),
            "child exploded".to_string(),
            "backtrace...".to_string(),
        );

        assert_eq!(info.instance_id, id);
        assert_eq!(info.panic_message, "child exploded");
        assert!(info.is_backtrace_available());
        assert_eq!(info.backtrace_status, "captured");
    }
}

// =============================================================================
// SCENARIO 2: Cascading failure propagation
// =============================================================================

mod cascading_failure_propagation {
    use super::*;

    #[test]
    fn rq_failure_transition_prevents_restart_from_terminal_failed() {
        let next = compute_next_state(ActorLifecycleState::Failed, LifecycleTransition::Start);
        assert!(next.is_none(), "Failed is terminal — no transitions allowed");

        let next = compute_next_state(ActorLifecycleState::Failed, LifecycleTransition::Fail);
        assert!(next.is_none(), "Failed is terminal — even Fail is rejected");
    }

    #[test]
    fn rq_failure_transition_prevents_restart_from_terminal_stopped() {
        let next = compute_next_state(ActorLifecycleState::Stopped, LifecycleTransition::Start);
        assert!(next.is_none(), "Stopped is terminal — no transitions allowed");
    }

    #[tokio::test]
    async fn rq_cascading_failure_all_children_fail_before_parent_terminal() {
        let registry = ParentChildRegistry::new();
        let parent_id = test_instance_id(0);
        let c1 = test_instance_id(1);
        let c2 = test_instance_id(2);
        let c3 = test_instance_id(3);

        registry.add_child(c1.clone()).await;
        registry.add_child(c2.clone()).await;
        registry.add_child(c3.clone()).await;

        for c in [&c1, &c2, &c3] {
            registry.update_child_state(c, ActorLifecycleState::Running).await;
        }

        assert!(!registry.all_children_terminal().await);

        registry.update_child_state(&c1, ActorLifecycleState::Failed).await;
        assert!(!registry.all_children_terminal().await, "parent must wait for all children");

        registry.update_child_state(&c2, ActorLifecycleState::Failed).await;
        assert!(!registry.all_children_terminal().await, "one child still active");

        registry.update_child_state(&c3, ActorLifecycleState::Failed).await;
        assert!(registry.all_children_terminal().await, "all children failed — parent can proceed");

        // Parent can now transition to terminal
        let parent_next = compute_next_state(ActorLifecycleState::Stopping, LifecycleTransition::AllChildrenStopped);
        assert_eq!(parent_next, Some(ActorLifecycleState::Stopped));
    }

    #[tokio::test]
    async fn rq_cascading_failure_mixed_terminal_states_count_as_terminal() {
        let registry = ParentChildRegistry::new();
        let c1 = test_instance_id(1);
        let c2 = test_instance_id(2);

        registry.add_child(c1.clone()).await;
        registry.add_child(c2.clone()).await;

        registry.update_child_state(&c1, ActorLifecycleState::Failed).await;
        registry.update_child_state(&c2, ActorLifecycleState::Stopped).await;

        assert!(registry.all_children_terminal().await,
            "mixed Failed+Stopped should both be terminal");
    }

    #[test]
    fn rq_epoch_failure_allows_lineage_continue() {
        let outcome = compute_failure_outcome(ActorLifecycleState::Running, vo_types::signal::FailureScope::Epoch);
        assert!(outcome.is_epoch_failure());
        assert!(outcome.can_lineage_spawn_epoch(), "epoch failure must allow lineage to continue");
        assert_eq!(outcome.actor_state(), ActorLifecycleState::Failed);
    }

    #[test]
    fn rq_lineage_failure_tombstones_lineage() {
        let outcome = compute_failure_outcome(ActorLifecycleState::Running, vo_types::signal::FailureScope::Lineage);
        assert!(outcome.is_lineage_failure());
        assert!(!outcome.can_lineage_spawn_epoch(), "lineage failure must block future epochs");
    }

    #[tokio::test]
    async fn rq_shutdown_propagation_order_is_lifo_children_before_parents() {
        let propagator = ShutdownPropagator::new(
            std::time::Duration::from_millis(100),
            std::time::Duration::from_millis(50),
        );
        let order = Arc::new(std::sync::Mutex::new(Vec::new()));

        let o1 = order.clone();
        propagator.register_drop_sync("parent", move || {
            o1.lock().unwrap().push("parent");
        });

        let o2 = order.clone();
        propagator.register_drop_sync("child", move || {
            o2.lock().unwrap().push("child");
        });

        let o3 = order.clone();
        propagator.register_drop_sync("grandchild", move || {
            o3.lock().unwrap().push("grandchild");
        });

        let result = propagator.propagate();
        assert!(matches!(result, ShutdownResult::Success));

        let order = order.lock().unwrap();
        assert_eq!(order.len(), 3);
        assert_eq!(order[0], "grandchild", "LIFO: last registered drops first");
        assert_eq!(order[1], "child");
        assert_eq!(order[2], "parent", "parent drops last");
    }

    #[tokio::test]
    async fn rq_child_removed_from_registry_during_shutdown() {
        let registry = ParentChildRegistry::new();
        let c1 = test_instance_id(1);
        let c2 = test_instance_id(2);

        registry.add_child(c1.clone()).await;
        registry.add_child(c2.clone()).await;

        registry.remove_child(&c1).await;
        assert_eq!(registry.active_children_count().await, 1);

        let children = registry.get_children().await;
        assert!(!children.contains_key(&c1));
        assert!(children.contains_key(&c2));
    }

    #[test]
    fn rq_running_actor_can_fail_directly() {
        let next = compute_next_state(ActorLifecycleState::Running, LifecycleTransition::Fail);
        assert_eq!(next, Some(ActorLifecycleState::Failed),
            "Running → Fail must be a valid transition for crash propagation");
    }

    #[test]
    fn rq_pending_actor_can_fail_before_start() {
        let next = compute_next_state(ActorLifecycleState::Pending, LifecycleTransition::Fail);
        assert_eq!(next, Some(ActorLifecycleState::Failed),
            "Pending → Fail must be valid (child crashes during initialization)");
    }

    #[test]
    fn rq_stopping_actor_rejects_fail_cascade_prevention() {
        let next = compute_next_state(ActorLifecycleState::Stopping, LifecycleTransition::Fail);
        assert!(next.is_none(),
            "Stopping must reject Fail — actor is already shutting down");
    }
}

// =============================================================================
// SCENARIO 3: Restart policy enforcement (max restarts, backoff, isolation)
// =============================================================================

mod restart_policy_enforcement {
    use super::*;

    #[test]
    fn rq_first_restart_is_immediate_no_backoff() {
        let state = ActorSupervisorState::new();
        let config = ActorSupervisorConfig::default();
        let decision = compute_restart_decision(&state, &config);
        assert!(matches!(decision, RestartDecision::RestartNow),
            "first restart attempt must have zero backoff");
    }

    #[test]
    fn rq_second_restart_uses_exponential_backoff() {
        let mut state = ActorSupervisorState::new();
        state.record_restart(); // attempt 1
        let config = ActorSupervisorConfig::default();
        let decision = compute_restart_decision(&state, &config);

        match decision {
            RestartDecision::RestartWithBackoff(delay) => {
                assert_eq!(delay, 100, "first backoff = initial_backoff_ms * 2^0 = 100ms");
            }
            other => panic!("expected RestartWithBackoff, got {:?}", other),
        }
    }

    #[test]
    fn rq_third_restart_uses_doubled_backoff() {
        let mut state = ActorSupervisorState::new();
        state.record_restart(); // attempt 1
        state.record_restart(); // attempt 2
        let config = ActorSupervisorConfig {
            initial_backoff_ms: 100,
            backoff_multiplier: 2.0,
            ..Default::default()
        };
        let decision = compute_restart_decision(&state, &config);

        match decision {
            RestartDecision::RestartWithBackoff(delay) => {
                assert_eq!(delay, 200, "second backoff = 100 * 2^1 = 200ms");
            }
            other => panic!("expected RestartWithBackoff, got {:?}", other),
        }
    }

    #[test]
    fn rq_isolation_triggered_after_max_restart_attempts() {
        let mut state = ActorSupervisorState::new();
        for _ in 0..3 {
            state.record_restart();
        }
        let config = ActorSupervisorConfig {
            max_restart_attempts: 3,
            ..Default::default()
        };
        let decision = compute_restart_decision(&state, &config);
        assert!(matches!(decision, RestartDecision::Isolate),
            "after max_restart_attempts, actor must be isolated");
        assert!(decision.should_isolate());
    }

    #[test]
    fn rq_backoff_capped_at_max_backoff_ms() {
        let mut state = ActorSupervisorState::new();
        state.restart_attempts = 50; // absurdly high
        let config = ActorSupervisorConfig {
            max_restart_attempts: 100, // even higher to avoid isolation
            initial_backoff_ms: 100,
            backoff_multiplier: 10.0,
            max_backoff_ms: 1000,
        };
        let decision = compute_restart_decision(&state, &config);

        match decision {
            RestartDecision::RestartWithBackoff(delay) => {
                assert!(delay <= 1000,
                    "backoff must be capped at max_backoff_ms, got {}", delay);
            }
            other => panic!("expected RestartWithBackoff, got {:?}", other),
        }
    }

    #[test]
    fn rq_isolation_means_no_more_restarts() {
        let decision = RestartDecision::Isolate;
        assert!(!decision.should_restart(), "isolated actor must not restart");
        assert!(decision.should_isolate());
    }

    #[test]
    fn rq_no_restart_means_no_restart() {
        let decision = RestartDecision::NoRestart;
        assert!(!decision.should_restart());
    }

    #[test]
    fn rq_restart_now_does_restart() {
        let decision = RestartDecision::RestartNow;
        assert!(decision.should_restart());
        assert!(!decision.should_isolate());
    }

    #[test]
    fn rq_backoff_with_ms_does_restart() {
        let decision = RestartDecision::RestartWithBackoff(500);
        assert!(decision.should_restart());
        assert!(!decision.should_isolate());
    }

    #[test]
    fn rq_state_tracks_restart_attempt_count_accurately() {
        let mut state = ActorSupervisorState::new();
        assert_eq!(state.restart_attempts, 0);
        assert!(state.can_restart(3));

        state.record_restart();
        assert_eq!(state.restart_attempts, 1);
        assert!(state.can_restart(3));

        state.record_restart();
        assert_eq!(state.restart_attempts, 2);
        assert!(state.can_restart(3));

        state.record_restart();
        assert_eq!(state.restart_attempts, 3);
        assert!(!state.can_restart(3), "at max attempts, can_restart must be false");
    }

    #[test]
    fn rq_should_isolate_true_only_at_or_above_max() {
        let mut state = ActorSupervisorState::new();
        state.restart_attempts = 2;
        assert!(!state.should_isolate(3));

        state.restart_attempts = 3;
        assert!(state.should_isolate(3));

        state.restart_attempts = 100;
        assert!(state.should_isolate(3));
    }

    #[test]
    fn rq_custom_config_respected_for_max_attempts() {
        let config = ActorSupervisorConfig {
            max_restart_attempts: 1,
            ..Default::default()
        };

        let state_fresh = ActorSupervisorState::new();
        assert!(matches!(
            compute_restart_decision(&state_fresh, &config),
            RestartDecision::RestartNow
        ));

        let mut state_one = ActorSupervisorState::new();
        state_one.record_restart();
        assert!(matches!(
            compute_restart_decision(&state_one, &config),
            RestartDecision::Isolate
        ));
    }

    #[test]
    fn rq_error_classification_panic_is_transient_not_fatal() {
        let id = test_instance_id(1);
        let error = ActorSupervisorError::ActorPanic {
            instance_id: id,
            panic_message: "boom".to_string(),
            backtrace: "".to_string(),
        };
        assert!(error.is_transient(), "individual panics are transient");
        assert!(!error.is_fatal(), "individual panics are not fatal");
    }

    #[test]
    fn rq_error_classification_max_restarts_is_fatal() {
        let id = test_instance_id(1);
        let error = ActorSupervisorError::MaxRestartsExceeded {
            instance_id: id,
            max_attempts: 3,
        };
        assert!(!error.is_transient());
        assert!(error.is_fatal(), "exceeding max restarts is fatal");
    }

    #[test]
    fn rq_error_classification_isolation_is_fatal() {
        let id = test_instance_id(1);
        let error = ActorSupervisorError::ActorIsolated { instance_id: id };
        assert!(!error.is_transient());
        assert!(error.is_fatal(), "isolation is fatal");
    }

    #[test]
    fn rq_metrics_track_panic_and_restart_independently() {
        let metrics = ActorSupervisorMetrics::new();

        metrics.record_panic();
        metrics.record_panic();
        metrics.record_restart();

        assert_eq!(metrics.get_panic_count(), 2);
        assert_eq!(metrics.get_restart_count(), 1);
        assert_eq!(metrics.get_isolation_count(), 0);
    }

    #[test]
    fn rq_metrics_track_isolation_on_max_restarts() {
        let metrics = ActorSupervisorMetrics::new();
        let config = ActorSupervisorConfig::default();
        let mut state = ActorSupervisorState::new();

        for _ in 0..config.max_restart_attempts {
            state.record_restart();
            metrics.record_restart_attempt();
        }

        let decision = compute_restart_decision(&state, &config);
        if decision.should_isolate() {
            metrics.record_isolation();
        }

        assert_eq!(metrics.get_isolation_count(), 1);
    }
}

// =============================================================================
// SCENARIO 4: State recovery after supervisor restart
// =============================================================================

mod state_recovery_after_restart {
    use super::*;

    #[test]
    fn rq_supervisor_state_preserves_last_known_good_state_on_panic() {
        let mut state = ActorSupervisorState::with_running();
        state.record_panic("thread 'main' panicked at 'assertion failed'\n  at foo.rs:42".to_string());

        assert!(state.last_panic_at.is_some());
        assert_eq!(
            state.last_known_good_state.as_deref(),
            Some("thread 'main' panicked at 'assertion failed'\n  at foo.rs:42")
        );
    }

    #[test]
    fn rq_supervisor_state_transitions_lifecycle_on_restart() {
        let mut state = ActorSupervisorState::new();
        assert_eq!(state.lifecycle_state, ActorLifecycleState::Pending);

        state.record_panic("crash!".to_string());
        state.lifecycle_state = ActorLifecycleState::Failed;

        state.record_restart();
        assert_eq!(state.lifecycle_state, ActorLifecycleState::Running,
            "after restart, lifecycle must be Running");
        assert_eq!(state.restart_attempts, 1);
    }

    #[test]
    fn rq_multiple_panic_restart_cycles_accumulate_state() {
        let mut state = ActorSupervisorState::with_running();
        let config = ActorSupervisorConfig {
            max_restart_attempts: 3,
            ..Default::default()
        };

        // Cycle 1: panic → restart
        state.record_panic("first crash".to_string());
        state.record_restart();
        assert_eq!(state.restart_attempts, 1);
        assert!(compute_restart_decision(&state, &config).should_restart());

        // Cycle 2: panic → restart
        state.record_panic("second crash".to_string());
        state.record_restart();
        assert_eq!(state.restart_attempts, 2);
        assert!(compute_restart_decision(&state, &config).should_restart());

        // Cycle 3: panic → isolate (max reached)
        state.record_panic("third crash".to_string());
        state.record_restart();
        assert_eq!(state.restart_attempts, 3);
        let decision = compute_restart_decision(&state, &config);
        assert!(decision.should_isolate(), "after 3 restarts, must isolate");
    }

    #[test]
    fn rq_last_known_good_state_updated_on_each_panic() {
        let mut state = ActorSupervisorState::new();

        state.record_panic("state A".to_string());
        assert_eq!(state.last_known_good_state.as_deref(), Some("state A"));

        state.record_panic("state B".to_string());
        assert_eq!(state.last_known_good_state.as_deref(), Some("state B"),
            "last_known_good_state must be overwritten on each panic");
    }

    #[test]
    fn rq_panic_catcher_preserves_message_across_different_payload_types() {
        let metrics = ActorSupervisorMetrics::new();
        let id = test_instance_id(1);

        // String panic
        let result = PanicCatcher::catch_panic(id.clone(), || {
            panic!("string message");
        }, &metrics);
        if let Err(ActorSupervisorError::ActorPanic { panic_message, .. }) = result {
            assert_eq!(panic_message, "string message");
        } else {
            panic!("expected ActorPanic with string message");
        }
    }

    #[test]
    fn rq_audit_entry_records_restart_sequence() {
        let id = test_instance_id(1);

        let entry = ActorSupervisorAuditEntry::new_restart(
            id.clone(),
            2,
            Some(200),
            Some("running".to_string()),
        );

        assert_eq!(entry.event_type, ActorSupervisorEventType::ActorRestart);
        match entry.details {
            ActorSupervisorAuditDetails::Restart {
                restart_attempt,
                backoff_ms,
                previous_state,
            } => {
                assert_eq!(restart_attempt, 2);
                assert_eq!(backoff_ms, Some(200));
                assert_eq!(previous_state, Some("running".to_string()));
            }
            other => panic!("expected Restart details, got {:?}", other),
        }
    }

    #[test]
    fn rq_audit_entry_records_isolation_with_final_state() {
        let id = test_instance_id(1);

        let entry = ActorSupervisorAuditEntry::new_isolation(
            id.clone(),
            3,
            3,
            Some("last good state json".to_string()),
        );

        assert_eq!(entry.event_type, ActorSupervisorEventType::ActorIsolation);
        match entry.details {
            ActorSupervisorAuditDetails::Isolation {
                total_restart_attempts,
                max_attempts,
                last_known_good_state,
            } => {
                assert_eq!(total_restart_attempts, 3);
                assert_eq!(max_attempts, 3);
                assert_eq!(last_known_good_state, Some("last good state json".to_string()));
            }
            other => panic!("expected Isolation details, got {:?}", other),
        }
    }

    #[test]
    fn rq_audit_entry_records_panic_with_restart_context() {
        let id = test_instance_id(1);

        let entry = ActorSupervisorAuditEntry::new_panic(
            id.clone(),
            "assertion failed: count > 0".to_string(),
            true,
            2, // restart attempts before this panic
        );

        assert_eq!(entry.event_type, ActorSupervisorEventType::ActorPanic);
        match entry.details {
            ActorSupervisorAuditDetails::Panic {
                panic_message,
                backtrace_available,
                restart_attempts_before,
                ..
            } => {
                assert_eq!(panic_message, "assertion failed: count > 0");
                assert!(backtrace_available);
                assert_eq!(restart_attempts_before, 2);
            }
            other => panic!("expected Panic details, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn rq_child_registry_state_survives_concurrent_updates() {
        let registry = Arc::new(ParentChildRegistry::new());
        let mut handles = Vec::new();

        for i in 0..10u8 {
            let reg = registry.clone();
            handles.push(tokio::spawn(async move {
                let id = test_instance_id(i);
                reg.add_child(id.clone()).await;
                reg.update_child_state(&id, ActorLifecycleState::Running).await;
                reg.update_child_state(&id, ActorLifecycleState::Failed).await;
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        let children = registry.get_children().await;
        assert_eq!(children.len(), 10);
        assert!(registry.all_children_terminal().await);
    }

    #[test]
    fn rq_concurrent_panic_catches_are_independent() {
        let metrics = Arc::new(ActorSupervisorMetrics::new());
        let mut handles = Vec::new();

        for i in 0..10u8 {
            let m = metrics.clone();
            handles.push(std::thread::spawn(move || {
                let id = test_instance_id(i);
                let result = PanicCatcher::catch_panic(id, || {
                    panic!("thread {} crash", i);
                }, &m);
                assert!(result.is_err());
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(metrics.get_panic_count(), 10);
    }

    #[test]
    fn rq_successful_execution_does_not_touch_metrics() {
        let metrics = ActorSupervisorMetrics::new();
        let id = test_instance_id(1);

        let result = PanicCatcher::catch_panic(id, || 42, &metrics);
        assert_eq!(result.unwrap(), 42);
        assert_eq!(metrics.get_panic_count(), 0);
        assert_eq!(metrics.get_restart_count(), 0);
        assert_eq!(metrics.get_isolation_count(), 0);
    }
}

// =============================================================================
// ADVERSARIAL BOUNDARY: Overflow and edge cases
// =============================================================================

mod adversarial_boundaries {
    use super::*;

    #[test]
    fn rq_zero_max_restart_attempts_means_immediate_isolation() {
        let config = ActorSupervisorConfig {
            max_restart_attempts: 0,
            ..Default::default()
        };
        let state = ActorSupervisorState::new();
        let decision = compute_restart_decision(&state, &config);
        assert!(matches!(decision, RestartDecision::Isolate),
            "max_restart_attempts=0 must isolate immediately");
    }

    #[test]
    fn rq_zero_backoff_multiplier_still_restarts_now() {
        let mut state = ActorSupervisorState::new();
        state.restart_attempts = 5;
        let config = ActorSupervisorConfig {
            max_restart_attempts: 100,
            initial_backoff_ms: 100,
            backoff_multiplier: 0.0,
            max_backoff_ms: 30_000,
        };
        let decision = compute_restart_decision(&state, &config);
        // 100 * 0^4 = 0, which maps to RestartNow
        assert!(matches!(decision, RestartDecision::RestartNow),
            "zero multiplier collapses all backoff to zero");
    }

    #[test]
    fn rq_very_large_backoff_multiplier_capped_at_max() {
        let mut state = ActorSupervisorState::new();
        state.restart_attempts = 1;
        let config = ActorSupervisorConfig {
            max_restart_attempts: 100,
            initial_backoff_ms: 100,
            backoff_multiplier: 1e18,
            max_backoff_ms: 5000,
        };
        let decision = compute_restart_decision(&state, &config);
        match decision {
            RestartDecision::RestartWithBackoff(delay) => {
                assert!(delay <= 5000, "must be capped at max_backoff_ms, got {}", delay);
            }
            other => panic!("expected RestartWithBackoff, got {:?}", other),
        }
    }

    #[test]
    fn rq_single_max_restart_allows_one_restart_then_isolate() {
        let config = ActorSupervisorConfig {
            max_restart_attempts: 1,
            ..Default::default()
        };

        let state_fresh = ActorSupervisorState::new();
        assert!(compute_restart_decision(&state_fresh, &config).should_restart());

        let mut state_used = ActorSupervisorState::new();
        state_used.record_restart();
        assert!(compute_restart_decision(&state_used, &config).should_isolate());
    }

    #[test]
    fn rq_panic_catcher_never_suppresses_errors() {
        let metrics = ActorSupervisorMetrics::new();
        let id = test_instance_id(1);

        let result = PanicCatcher::catch_panic(id.clone(), || {
            panic!("must not be suppressed");
        }, &metrics);

        assert!(result.is_err(), "PanicCatcher must never return Ok on panic");
        let err = result.unwrap_err();
        assert!(err.to_string().contains("must not be suppressed"),
            "error message must propagate");
    }

    #[tokio::test]
    async fn rq_empty_registry_reports_all_children_terminal() {
        let registry = ParentChildRegistry::new();
        assert!(registry.all_children_terminal().await,
            "empty registry vacuously has all children terminal");
        assert_eq!(registry.active_children_count().await, 0);
    }

    #[test]
    fn rq_panic_info_with_empty_backtrace_detected() {
        let id = test_instance_id(1);
        let info = PanicInfo::new(id, "msg".to_string(), "".to_string());
        assert!(!info.is_backtrace_available());
        assert_eq!(info.backtrace_status, "not captured");
    }

    #[test]
    fn rq_backoff_monotonically_non_decreasing() {
        let config = ActorSupervisorConfig {
            max_restart_attempts: 100,
            initial_backoff_ms: 100,
            backoff_multiplier: 2.0,
            max_backoff_ms: 30_000,
        };

        let mut last_backoff: u64 = 0;
        for attempts in 1..10 {
            let mut state = ActorSupervisorState::new();
            state.restart_attempts = attempts;
            let decision = compute_restart_decision(&state, &config);

            if let RestartDecision::RestartWithBackoff(delay) = decision {
                assert!(delay >= last_backoff,
                    "backoff must be non-decreasing: attempt {} gave {} < {}",
                    attempts, delay, last_backoff);
                last_backoff = delay;
            }
        }
    }

    #[test]
    fn rq_supervisor_not_running_error_is_never_transient_nor_fatal() {
        let id = test_instance_id(1);
        let error = ActorSupervisorError::SupervisorNotRunning { instance_id: id };
        assert!(!error.is_transient());
        assert!(!error.is_fatal(), "SupervisorNotRunning is operational, not fatal");
    }

    #[test]
    fn rq_invalid_state_transition_error_is_never_transient_nor_fatal() {
        let id = test_instance_id(1);
        let error = ActorSupervisorError::InvalidStateTransition {
            instance_id: id,
            reason: "bad transition".to_string(),
        };
        assert!(!error.is_transient());
        assert!(!error.is_fatal());
    }

    #[test]
    fn rq_actor_supervisor_state_default_matches_new() {
        let default = ActorSupervisorState::default();
        let new = ActorSupervisorState::new();
        assert_eq!(default.restart_attempts, new.restart_attempts);
        assert_eq!(default.lifecycle_state, new.lifecycle_state);
        assert_eq!(default.last_restart_at.is_none(), new.last_restart_at.is_none());
        assert_eq!(default.last_panic_at.is_none(), new.last_panic_at.is_none());
        assert_eq!(default.last_known_good_state.is_none(), new.last_known_good_state.is_none());
    }

    #[tokio::test]
    async fn rq_shutdown_propagator_drop_without_explicit_shutdown_runs_cleanup() {
        let executed = Arc::new(AtomicUsize::new(0));
        let exec_clone = executed.clone();

        {
            let propagator = ShutdownPropagator::default_propagator();
            propagator.register_drop_sync("cleanup", move || {
                exec_clone.fetch_add(1, SeqCst);
            });
            // Drop propagator without calling propagate()
        }

        assert_eq!(executed.load(SeqCst), 1,
            "Drop impl must execute cleanup actions");
    }

    #[test]
    fn rq_permanent_failure_audit_entry_created() {
        let id = test_instance_id(1);
        let entry = ActorSupervisorAuditEntry::new_permanent_failure(
            id,
            "unrecoverable corruption".to_string(),
            Some("failed".to_string()),
        );
        assert_eq!(entry.event_type, ActorSupervisorEventType::ActorPermanentFailure);
    }
}
