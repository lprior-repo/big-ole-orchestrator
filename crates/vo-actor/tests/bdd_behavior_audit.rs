#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::disallowed_methods)]
//! BDD Behavioral Contract Verification for vo-actor (ve-be5o8)
//!
//! Audit: verifies existing public API behavioral contracts.
//! Three lenses: Liar Check, Breakage Check, Completeness Check.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use vo_actor::fairness::{WorkloadClass, ALL_WORKLOAD_CLASSES};
use vo_actor::instance_registry::{
    InstanceActorHandle, InstanceRegistry, RegistryConfig, RegistryError,
};
use vo_actor::lifecycle::{
    compute_next_state, is_valid_transition, ActorLifecycleState, LifecycleTransition,
    ShutdownPropagator,
};
use vo_actor::message_router::{
    ActorDestination, ChannelId, DeadLetterQueue, DeadLetterReason, MessageMetadata, MessageRouter,
    RouteError, TimestampMs as RouterTimestampMs,
};
use vo_actor::probe::{
    AggregatedStatus, BackoffConfig, ProbeConfig, ProbeId, ProbeRegistry, ProbeStatus,
};
use vo_actor::reanimator::{
    calculate_batch_size, check_resume_budget, filter_timers_by_fairness, validate_timer_record,
    FairnessBudget, ReanimatorConfig, ReanimatorError, ReanimatorState,
    TimerRecord as ReanimatorTimerRecord,
};
use vo_actor::semaphore::{
    calculate_backpressure_status, estimate_wait_ms, is_workflow_saturated, BackpressureStatus,
    ExecutionSemaphore, SemaphoreConfig, WorkflowSemaphoreMap,
};
use vo_actor::signal_buffer::{can_buffer, SignalBuffer, SignalBufferConfig};
use vo_actor::spawn_supervisor::{
    calculate_backoff_delay, is_zombie_state, should_respawn, SpawnPhase, SpawnRecord,
    SpawnSupervisorError,
};
use vo_actor::timer_lifecycle::validate_timer_for_cancellation;
use vo_actor::timer_supervisor::{
    is_overdue, verify_dual_clock, Counter, TimerSupervisorError, TimerSupervisorMetrics,
};
use vo_types::BufferPolicy;
use vo_types::{InstanceId, TimerId};

fn make_id(s: &str) -> InstanceId {
    InstanceId::parse(s).unwrap()
}
fn make_instance(suffix: &str) -> InstanceId {
    make_id(&format!("01H5JYV4XHGSR2F8KZ9B000{}", suffix))
}
fn wf(name: &str) -> vo_types::WorkflowName {
    vo_types::WorkflowName::parse(name).unwrap()
}
fn timer_id(s: &str) -> TimerId {
    TimerId::parse(s).unwrap()
}

// Helper to create vo_types::TimestampMs for reanimator records
fn vts(v: u64) -> vo_types::TimestampMs {
    vo_types::TimestampMs::parse(&v.to_string()).unwrap()
}

// =============================================================================
// Module 1: fairness::WorkloadClass
// =============================================================================

mod bdd_fairness {
    use super::*;

    #[test]
    fn given_workload_class_when_display_then_lower_case() {
        assert_eq!(format!("{}", WorkloadClass::Recovery), "recovery");
        assert_eq!(format!("{}", WorkloadClass::NewInstance), "new_instance");
        assert_eq!(format!("{}", WorkloadClass::Internal), "internal");
    }

    #[test]
    fn given_valid_string_when_parse_then_correct_class() {
        use std::str::FromStr;
        assert_eq!(
            WorkloadClass::from_str("recovery").unwrap(),
            WorkloadClass::Recovery
        );
        assert_eq!(
            WorkloadClass::from_str("RECOVERY").unwrap(),
            WorkloadClass::Recovery
        );
        assert_eq!(
            WorkloadClass::from_str("new_instance").unwrap(),
            WorkloadClass::NewInstance
        );
        assert_eq!(
            WorkloadClass::from_str("newinstance").unwrap(),
            WorkloadClass::NewInstance
        );
        assert_eq!(
            WorkloadClass::from_str("internal").unwrap(),
            WorkloadClass::Internal
        );
    }

    #[test]
    fn given_invalid_string_when_parse_then_error() {
        use std::str::FromStr;
        assert!(WorkloadClass::from_str("invalid").is_err());
    }

    #[test]
    fn given_workload_class_when_default_then_internal() {
        assert_eq!(WorkloadClass::default(), WorkloadClass::Internal);
    }

    #[test]
    fn given_all_workload_classes_constant_then_covers_all() {
        assert_eq!(ALL_WORKLOAD_CLASSES.len(), 3);
        assert!(ALL_WORKLOAD_CLASSES.contains(&WorkloadClass::Recovery));
        assert!(ALL_WORKLOAD_CLASSES.contains(&WorkloadClass::NewInstance));
        assert!(ALL_WORKLOAD_CLASSES.contains(&WorkloadClass::Internal));
    }

    #[test]
    fn given_workload_class_when_copy_then_independent() {
        let a = WorkloadClass::Recovery;
        let b = a;
        assert_eq!(a, b);
    }
}

// =============================================================================
// Module 2: lifecycle — state machine transitions
// =============================================================================

mod bdd_lifecycle {
    use super::*;

    #[test]
    fn given_pending_when_start_then_running() {
        assert_eq!(
            compute_next_state(ActorLifecycleState::Pending, LifecycleTransition::Start),
            Some(ActorLifecycleState::Running)
        );
    }

    #[test]
    fn given_running_when_stop_then_stopping() {
        assert_eq!(
            compute_next_state(ActorLifecycleState::Running, LifecycleTransition::Stop),
            Some(ActorLifecycleState::Stopping)
        );
    }

    #[test]
    fn given_stopping_when_all_children_stopped_then_stopped() {
        assert_eq!(
            compute_next_state(
                ActorLifecycleState::Stopping,
                LifecycleTransition::AllChildrenStopped
            ),
            Some(ActorLifecycleState::Stopped)
        );
    }

    #[test]
    fn given_running_when_fail_then_failed() {
        assert_eq!(
            compute_next_state(ActorLifecycleState::Running, LifecycleTransition::Fail),
            Some(ActorLifecycleState::Failed)
        );
    }

    #[test]
    fn given_terminal_state_when_any_transition_then_none() {
        for state in [ActorLifecycleState::Stopped, ActorLifecycleState::Failed] {
            for t in [
                LifecycleTransition::Start,
                LifecycleTransition::Stop,
                LifecycleTransition::Fail,
                LifecycleTransition::ChildStopped,
                LifecycleTransition::AllChildrenStopped,
            ] {
                assert_eq!(
                    compute_next_state(state, t),
                    None,
                    "Terminal {:?} should reject {:?}",
                    state,
                    t
                );
            }
        }
    }

    #[test]
    fn given_state_when_is_terminal_then_only_stopped_and_failed() {
        assert!(!ActorLifecycleState::Pending.is_terminal());
        assert!(!ActorLifecycleState::Running.is_terminal());
        assert!(!ActorLifecycleState::Stopping.is_terminal());
        assert!(ActorLifecycleState::Stopped.is_terminal());
        assert!(ActorLifecycleState::Failed.is_terminal());
    }

    #[test]
    fn given_state_when_can_spawn_child_then_pending_and_running() {
        assert!(ActorLifecycleState::Pending.can_spawn_child());
        assert!(ActorLifecycleState::Running.can_spawn_child());
        assert!(!ActorLifecycleState::Stopping.can_spawn_child());
        assert!(!ActorLifecycleState::Stopped.can_spawn_child());
        assert!(!ActorLifecycleState::Failed.can_spawn_child());
    }

    #[test]
    fn given_shutdown_propagator_when_default_then_reasonable_timeouts() {
        let prop = ShutdownPropagator::default_propagator();
        assert!(prop.graceful_timeout() > Duration::ZERO);
        assert!(prop.force_kill_timeout() > Duration::ZERO);
    }

    #[test]
    fn given_invalid_transition_when_check_then_false() {
        assert!(!is_valid_transition(
            ActorLifecycleState::Pending,
            LifecycleTransition::Stop
        ));
        assert!(!is_valid_transition(
            ActorLifecycleState::Stopped,
            LifecycleTransition::Start
        ));
    }
}

// =============================================================================
// Module 3: instance_registry — INV-1..INV-5
// =============================================================================

mod bdd_instance_registry {
    use super::*;

    #[test]
    fn given_empty_registry_when_lookup_then_none() {
        let reg = InstanceRegistry::new(RegistryConfig::default());
        assert!(reg.lookup(&make_id("01H5JYV4XHGSR2F8KZ9B000001")).is_none());
        assert_eq!(reg.active_count(), 0);
    }

    #[test]
    fn given_registry_when_register_then_active_count_consistent() {
        let mut reg = InstanceRegistry::new(RegistryConfig::default());
        let id = make_id("01H5JYV4XHGSR2F8KZ9B000001");
        reg.register(id.clone(), InstanceActorHandle::test(1), |_| Ok(()))
            .unwrap();
        assert_eq!(reg.active_count(), 1);
        assert!(reg.is_active(&id));
    }

    #[test]
    fn given_registry_when_deregister_then_count_decreases() {
        let mut reg = InstanceRegistry::new(RegistryConfig::default());
        let id = make_id("01H5JYV4XHGSR2F8KZ9B000001");
        reg.register(id.clone(), InstanceActorHandle::test(1), |_| Ok(()))
            .unwrap();
        reg.deregister(&id).unwrap();
        assert_eq!(reg.active_count(), 0);
        assert!(!reg.is_active(&id));
    }

    #[test]
    fn given_registry_when_deregister_unknown_then_error() {
        let mut reg = InstanceRegistry::new(RegistryConfig::default());
        let result = reg.deregister(&make_id("01H5JYV4XHGSR2F8KZ9B000001"));
        assert!(matches!(result, Err(RegistryError::NotRegistered { .. })));
    }

    #[test]
    fn given_registry_when_stop_before_replace_then_prior_stopped() {
        let mut reg = InstanceRegistry::new(RegistryConfig::default());
        let id = make_id("01H5JYV4XHGSR2F8KZ9B000001");
        let count = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let sc = count.clone();
        reg.register(id.clone(), InstanceActorHandle::test(1), move |_| {
            sc.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        })
        .unwrap();
        let sc2 = count.clone();
        reg.register(id.clone(), InstanceActorHandle::test(2), move |_| {
            sc2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        })
        .unwrap();
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(reg.active_count(), 1);
    }

    #[test]
    fn given_registry_when_stop_fn_fails_then_no_partial_mutation() {
        let mut reg = InstanceRegistry::new(RegistryConfig::default());
        let id = make_id("01H5JYV4XHGSR2F8KZ9B000001");
        reg.register(id.clone(), InstanceActorHandle::test(1), |_| Ok(()))
            .unwrap();
        let result = reg.register(id.clone(), InstanceActorHandle::test(2), |_| {
            Err("forced stop failure".to_string())
        });
        assert!(result.is_err());
        assert_eq!(reg.active_count(), 1);
        assert_eq!(reg.lookup(&id).unwrap().handle_id(), 1);
    }

    #[test]
    fn given_config_when_zero_timeout_then_panics() {
        let result = std::panic::catch_unwind(|| {
            InstanceRegistry::new(RegistryConfig {
                stop_timeout: Duration::ZERO,
            });
        });
        assert!(result.is_err());
    }
}

// =============================================================================
// Module 4: semaphore — backpressure, execution, workflow maps
// =============================================================================

mod bdd_semaphore {
    use super::*;

    // --- Backpressure calc ---

    #[test]
    fn given_high_available_when_backpressure_then_healthy() {
        assert_eq!(
            calculate_backpressure_status(400, 500, 0, 5000),
            BackpressureStatus::Healthy
        );
    }

    #[test]
    fn given_exceeded_waiters_when_backpressure_then_shed() {
        let status = calculate_backpressure_status(100, 500, 5000, 5000);
        assert_eq!(status, BackpressureStatus::ShedLoad);
        assert!(status.should_reject());
    }

    #[test]
    fn given_backpressure_when_should_reject_then_only_shed() {
        assert!(!BackpressureStatus::Healthy.should_reject());
        assert!(!BackpressureStatus::Moderate.should_reject());
        assert!(!BackpressureStatus::Heavy.should_reject());
        assert!(BackpressureStatus::ShedLoad.should_reject());
    }

    #[test]
    fn given_backpressure_when_is_queued_then_heavy_and_shed() {
        assert!(!BackpressureStatus::Healthy.is_queued());
        assert!(!BackpressureStatus::Moderate.is_queued());
        assert!(BackpressureStatus::Heavy.is_queued());
        assert!(BackpressureStatus::ShedLoad.is_queued());
    }

    #[test]
    fn given_ordering_when_compare_then_monotonic() {
        assert!(BackpressureStatus::Healthy < BackpressureStatus::Moderate);
        assert!(BackpressureStatus::Moderate < BackpressureStatus::Heavy);
        assert!(BackpressureStatus::Heavy < BackpressureStatus::ShedLoad);
    }

    #[test]
    fn given_position_when_estimate_wait_then_positive() {
        assert!(estimate_wait_ms(5, 2, 1000) > 0);
        assert!(estimate_wait_ms(0, 2, 1000) >= 0);
    }

    #[test]
    fn given_saturated_when_check_then_true() {
        assert!(is_workflow_saturated(10, 10));
        assert!(is_workflow_saturated(11, 10));
        assert!(!is_workflow_saturated(9, 10));
    }

    // --- ExecutionSemaphore ---

    #[test]
    fn given_fresh_semaphore_when_try_acquire_then_some() {
        assert!(ExecutionSemaphore::default().try_acquire().is_some());
    }

    #[test]
    fn given_exhausted_semaphore_when_try_acquire_then_none() {
        let config = SemaphoreConfig {
            max_concurrent_binaries: 1,
            ..SemaphoreConfig::default()
        };
        let sem = ExecutionSemaphore::new(config);
        let _p = sem.try_acquire();
        assert!(sem.try_acquire().is_none());
    }

    #[test]
    fn given_permit_when_dropped_then_available() {
        let config = SemaphoreConfig {
            max_concurrent_binaries: 1,
            ..SemaphoreConfig::default()
        };
        let sem = ExecutionSemaphore::new(config);
        {
            let _p = sem.try_acquire();
        }
        assert!(sem.try_acquire().is_some());
    }

    #[test]
    fn given_reserved_acquire_then_doesnt_consume_general() {
        let sem = ExecutionSemaphore::default();
        let before = sem.available_permits();
        let _r = sem.try_acquire_recovery();
        assert_eq!(sem.available_permits(), before);
    }

    #[test]
    fn given_reserved_exhausted_when_try_recovery_then_none() {
        let config = SemaphoreConfig {
            reserved_permits: 1,
            ..SemaphoreConfig::default()
        };
        let sem = ExecutionSemaphore::new(config);
        let _p = sem.try_acquire_recovery();
        assert!(sem.try_acquire_recovery().is_none());
    }

    #[test]
    fn given_reserved_dropped_when_try_recovery_then_available() {
        let config = SemaphoreConfig {
            reserved_permits: 1,
            ..SemaphoreConfig::default()
        };
        let sem = ExecutionSemaphore::new(config);
        {
            let _p = sem.try_acquire_recovery();
        }
        assert!(sem.try_acquire_recovery().is_some());
    }

    #[test]
    fn given_semaphore_when_status_queried_then_consistent() {
        let sem = ExecutionSemaphore::default();
        assert!(sem.available_permits() <= sem.total_permits());
        assert_eq!(sem.waiting_count(), 0);
    }

    // --- WorkflowSemaphoreMap ---

    #[test]
    fn given_map_when_semaphore_for_then_creates() {
        let map = WorkflowSemaphoreMap::new(5);
        let sem = map.semaphore_for(&wf("wf-1"));
        assert_eq!(sem.available_permits(), 5);
    }

    #[test]
    fn given_map_when_same_workflow_twice_then_same_semaphore() {
        let map = WorkflowSemaphoreMap::new(5);
        let s1 = map.semaphore_for(&wf("wf-1"));
        let _p = s1.try_acquire();
        let s2 = map.semaphore_for(&wf("wf-1"));
        assert_eq!(s2.available_permits(), 4);
    }

    #[test]
    fn given_map_when_different_workflows_then_independent() {
        let map = WorkflowSemaphoreMap::new(2);
        let s1 = map.semaphore_for(&wf("wf-1"));
        let _p = s1.try_acquire();
        let s2 = map.semaphore_for(&wf("wf-2"));
        assert_eq!(s2.available_permits(), 2);
    }

    #[test]
    fn given_map_when_len_then_counts_unique() {
        let map = WorkflowSemaphoreMap::new(5);
        assert!(map.is_empty());
        map.semaphore_for(&wf("wf-1"));
        map.semaphore_for(&wf("wf-2"));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn given_default_map_then_uses_default_max() {
        let map = WorkflowSemaphoreMap::default();
        let sem = map.semaphore_for(&wf("wf"));
        assert_eq!(
            sem.available_permits(),
            vo_actor::semaphore::DEFAULT_MAX_PER_WORKFLOW
        );
    }
}

// =============================================================================
// Module 5: message_router — channels, DLQ, routing
// =============================================================================

mod bdd_message_router {
    use super::*;
    use vo_actor::message_router::{ActorDestination, DeadLetterEntry, DeadLetterMessage};

    #[test]
    fn given_channel_id_when_empty_then_parse_fails() {
        assert!(ChannelId::parse("").is_err());
    }

    #[test]
    fn given_channel_id_when_valid_then_succeeds() {
        assert_eq!(ChannelId::parse("test-ch").unwrap().as_str(), "test-ch");
    }

    #[test]
    fn given_router_when_register_channel_then_has_channel() {
        let mut router = MessageRouter::with_default_config();
        router
            .register_channel(ChannelId::new("ch-1"), ActorDestination::test())
            .unwrap();
        assert!(router.has_channel(&ChannelId::new("ch-1")));
        assert_eq!(router.num_channels(), 1);
    }

    #[test]
    fn given_router_when_duplicate_channel_then_error() {
        let mut router = MessageRouter::with_default_config();
        let dest = ActorDestination::test();
        router
            .register_channel(ChannelId::new("ch-1"), dest.clone())
            .unwrap();
        assert!(matches!(
            router.register_channel(ChannelId::new("ch-1"), dest),
            Err(RouteError::ChannelAlreadyExists(_))
        ));
    }

    #[test]
    fn given_router_when_unregister_then_removed() {
        let mut router = MessageRouter::with_default_config();
        router
            .register_channel(ChannelId::new("ch-1"), ActorDestination::test())
            .unwrap();
        router.unregister_channel(&ChannelId::new("ch-1"));
        assert!(!router.has_channel(&ChannelId::new("ch-1")));
    }

    #[test]
    fn given_router_when_deactivate_then_not_active() {
        let mut router = MessageRouter::with_default_config();
        router
            .register_channel(ChannelId::new("ch-1"), ActorDestination::test())
            .unwrap();
        router.deactivate_channel(&ChannelId::new("ch-1")).unwrap();
        assert!(!router.is_channel_active(&ChannelId::new("ch-1")));
    }

    #[test]
    fn given_metadata_when_default_then_auto_generated() {
        let meta = MessageMetadata::default();
        assert!(!meta.message_id.is_empty());
        assert!(meta.timestamp.as_i64() > 0);
        assert_eq!(meta.attempt, 0);
    }

    #[test]
    fn given_metadata_when_increment_attempt_then_one() {
        assert_eq!(
            MessageMetadata::default()
                .with_incremented_attempt()
                .attempt,
            1
        );
    }

    #[test]
    fn given_dlq_when_enqueue_beyond_max_then_evicts_oldest() {
        let mut dlq = DeadLetterQueue::new(2);
        for i in 0..3 {
            dlq.enqueue(DeadLetterEntry {
                channel_id: ChannelId::new(format!("ch-{}", i)),
                message: DeadLetterMessage::new(&format!("msg-{}", i)).unwrap(),
                enqueued_at: RouterTimestampMs::now(),
                reason: DeadLetterReason::ChannelNotFound,
            });
        }
        assert_eq!(dlq.len(), 2);
        assert!(dlq
            .entries()
            .iter()
            .all(|e| e.channel_id.as_str() != "ch-0"));
    }

    #[test]
    fn given_dlq_when_clear_then_emptied() {
        let mut dlq = DeadLetterQueue::new(10);
        dlq.enqueue(DeadLetterEntry {
            channel_id: ChannelId::new("ch-1"),
            message: DeadLetterMessage::new(&"msg").unwrap(),
            enqueued_at: RouterTimestampMs::now(),
            reason: DeadLetterReason::ChannelNotFound,
        });
        dlq.clear();
        assert!(dlq.is_empty());
    }

    #[test]
    fn given_dlq_when_empty_then_dequeue_none() {
        assert!(DeadLetterQueue::new(10).dequeue().is_none());
    }
}

// =============================================================================
// Module 6: signal_buffer — policy-based buffering
// =============================================================================

mod bdd_signal_buffer {
    use super::*;
    use vo_actor::signal_buffer::BufferedSignal;
    use vo_actor::{SignalPayload, WaitKey};

    fn buf_id() -> InstanceId {
        make_id("01H5JYV4XHGSR2F8KZ9B000001")
    }

    #[test]
    fn given_buffer_when_buffer_signal_then_count_increases() {
        let mut buf = SignalBuffer::with_default_config();
        let id = buf_id();
        let wk = WaitKey::parse("test-key").unwrap();
        buf.buffer_signal(
            id.clone(),
            wk.clone(),
            BufferedSignal::new("s1", SignalPayload::empty(), vts(1)),
            BufferPolicy::BufferOne,
        );
        assert_eq!(buf.buffered_count(&id, &wk), 1);
    }

    #[test]
    fn given_buffered_when_pop_then_returns_and_removes() {
        let mut buf = SignalBuffer::with_default_config();
        let id = buf_id();
        let wk = WaitKey::parse("test-key").unwrap();
        buf.buffer_signal(
            id.clone(),
            wk.clone(),
            BufferedSignal::new("s1", SignalPayload::empty(), vts(1)),
            BufferPolicy::BufferOne,
        );
        assert!(buf.pop_buffered(&id, &wk).is_some());
        assert_eq!(buf.buffered_count(&id, &wk), 0);
    }

    #[test]
    fn given_buffer_when_clear_then_empty() {
        let mut buf = SignalBuffer::with_default_config();
        let id = buf_id();
        let wk = WaitKey::parse("test-key").unwrap();
        buf.buffer_signal(
            id.clone(),
            wk.clone(),
            BufferedSignal::new("s1", SignalPayload::empty(), vts(1)),
            BufferPolicy::BufferOne,
        );
        buf.clear(&id, &wk);
        assert_eq!(buf.buffered_count(&id, &wk), 0);
    }

    #[test]
    fn given_config_when_new_clamps_to_minimum_one() {
        assert_eq!(SignalBufferConfig::new(0).max_buffered_per_key, 1);
    }

    #[test]
    fn given_can_buffer_when_reject_then_false_and_buffer_many_at_capacity_then_false() {
        // Reject policy never allows buffering
        assert!(!can_buffer(
            BufferPolicy::Reject,
            false,
            0,
            &SignalBufferConfig::new(100)
        ));
        // BufferMany respects max_buffered_per_key
        assert!(!can_buffer(
            BufferPolicy::BufferMany,
            true,
            100,
            &SignalBufferConfig::new(1)
        ));
        // BufferOne always returns true (unconditional)
        assert!(can_buffer(
            BufferPolicy::BufferOne,
            true,
            100,
            &SignalBufferConfig::new(1)
        ));
    }

    #[test]
    fn given_multiple_keys_when_total_count_then_summed() {
        let mut buf = SignalBuffer::with_default_config();
        let id = buf_id();
        let wk1 = WaitKey::parse("key-1").unwrap();
        let wk2 = WaitKey::parse("key-2").unwrap();
        buf.buffer_signal(
            id.clone(),
            wk1,
            BufferedSignal::new("s1", SignalPayload::empty(), vts(1)),
            BufferPolicy::BufferOne,
        );
        buf.buffer_signal(
            id.clone(),
            wk2,
            BufferedSignal::new("s2", SignalPayload::empty(), vts(1)),
            BufferPolicy::BufferOne,
        );
        assert_eq!(buf.total_buffered_count(), 2);
        assert_eq!(buf.num_keys_with_signals(), 2);
    }
}

// =============================================================================
// Module 7: probe — health checks and registry
// =============================================================================

mod bdd_probe {
    use super::*;
    use vo_actor::probe::ProbeDefinition;

    #[test]
    fn given_probe_id_when_new_then_unique() {
        assert_ne!(ProbeId::new(), ProbeId::new());
    }

    #[test]
    fn given_registry_when_register_then_present() {
        let mut reg = ProbeRegistry::new();
        assert!(reg.is_empty());
        reg.register(ProbeDefinition {
            id: ProbeId::new(),
            name: "test".into(),
            config: ProbeConfig::http("http://localhost/health"),
            interval: Duration::from_secs(10),
            backoff: BackoffConfig::default(),
            failure_threshold: 3,
            success_threshold: 2,
        });
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn given_registry_when_unregister_then_removed() {
        let mut reg = ProbeRegistry::new();
        let id = ProbeId::new();
        reg.register(ProbeDefinition {
            id,
            name: "test".into(),
            config: ProbeConfig::http("http://localhost/health"),
            interval: Duration::from_secs(10),
            backoff: BackoffConfig::default(),
            failure_threshold: 3,
            success_threshold: 2,
        });
        reg.unregister(id);
        assert!(reg.is_empty());
    }

    #[test]
    fn given_backoff_when_calculate_then_exponential() {
        let config = BackoffConfig::default();
        assert!(config.calculate_interval(2) > config.calculate_interval(1));
        assert!(config.calculate_interval(3) > config.calculate_interval(2));
    }

    #[test]
    fn given_aggregated_when_healthy_then_is_healthy() {
        let mut status = AggregatedStatus::new();
        status.update(vo_actor::probe::ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Healthy,
            latency_ms: 10,
            consecutive_failures: 0,
            last_check_ms: 100,
            message: None,
        });
        assert!(status.is_healthy());
    }

    #[test]
    fn given_probe_config_when_probe_type_then_correct() {
        assert_eq!(
            ProbeConfig::http("http://x").probe_type(),
            vo_actor::probe::ProbeType::Http
        );
        assert_eq!(
            ProbeConfig::tcp("127.0.0.1", 8080).probe_type(),
            vo_actor::probe::ProbeType::Tcp
        );
        assert_eq!(
            ProbeConfig::exec("ls", vec![]).probe_type(),
            vo_actor::probe::ProbeType::Exec
        );
    }

    #[test]
    fn given_probe_config_when_timeout_then_correct() {
        assert_eq!(
            ProbeConfig::http("http://x")
                .with_timeout(Duration::from_secs(5))
                .timeout(),
            Duration::from_secs(5)
        );
    }
}

// =============================================================================
// Module 8: spawn_supervisor — backoff, zombie, respawn
// =============================================================================

mod bdd_spawn_supervisor {
    use super::*;

    fn test_spawn_record() -> SpawnRecord {
        SpawnRecord::new(
            make_id("01H5JYV4XHGSR2F8KZ9B000001"),
            std::path::PathBuf::from("bin"),
            vec![],
            None,
        )
    }

    #[test]
    fn given_backoff_when_calculate_then_exponential() {
        assert_eq!(calculate_backoff_delay(100, 2.0, 1), 100);
        assert_eq!(calculate_backoff_delay(100, 2.0, 2), 200);
        assert_eq!(calculate_backoff_delay(100, 2.0, 3), 400);
    }

    #[test]
    fn given_record_when_failed_many_attempts_then_zombie() {
        let mut r = test_spawn_record();
        r.spawn_attempts = 4;
        r.spawn_phase = SpawnPhase::Failed;
        assert!(is_zombie_state(&r));
    }

    #[test]
    fn given_record_when_failed_few_attempts_then_not_zombie() {
        let mut r = test_spawn_record();
        r.spawn_attempts = 2;
        r.spawn_phase = SpawnPhase::Failed;
        assert!(!is_zombie_state(&r));
    }

    #[test]
    fn given_record_when_not_failed_then_not_zombie() {
        assert!(!is_zombie_state(&test_spawn_record()));
    }

    #[test]
    fn given_record_when_should_respawn_under_max_then_true() {
        let mut r = test_spawn_record();
        r.spawn_phase = SpawnPhase::Failed;
        r.spawn_attempts = 2;
        assert!(should_respawn(&r, 5));
    }

    #[test]
    fn given_record_when_should_respawn_at_max_then_false() {
        let mut r = test_spawn_record();
        r.spawn_phase = SpawnPhase::Failed;
        r.spawn_attempts = 5;
        assert!(!should_respawn(&r, 5));
    }

    #[test]
    fn given_record_when_respawn_then_attempts_incremented() {
        let mut r = test_spawn_record();
        r.spawn_attempts = 1;
        let respawned = r.respawn(None);
        assert_eq!(respawned.spawn_attempts, 2);
    }

    #[test]
    fn given_record_when_transitions_then_phases_correct() {
        let r = test_spawn_record();
        assert_eq!(r.spawn_phase, SpawnPhase::Spawn);
        assert_eq!(
            r.transition_to_health_check().spawn_phase,
            SpawnPhase::HealthCheck
        );
        assert_eq!(
            r.transition_to_health_check()
                .transition_to_running()
                .spawn_phase,
            SpawnPhase::Running
        );
    }

    #[test]
    fn given_error_when_transient_then_correct() {
        assert!(SpawnSupervisorError::StorageError("x".into()).is_transient());
    }

    #[test]
    fn given_error_when_resumable_then_correct() {
        let id = make_id("01H5JYV4XHGSR2F8KZ9B000001");
        assert!(SpawnSupervisorError::SpawnFailed {
            command: "c".into(),
            error: "x".into()
        }
        .is_resumable());
        assert!(SpawnSupervisorError::HealthCheckFailed {
            instance_id: id,
            check_number: 1,
            error: "x".into()
        }
        .is_resumable());
    }

    #[test]
    fn given_error_when_fatal_then_correct() {
        assert!(SpawnSupervisorError::CorruptSpawn("x".into()).is_fatal());
        assert!(SpawnSupervisorError::InvalidConfig("x".into()).is_fatal());
    }

    #[test]
    fn given_error_when_operational_then_correct() {
        assert!(SpawnSupervisorError::AlreadyRunning.is_operational());
    }
}

// =============================================================================
// Module 9: timer_supervisor — dual-clock, overdue, counters
// =============================================================================

mod bdd_timer_supervisor {
    use super::*;

    #[test]
    fn given_both_past_when_dual_clock_then_verified() {
        assert!(verify_dual_clock(100, 90, 20, 110));
    }

    #[test]
    fn given_wall_past_mono_not_when_dual_clock_then_not_verified() {
        assert!(!verify_dual_clock(100, 90, 50, 110));
    }

    #[test]
    fn given_mono_past_wall_not_when_dual_clock_then_not_verified() {
        assert!(!verify_dual_clock(120, 90, 20, 110));
    }

    #[test]
    fn given_neither_past_when_dual_clock_then_not_verified() {
        assert!(!verify_dual_clock(200, 90, 20, 110));
    }

    #[test]
    fn given_overdue_fire_at_when_check_then_true() {
        assert!(is_overdue(100, 200, 10));
    }

    #[test]
    fn given_not_overdue_when_check_then_false() {
        assert!(!is_overdue(100, 105, 10));
    }

    #[test]
    fn given_counter_when_incr_then_increases() {
        let c = Counter::new();
        assert_eq!(c.get(), 0);
        c.incr();
        assert_eq!(c.get(), 1);
        c.incr();
        assert_eq!(c.get(), 2);
    }

    #[test]
    fn given_metrics_when_default_then_zero() {
        assert_eq!(TimerSupervisorMetrics::default().timers_fired.get(), 0);
    }

    #[test]
    fn given_supervisor_error_when_transient_then_correct() {
        assert!(TimerSupervisorError::StorageError("x".into()).is_transient());
    }

    #[test]
    fn given_supervisor_error_when_fatal_then_correct() {
        assert!(TimerSupervisorError::CorruptTimer("x".into()).is_fatal());
        assert!(TimerSupervisorError::InvalidConfig("x".into()).is_fatal());
    }
}

// =============================================================================
// Module 10: reanimator — FairnessBudget, validation, batch calc
// =============================================================================

mod bdd_reanimator {
    use super::*;

    #[test]
    fn given_fairness_when_can_resume_under_limit_then_true() {
        assert!(FairnessBudget::new().can_resume(&make_instance("001")));
    }

    #[test]
    fn given_fairness_when_record_resume_exceeds_limit_then_false() {
        let mut b = FairnessBudget::with_limits(1, 50);
        let id = make_instance("001");
        assert!(b.record_resume(id.clone()));
        assert!(!b.record_resume(id.clone()));
    }

    #[test]
    fn given_fairness_when_reset_then_cleared() {
        let mut b = FairnessBudget::with_limits(1, 50);
        let id = make_instance("001");
        b.record_resume(id.clone());
        b.reset();
        assert!(b.can_resume(&id));
    }

    #[test]
    fn given_fairness_when_zero_max_then_no_resumes() {
        assert!(!FairnessBudget::with_limits(0, 50).can_resume(&make_instance("001")));
    }

    #[test]
    fn given_timer_when_fire_at_zero_then_invalid() {
        let r = ReanimatorTimerRecord::new(make_instance("001"), vts(0), None, vts(100));
        assert!(validate_timer_record(&r).is_err());
    }

    #[test]
    fn given_timer_when_fire_at_before_scheduled_then_invalid() {
        let r = ReanimatorTimerRecord::new(make_instance("001"), vts(50), None, vts(100));
        assert!(validate_timer_record(&r).is_err());
    }

    #[test]
    fn given_timer_when_valid_then_ok() {
        let r = ReanimatorTimerRecord::new(make_instance("001"), vts(200), None, vts(100));
        assert!(validate_timer_record(&r).is_ok());
    }

    #[test]
    fn given_batch_when_remaining_exceeds_max_then_capped() {
        assert_eq!(calculate_batch_size(200, 100, 0), 100);
    }

    #[test]
    fn given_batch_when_remaining_under_max_then_remaining() {
        assert_eq!(calculate_batch_size(50, 100, 0), 50);
    }

    #[test]
    fn given_config_when_default_then_sane() {
        let c = ReanimatorConfig::default();
        assert_eq!(c.scan_interval, Duration::from_secs(1));
        assert_eq!(c.max_timers_per_cycle, 100);
    }

    #[test]
    fn given_state_when_is_active_then_running_and_shutting_down() {
        assert!(ReanimatorState::Running.is_active());
        assert!(ReanimatorState::ShuttingDown.is_active());
        assert!(!ReanimatorState::Stopped.is_active());
        assert!(!ReanimatorState::ShutDown.is_active());
    }

    #[test]
    fn given_filter_when_budget_exhausted_then_rejected() {
        let mut b = FairnessBudget::with_limits(1, 50);
        let id = make_instance("001");
        b.record_resume(id.clone());
        let timers = vec![ReanimatorTimerRecord::new(id, vts(200), None, vts(100))];
        let (allowed, rejected) = filter_timers_by_fairness(timers, &b);
        assert!(allowed.is_empty());
        assert_eq!(rejected.len(), 1);
    }

    #[test]
    fn given_reanimator_error_when_transient_then_correct() {
        assert!(ReanimatorError::StorageError("x".into()).is_transient());
        assert!(ReanimatorError::EnqueueFailed("x".into()).is_transient());
        assert!(ReanimatorError::AtomicityViolation("x".into()).is_transient());
        assert!(!ReanimatorError::CorruptKey("x".into()).is_transient());
    }

    #[test]
    fn given_reanimator_error_when_fatal_then_correct() {
        assert!(ReanimatorError::CorruptKey("x".into()).is_fatal());
        assert!(ReanimatorError::AlreadyRunning.is_fatal());
        assert!(ReanimatorError::AlreadyShutdown.is_fatal());
        assert!(!ReanimatorError::StorageError("x".into()).is_fatal());
    }

    #[test]
    fn given_resume_budget_when_exhausted_then_err() {
        let mut b = FairnessBudget::with_limits(0, 50);
        let id = make_instance("001");
        b.record_resume(id.clone());
        assert!(check_resume_budget(&id, &b).is_err());
    }
}

// =============================================================================
// Module 11: timer_lifecycle — cancellation validation
// =============================================================================

mod bdd_timer_lifecycle {
    use super::*;

    #[test]
    fn given_timer_matching_instance_when_validate_then_ok() {
        let id = make_id("01H5JYV4XHGSR2F8KZ9B000001");
        let timer =
            ReanimatorTimerRecord::new(id.clone(), vts(1000), Some(timer_id("t-1")), vts(900));
        assert!(validate_timer_for_cancellation(&timer, &id).is_ok());
    }

    #[test]
    fn given_timer_mismatched_instance_when_validate_then_error() {
        let id1 = make_id("01H5JYV4XHGSR2F8KZ9B000001");
        let id2 = make_id("01H5JYV4XHGSR2F8KZ9B000002");
        let timer = ReanimatorTimerRecord::new(id1, vts(1000), Some(timer_id("t-1")), vts(900));
        assert!(validate_timer_for_cancellation(&timer, &id2).is_err());
    }
}

// =============================================================================
// Module 12: Completeness — re-exports and TimestampMs
// =============================================================================

mod bdd_completeness {
    use super::*;

    #[test]
    fn given_lib_reexport_workload_class_then_accessible() {
        assert_eq!(vo_actor::WorkloadClass::Internal, WorkloadClass::Internal);
    }

    #[test]
    fn given_timestamp_ms_when_now_then_positive() {
        assert!(vts(1).as_u64() > 0);
    }
}
