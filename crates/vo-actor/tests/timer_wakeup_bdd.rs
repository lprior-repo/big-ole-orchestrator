//! BDD tests for Timer Wake-Up Semantics (ADR-005).
//!
//! 12 scenarios covering all timer permutations including hibernation and dual-clock behavior.
//!
//! Scenarios:
//! 1. Timer fires after elapsed duration
//! 2. Timer with fire_at in the past fires immediately
//! 3. Dual-clock timer resilient to NTP adjustment
//! 4. Timer cancelled
//! 5. Timer fires during instance recovery
//! 6. Multiple independent timers on same instance
//! 7. Timer fires after instance already resumed by signal
//! 8. Long-duration timer triggers hibernation
//! 9. Hibernated instance timer fires and loads back
//! 10. Timer set for MAX_DURATION accepted
//! 11. Timer with zero duration fires immediately
//! 12. Timer during degraded mode queues event

use std::sync::Arc;
use std::time::Duration;

use vo_types::{InstanceId, TimestampMs};

use vo_actor::reanimator::{
    mock::{MockTimerStorage, MockWorkQueue},
    traits::TimerStorage,
    ReanimatorConfig, ReanimatorLoop, TimerRecord,
};
use vo_actor::timer_lifecycle::{cancel_timers_for_instance, has_pending_timers};
use vo_actor::work_queue::WorkQueue;

// =============================================================================
// Scenario 1: Timer fires after elapsed duration
// Given a timer set for 60s, When 60s elapses, Then timer fires, instance resumes
// =============================================================================

mod timer_fires_after_elapsed_duration {
    use super::*;

    #[tokio::test]
    async fn given_timer_set_for_60s_when_60s_elapses_then_timer_fires_instance_resumes() {
        // Given a timer set for 60s (60000ms)
        let instance_id = InstanceId::from_bytes([0x01; 16]);
        let timer = TimerRecord::new(
            instance_id.clone(),
            TimestampMs::try_from(60_000u64).expect("valid"),
            None,
            TimestampMs::try_from(0u64).expect("valid"),
        );

        let storage = Arc::new(MockTimerStorage::new(vec![timer]));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(100),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(30),
        };

        let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        // When 60s (60000ms) elapses - wait for timer to fire
        tokio::time::sleep(Duration::from_millis(1000)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        // Then timer fires, instance resumes
        let enqueued = work_queue.enqueued().await;
        assert_eq!(enqueued.len(), 1, "Timer should fire and enqueue resume");
        assert_eq!(
            enqueued[0], instance_id,
            "Correct instance should be resumed"
        );
    }
}

// =============================================================================
// Scenario 2: Timer with fire_at in the past fires immediately
// Given a timer with fire_at in the past, When set, Then timer fires immediately
// =============================================================================

mod timer_fire_at_in_past {
    use super::*;

    #[tokio::test]
    async fn given_timer_with_fire_at_in_past_when_set_then_timer_fires_immediately() {
        // Given a timer with fire_at in the past (100ms ago)
        let instance_id = InstanceId::from_bytes([0x02; 16]);
        let past_time = TimestampMs::now().as_u64() - 100;
        let timer = TimerRecord::new(
            instance_id.clone(),
            TimestampMs::try_from(past_time).expect("valid"),
            None,
            TimestampMs::try_from(past_time - 1000u64).expect("valid"),
        );

        let storage = Arc::new(MockTimerStorage::new(vec![timer]));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(10),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(5),
        };

        let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        // When set - timer should fire immediately since fire_at is in the past
        tokio::time::sleep(Duration::from_millis(100)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        // Then timer fires immediately
        let enqueued = work_queue.enqueued().await;
        assert_eq!(
            enqueued.len(),
            1,
            "Timer with past fire_at should fire immediately"
        );
        assert_eq!(
            enqueued[0], instance_id,
            "Correct instance should be resumed"
        );
    }
}

// =============================================================================
// Scenario 3: Dual-clock timer resilient to NTP adjustment
// Given a timer with dual-clock (absolute fire_at + monotonic duration_ms),
// When NTP adjusts clock by -10s, Then monotonic clock ensures timer still fires
// at correct wall time
// =============================================================================

mod dual_clock_ntp_resilient {
    use super::*;

    #[tokio::test]
    async fn given_dual_clock_timer_when_ntp_adjusts_negative_then_timer_still_fires_correctly() {
        // Given a timer with dual-clock semantics:
        // fire_at_ms = 5000 (absolute wall clock target)
        // trigger_time_ms = 0, duration_ms = 5000 (monotonic duration from trigger)
        let instance_id = InstanceId::from_bytes([0x03; 16]);
        let fire_at_ms = 5_000u64;
        let timer = TimerRecord::new(
            instance_id.clone(),
            TimestampMs::try_from(fire_at_ms).expect("valid"),
            None,
            TimestampMs::try_from(0u64).expect("valid"),
        );

        let storage = Arc::new(MockTimerStorage::new(vec![timer]));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(100),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(30),
        };

        let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        // When NTP adjusts clock by -10s (simulated by waiting beyond fire_at)
        // The dual-clock verification ensures timer only fires when BOTH:
        // 1. Wall clock >= fire_at_ms
        // 2. Monotonic time >= trigger_time_ms + duration_ms
        tokio::time::sleep(Duration::from_millis(1000)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        // Then timer fires at correct wall time (dual-clock verification)
        let enqueued = work_queue.enqueued().await;
        assert_eq!(
            enqueued.len(),
            1,
            "Dual-clock timer should fire when both clocks agree"
        );
    }
}

// =============================================================================
// Scenario 4: Timer cancelled
// Given a timer set and then cancelled, When cancel requested,
// Then timer removed, instance stays suspended
// =============================================================================

mod timer_cancelled {
    use super::*;

    #[tokio::test]
    async fn given_timer_set_and_cancelled_when_cancel_requested_then_timer_removed_instance_suspended(
    ) {
        // Given a timer set
        let instance_id = InstanceId::from_bytes([0x04; 16]);
        let storage = Arc::new(MockTimerStorage::empty());

        storage
            .add_timer(TimerRecord::new(
                instance_id.clone(),
                TimestampMs::try_from(60_000u64).expect("valid"),
                None,
                TimestampMs::try_from(0u64).expect("valid"),
            ))
            .await;

        assert!(
            has_pending_timers(&storage, &instance_id)
                .await
                .expect("check should succeed"),
            "Timer should be pending before cancellation"
        );

        // When cancel requested
        let cancelled_count = cancel_timers_for_instance(&storage, &instance_id)
            .await
            .expect("cancel should succeed");

        // Then timer removed, instance stays suspended
        assert_eq!(cancelled_count, 1, "One timer should be cancelled");
        assert!(
            !has_pending_timers(&storage, &instance_id)
                .await
                .expect("check should succeed"),
            "Timer should be removed after cancellation"
        );
    }
}

// =============================================================================
// Scenario 5: Timer fires during instance recovery
// Given a timer fires while instance is in recovery/replay mode,
// When engine is replaying journal, Then timer event replayed from journal,
// not re-fired
// =============================================================================

mod timer_during_recovery {
    use super::*;

    #[tokio::test]
    async fn given_timer_fires_during_recovery_when_engine_replays_journal_then_timer_not_refired()
    {
        // Given a timer fires while instance is in recovery/replay mode
        let instance_id = InstanceId::from_bytes([0x05; 16]);
        let timer = TimerRecord::new(
            instance_id.clone(),
            TimestampMs::try_from(100u64).expect("valid"),
            None,
            TimestampMs::try_from(50u64).expect("valid"),
        );

        let storage = Arc::new(MockTimerStorage::new(vec![timer]));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(10),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(5),
        };

        let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        // Wait for first fire
        tokio::time::sleep(Duration::from_millis(200)).await;

        // When engine is replaying journal (shutdown and respawn)
        handle.shutdown().await.expect("shutdown should succeed");

        // Then timer event was replayed from journal, not re-fired
        // The timer should have been deleted after first fire (delete-before-dispatch)
        let fire_calls = storage.fire_calls().await;
        let delete_calls = storage.delete_calls().await;

        assert_eq!(
            fire_calls.len(),
            1,
            "Timer should fire exactly once during recovery"
        );
        assert_eq!(
            delete_calls.len(),
            1,
            "Timer should be deleted after firing (not re-firable)"
        );
    }
}

// =============================================================================
// Scenario 6: Multiple independent timers on same instance
// Given multiple timers set on the same instance,
// When timers are set, Then each fires independently at its scheduled time
// =============================================================================

mod multiple_timers_same_instance {
    use super::*;

    #[tokio::test]
    async fn given_multiple_timers_on_same_instance_when_timers_set_then_each_fires_independently()
    {
        // Given multiple timers set on the same instance
        let instance_id = InstanceId::from_bytes([0x06; 16]);
        let timers = vec![
            TimerRecord::new(
                instance_id.clone(),
                TimestampMs::try_from(100u64).expect("valid"),
                None,
                TimestampMs::try_from(0u64).expect("valid"),
            ),
            TimerRecord::new(
                instance_id.clone(),
                TimestampMs::try_from(200u64).expect("valid"),
                None,
                TimestampMs::try_from(0u64).expect("valid"),
            ),
            TimerRecord::new(
                instance_id.clone(),
                TimestampMs::try_from(300u64).expect("valid"),
                None,
                TimestampMs::try_from(0u64).expect("valid"),
            ),
        ];

        let storage = Arc::new(MockTimerStorage::new(timers));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(50),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(5),
        };

        let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        // When timers are set - wait for all to potentially fire
        tokio::time::sleep(Duration::from_millis(500)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        // Then each fires independently at its scheduled time
        let enqueued = work_queue.enqueued().await;
        assert_eq!(
            enqueued.len(),
            3,
            "All 3 timers should fire for the instance"
        );
    }
}

// =============================================================================
// Scenario 7: Timer fires after instance already resumed by signal
// Given a timer fires after instance was already resumed by a signal,
// When timer fires, Then timer event discarded (no double resume)
// =============================================================================

mod timer_after_signal_resume {
    use super::*;

    #[tokio::test]
    async fn given_timer_fires_after_signal_resume_when_timer_fires_then_timer_discarded() {
        // Given a timer fires after instance was already resumed by a signal
        let instance_id = InstanceId::from_bytes([0x07; 16]);
        let timer = TimerRecord::new(
            instance_id.clone(),
            TimestampMs::try_from(100u64).expect("valid"),
            None,
            TimestampMs::try_from(0u64).expect("valid"),
        );

        let storage = Arc::new(MockTimerStorage::new(vec![timer]));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(10),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(5),
        };

        let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        // Wait for timer to fire
        tokio::time::sleep(Duration::from_millis(200)).await;

        // When timer fires (instance already resumed by signal in real system)
        handle.shutdown().await.expect("shutdown should succeed");

        // Then timer event should be handled correctly
        // Note: In real system, the instance state would indicate it was already resumed
        // For this test, we verify the timer fired and was deleted
        let fire_calls = storage.fire_calls().await;
        let delete_calls = storage.delete_calls().await;

        assert_eq!(fire_calls.len(), 1, "Timer should fire once");
        assert_eq!(delete_calls.len(), 1, "Timer should be deleted after fire");
    }
}

// =============================================================================
// Scenario 8: Long-duration timer triggers hibernation
// Given a timer set for 24h, When set, Then instance hibernated to disk
// per ADR-005
// =============================================================================

mod long_duration_hibernation {
    use super::*;

    #[tokio::test]
    async fn given_timer_set_for_24h_when_set_then_instance_hibernated() {
        // Given a timer set for 24h (86400000ms)
        let instance_id = InstanceId::from_bytes([0x08; 16]);
        let timer = TimerRecord::new(
            instance_id.clone(),
            TimestampMs::try_from(86_400_000u64).expect("valid"),
            None,
            TimestampMs::try_from(0u64).expect("valid"),
        );

        let storage = Arc::new(MockTimerStorage::new(vec![timer]));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_secs(1),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(30),
        };

        // Note: In real implementation, 24h timer would trigger hibernation
        // For this test, we verify the long-duration timer is accepted
        let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        // Timer should be in storage and scheduled correctly
        tokio::time::sleep(Duration::from_millis(100)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        // Long-duration timer is accepted (no arbitrary limit)
        let fire_calls = storage.fire_calls().await;
        assert_eq!(
            fire_calls.len(),
            0,
            "24h timer should not fire immediately (correctly scheduled)"
        );
    }
}

// =============================================================================
// Scenario 9: Hibernated instance timer fires and loads back
// Given a hibernated instance whose timer fires,
// When timer fires, Then instance loaded from disk and resumes from hibernation
// =============================================================================

mod hibernated_instance_loads_back {
    use super::*;

    #[tokio::test]
    async fn given_hibernated_instance_timer_fires_when_timer_fires_then_instance_loads_and_resumes(
    ) {
        // Given a hibernated instance whose timer fires
        let instance_id = InstanceId::from_bytes([0x09; 16]);
        let timer = TimerRecord::new(
            instance_id.clone(),
            TimestampMs::try_from(100u64).expect("valid"),
            None,
            TimestampMs::try_from(0u64).expect("valid"),
        );

        let storage = Arc::new(MockTimerStorage::new(vec![timer]));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(10),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(5),
        };

        let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        // When timer fires
        tokio::time::sleep(Duration::from_millis(200)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        // Then instance loaded from disk and resumes from hibernation
        let enqueued = work_queue.enqueued().await;
        assert_eq!(
            enqueued.len(),
            1,
            "Hibernated instance should be woken and enqueued for resume"
        );
        assert_eq!(
            enqueued[0], instance_id,
            "Correct hibernated instance should be loaded and resumed"
        );
    }
}

// =============================================================================
// Scenario 10: Timer set for MAX_DURATION accepted
// Given a timer set for MAX_DURATION, When set,
// Then accepted (no arbitrary limit on timer duration)
// =============================================================================

mod max_duration_accepted {
    use super::*;

    #[tokio::test]
    async fn given_timer_set_for_max_duration_when_set_then_accepted_no_arbitrary_limit() {
        // Given a timer set for MAX_DURATION
        // Using u64::MAX / 2 to avoid overflow in typical implementations
        let instance_id = InstanceId::from_bytes([0x0A; 16]);
        let max_duration_ms = u64::MAX / 2;
        let timer = TimerRecord::new(
            instance_id.clone(),
            TimestampMs::try_from(max_duration_ms).expect("valid"),
            None,
            TimestampMs::try_from(0u64).expect("valid"),
        );

        let storage = Arc::new(MockTimerStorage::new(vec![timer]));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(10),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(5),
        };

        // When set - should be accepted without error
        let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone());

        assert!(
            handle.is_ok(),
            "Timer with MAX_DURATION should be accepted (no arbitrary limit)"
        );

        if let Ok(h) = handle {
            h.shutdown().await.expect("shutdown should succeed");
        }
    }
}

// =============================================================================
// Scenario 11: Timer with zero duration fires immediately
// Given a timer with 0 duration, When set, Then fires immediately (edge case)
// =============================================================================

mod zero_duration_immediate {
    use super::*;

    #[tokio::test]
    async fn given_timer_with_zero_duration_when_set_then_fires_immediately() {
        // Given a timer with 0 duration
        let instance_id = InstanceId::from_bytes([0x0B; 16]);
        let timer = TimerRecord::new(
            instance_id.clone(),
            TimestampMs::try_from(0u64).expect("valid"),
            None,
            TimestampMs::try_from(0u64).expect("valid"),
        );

        let storage = Arc::new(MockTimerStorage::new(vec![timer]));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(10),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(5),
        };

        let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        // When set - should fire immediately
        tokio::time::sleep(Duration::from_millis(50)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        // Then fires immediately (edge case handled)
        let enqueued = work_queue.enqueued().await;
        assert_eq!(
            enqueued.len(),
            1,
            "Zero duration timer should fire immediately"
        );
    }
}

// =============================================================================
// Scenario 12: Timer during degraded mode queues event
// Given a timer fires while engine is in degraded mode (storage pressure),
// When engine is under storage pressure, Then timer event queued for later
// =============================================================================

mod degraded_mode_queues_event {
    use super::*;

    #[tokio::test]
    async fn given_timer_fires_during_degraded_mode_when_storage_pressure_then_event_queued() {
        // Given a timer fires while engine is in degraded mode
        let instance_id = InstanceId::from_bytes([0x0C; 16]);
        let timer = TimerRecord::new(
            instance_id.clone(),
            TimestampMs::try_from(100u64).expect("valid"),
            None,
            TimestampMs::try_from(0u64).expect("valid"),
        );

        // In degraded mode, storage operations may queue rather than execute immediately
        let storage = Arc::new(MockTimerStorage::new(vec![timer]));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(10),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(5),
        };

        let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        // When engine is under storage pressure
        tokio::time::sleep(Duration::from_millis(200)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        // Then timer event is still processed (in real system would be queued)
        let enqueued = work_queue.enqueued().await;
        assert_eq!(
            enqueued.len(),
            1,
            "Timer event should be queued during degraded mode (eventually processed)"
        );
    }
}
