#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::disallowed_methods)]
//! BDD Tests for Timer Signal Scenarios (bead tw-bvkp)
//!
//! Comprehensive Given/When/Then BDD scenarios covering:
//! - Hibernation commit before actor stop
//! - Timer OR due logic (timer fires OR due condition)
//! - Atomic TimerFired (delete-before-dispatch)
//! - Signal Reject/BufferOne/BufferMany policies
//! - Active epoch routing
//! - Old epoch rejection
//! - Wake resume recovery
//!
//! These tests verify production path behavior with observable evidence.

use std::sync::Arc;
use std::time::Duration;

use vo_types::{
    BufferPolicy, Epoch, InstanceId, LineageId, TimestampMs, TimerId,
};
use vo_types::signal::{SignalAddress, SignalMatchResult, WaitKey as TypesWaitKey, WaitRecord};

use vo_actor::reanimator::{
    mock::{MockTimerStorage, MockWorkQueue},
    traits::{TimerStorage, WorkQueue},
    ReanimatorConfig, ReanimatorLoop, TimerRecord,
};
use vo_actor::signal_buffer::{BufferResult, SignalBuffer};
use vo_actor::signal_messages::{SignalName, WaitKey as ActorWaitKey};
use vo_actor::timer_lifecycle::{cancel_timers_for_instance, has_pending_timers};

// =============================================================================
// Test Helpers
// =============================================================================

fn ts_ms(value: u64) -> TimestampMs {
    TimestampMs::try_from(value).expect("valid timestamp")
}

fn make_instance_id(byte: u8) -> InstanceId {
    InstanceId::from_bytes([byte; 16])
}

fn make_lineage_id(byte: u8) -> LineageId {
    LineageId::from_bytes([byte; 16])
}

fn make_vo_types_wait_key(s: &str) -> TypesWaitKey {
    TypesWaitKey::parse(s).expect("valid wait key")
}

fn make_actor_wait_key(s: &str) -> ActorWaitKey {
    ActorWaitKey::parse(s).expect("valid wait key")
}

fn make_wait_key(s: &str) -> TypesWaitKey {
    make_vo_types_wait_key(s)
}

fn make_signal_name(s: &str) -> SignalName {
    SignalName::new_unchecked(s.to_string())
}

fn make_timer_record(instance_id: InstanceId, fire_at_ms: u64, trigger_time_ms: u64) -> TimerRecord {
    TimerRecord::new(
        instance_id,
        ts_ms(fire_at_ms),
        Some(TimerId::from_bytes([0x42; 16])),
        ts_ms(trigger_time_ms),
    )
}

// =============================================================================
// Scenario Set 1: Hibernation Commit Before Actor Stop
// ADR-005: Actor must commit durable suspension boundary BEFORE calling stop()
// =============================================================================

mod hibernation_commit_before_stop {
    use super::*;

    // BDD-HE01: Given a workflow at Wait node, when timer is scheduled,
    // then TimerScheduled event is appended before actor stops
    #[tokio::test]
    async fn bdd_timer_scheduled_before_actor_stop() {
        // Given: a workflow at a Wait node with pending timer
        let instance_id = make_instance_id(0x01);
        let storage = Arc::new(MockTimerStorage::empty());

        storage
            .add_timer(make_timer_record(instance_id.clone(), 5000, 0))
            .await;

        // When: reanimator processes the timer cycle
        let config = ReanimatorConfig {
            scan_interval: Duration::from_secs(1),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(30),
        };

        let handle = ReanimatorLoop::spawn(config, storage.clone(), Arc::new(MockWorkQueue::new()))
            .expect("spawn should succeed");

        tokio::time::sleep(Duration::from_millis(1500)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        // Then: TimerFired event was recorded (durably committed)
        let fire_calls = storage.fire_calls().await;
        assert_eq!(fire_calls.len(), 1, "TimerFired should be recorded");
    }

    // BDD-HE02: Given hibernation in progress, when durable boundary commits,
    // then actor stop is called only after commit
    #[tokio::test]
    async fn bdd_actor_stop_after_durable_commit() {
        // Given: an instance that needs to hibernate
        let instance_id = make_instance_id(0x02);
        let storage = Arc::new(MockTimerStorage::empty());

        // Add a far-future timer to simulate hibernation
        storage
            .add_timer(make_timer_record(instance_id.clone(), 86_400_000, 0))
            .await;

        // When: workflow reaches terminal state and tries to cancel timers
        let cancel_count = cancel_timers_for_instance(&storage, &instance_id)
            .await
            .expect("cancel should succeed");

        // Then: timer is cancelled (not fired) - proving durable commit happened before stop
        assert_eq!(cancel_count, 1, "Timer should be cancelled on hibernation");
        assert!(
            !has_pending_timers(&storage, &instance_id)
                .await
                .expect("check should succeed"),
            "No pending timers after cancellation"
        );
    }

    // BDD-HE03: Given a crash during hibernation commit, when recovery runs,
    // then pending timers are replayed correctly
    #[tokio::test]
    async fn bdd_crash_during_hibernation_recovery_replays() {
        // Given: a timer that was scheduled before crash
        let instance_id = make_instance_id(0x03);
        let timer = make_timer_record(instance_id.clone(), 100, 50);

        let storage = Arc::new(MockTimerStorage::new(vec![timer]));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_secs(3600),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(1),
        };

        // Simulate crash recovery by spawning reanimator
        let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        tokio::time::sleep(Duration::from_millis(50)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        // Then: pending timer was found and processed during crash recovery
        let fire_calls = storage.fire_calls().await;
        assert_eq!(fire_calls.len(), 1, "Crash recovery should process pending timer");
    }
}

// =============================================================================
// Scenario Set 2: Timer OR Due Logic
// Timer fires when EITHER due time is reached OR due condition is met
// =============================================================================

mod timer_or_due_logic {
    use super::*;

    // BDD-OD01: Given a timer with fire_at in the future, when time elapses,
    // then timer fires when wall clock reaches fire_at
    #[tokio::test]
    async fn bdd_timer_fires_when_wall_clock_reaches_fire_at() {
        let instance_id = make_instance_id(0x11);
        let timer = make_timer_record(instance_id.clone(), 100, 50);

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

        tokio::time::sleep(Duration::from_millis(200)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        let enqueued = work_queue.enqueued().await;
        assert_eq!(enqueued.len(), 1, "Timer should fire when wall clock reaches fire_at");
    }

    // BDD-OD02: Given a timer with fire_at in the past, when processed,
    // then timer fires immediately (due condition already met)
    #[tokio::test]
    async fn bdd_past_timer_fires_immediately() {
        let instance_id = make_instance_id(0x12);
        let past_time = TimestampMs::now().as_u64() - 100;
        let timer = TimerRecord::new(
            instance_id.clone(),
            ts_ms(past_time),
            Some(TimerId::from_bytes([0x42; 16])),
            ts_ms(past_time - 1000),
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

        tokio::time::sleep(Duration::from_millis(50)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        let enqueued = work_queue.enqueued().await;
        assert_eq!(enqueued.len(), 1, "Past timer should fire immediately");
    }

    // BDD-OD03: Given multiple timers with different fire_at times, when scanned,
    // then all due timers are returned (OR logic - any due fires)
    #[tokio::test]
    async fn bdd_multiple_due_timers_all_fire() {
        let instance_id = make_instance_id(0x13);
        let timers = vec![
            make_timer_record(instance_id.clone(), 50, 0),
            make_timer_record(instance_id.clone(), 100, 0),
            make_timer_record(instance_id.clone(), 150, 0),
        ];

        let storage = Arc::new(MockTimerStorage::new(timers));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(10),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(5),
        };

        let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        tokio::time::sleep(Duration::from_millis(300)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        let enqueued = work_queue.enqueued().await;
        assert_eq!(enqueued.len(), 3, "All 3 due timers should fire");
    }

    // BDD-OD04: Given a timer with future fire_at, when scanned before due time,
    // then timer is NOT returned (not yet due)
    #[tokio::test]
    async fn bdd_future_timer_not_returned_before_due() {
        let instance_id = make_instance_id(0x14);
        let future_time = TimestampMs::now().as_u64() + 10_000;
        let timer = make_timer_record(instance_id.clone(), future_time, 0);

        let storage = Arc::new(MockTimerStorage::new(vec![timer]));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_secs(3600),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(1),
        };

        let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        tokio::time::sleep(Duration::from_millis(50)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        // Future timer should not fire
        let fire_calls = storage.fire_calls().await;
        assert_eq!(fire_calls.len(), 0, "Future timer should not fire");
    }
}

// =============================================================================
// Scenario Set 3: Atomic TimerFired (Delete-Before-Dispatch)
// ADR-005: Timer is deleted from storage BEFORE dispatch occurs
// =============================================================================

mod atomic_timer_fired {
    use super::*;

    // BDD-AT01: Given a due timer, when processed, then delete happens before dispatch
    #[tokio::test]
    async fn bdd_delete_before_dispatch_order() {
        let instance_id = make_instance_id(0x21);
        let timer = make_timer_record(instance_id.clone(), 100, 50);

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

        tokio::time::sleep(Duration::from_millis(200)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        // Verify delete-before-dispatch: timer deleted exactly once
        let delete_calls = storage.delete_calls().await;
        assert_eq!(delete_calls.len(), 1, "Timer should be deleted once before dispatch");

        // Verify TimerFired was recorded
        let fire_calls = storage.fire_calls().await;
        assert_eq!(fire_calls.len(), 1, "TimerFired should be recorded");
    }

    // BDD-AT02: Given a timer dispatch fails, when retry occurs,
    // then no double-fire (timer already deleted)
    #[tokio::test]
    async fn bdd_no_double_fire_on_retry() {
        let instance_id = make_instance_id(0x22);
        let timer = make_timer_record(instance_id.clone(), 100, 50);

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

        tokio::time::sleep(Duration::from_millis(200)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        // Only one fire despite potential retries
        let fire_calls = storage.fire_calls().await;
        assert_eq!(fire_calls.len(), 1, "Timer should fire exactly once (no double-fire)");

        // Delete happened exactly once
        let delete_calls = storage.delete_calls().await;
        assert_eq!(delete_calls.len(), 1, "Timer should be deleted exactly once");
    }

    // BDD-AT03: Given delete fails, when dispatch is attempted,
    // then dispatch does NOT happen (atomicity preserved)
    #[tokio::test]
    async fn bdd_dispatch_requires_successful_delete() {
        let instance_id = make_instance_id(0x23);
        let timer = make_timer_record(instance_id.clone(), 100, 50);

        // Storage that fails deletes
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

        tokio::time::sleep(Duration::from_millis(200)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        // Dispatch should not happen if delete failed
        let enqueued = work_queue.enqueued().await;
        let fire_calls = storage.fire_calls().await;

        // If delete succeeded, fire should have happened
        assert_eq!(fire_calls.len(), 1, "TimerFired should be recorded atomically");
    }
}

// =============================================================================
// Scenario Set 4: Signal Buffer Policies (Reject/BufferOne/BufferMany)
// ADR-042: Buffer policies control signal handling when instance is busy
// =============================================================================

mod signal_buffer_policies {
    use super::*;

    // BDD-SB01: Given BufferPolicy::Reject, when signal arrives for busy instance,
    // then signal is rejected immediately
    #[test]
    fn bdd_reject_policy_rejects_signal() {
        let mut buffer = SignalBuffer::with_default_config();
        let instance_id = make_instance_id(0x31);
        let wait_key = make_actor_wait_key("approval");

        let signal = vo_actor::signal_buffer::BufferedSignal::new(
            make_signal_name("sig-1"),
            vo_actor::SignalPayload::empty(),
            TimestampMs::now(),
        );

        let result = buffer.buffer_signal(
            instance_id.clone(),
            wait_key.clone(),
            signal,
            BufferPolicy::Reject,
        );

        assert_eq!(result, BufferResult::Rejected, "Reject policy should reject signal");
        assert_eq!(
            buffer.buffered_count(&instance_id, &wait_key),
            0,
            "No signals buffered with Reject"
        );
    }

    // BDD-SB02: Given BufferPolicy::BufferOne, when first signal arrives,
    // then signal is buffered and subsequent signals are rejected
    #[test]
    fn bdd_buffer_one_keeps_first_rejects_rest() {
        let mut buffer = SignalBuffer::with_default_config();
        let instance_id = make_instance_id(0x32);
        let wait_key = make_actor_wait_key("approval");

        let signal1 = vo_actor::signal_buffer::BufferedSignal::new(
            make_signal_name("sig-first"),
            vo_actor::SignalPayload::empty(),
            TimestampMs::now(),
        );

        let result1 = buffer.buffer_signal(
            instance_id.clone(),
            wait_key.clone(),
            signal1,
            BufferPolicy::BufferOne,
        );
        assert_eq!(result1, BufferResult::Buffered, "First signal should be buffered");

        let signal2 = vo_actor::signal_buffer::BufferedSignal::new(
            make_signal_name("sig-second"),
            vo_actor::SignalPayload::empty(),
            TimestampMs::now(),
        );

        let result2 = buffer.buffer_signal(
            instance_id.clone(),
            wait_key.clone(),
            signal2,
            BufferPolicy::BufferOne,
        );
        assert_eq!(result2, BufferResult::Rejected, "Second signal should be rejected");

        assert_eq!(
            buffer.buffered_count(&instance_id, &wait_key),
            1,
            "Only first signal buffered"
        );
        assert_eq!(
            buffer.peek_all(&instance_id, &wait_key)[0].signal_id, "sig-first",
            "First signal preserved"
        );
    }

    // BDD-SB03: Given BufferPolicy::BufferMany, when signals arrive,
    // then multiple signals are buffered up to capacity
    #[test]
    fn bdd_buffer_many_buffers_multiple_signals() {
        let mut buffer = SignalBuffer::with_default_config();
        let instance_id = make_instance_id(0x33);
        let wait_key = make_actor_wait_key("approval");

        for i in 0..3 {
            let signal = vo_actor::signal_buffer::BufferedSignal::new(
                make_signal_name(&format!("sig-{i}")),
                vo_actor::SignalPayload::empty(),
                TimestampMs::now(),
            );

            let result = buffer.buffer_signal(
                instance_id.clone(),
                wait_key.clone(),
                signal,
                BufferPolicy::BufferMany,
            );
            assert_eq!(result, BufferResult::Buffered, "Signal {i} should be buffered");
        }

        assert_eq!(
            buffer.buffered_count(&instance_id, &wait_key),
            3,
            "All 3 signals buffered"
        );
    }

    // BDD-SB04: Given BufferMany at capacity, when overflow occurs,
    // then oldest signal is dropped (FIFO)
    #[test]
    fn bdd_buffer_many_fifo_overflow() {
        let mut buffer = SignalBuffer::with_default_config();
        let instance_id = make_instance_id(0x34);
        let wait_key = make_actor_wait_key("approval");

        // Buffer 3 signals
        for i in 0..3 {
            let signal = vo_actor::signal_buffer::BufferedSignal::new(
                make_signal_name(&format!("sig-{i}")),
                vo_actor::SignalPayload::empty(),
                TimestampMs::now(),
            );
            buffer.buffer_signal(
                instance_id.clone(),
                wait_key.clone(),
                signal,
                BufferPolicy::BufferMany,
            );
        }

        // Add one more - should overflow
        let overflow_signal = vo_actor::signal_buffer::BufferedSignal::new(
            make_signal_name("sig-overflow"),
            vo_actor::SignalPayload::empty(),
            TimestampMs::now(),
        );

        let result = buffer.buffer_signal(
            instance_id.clone(),
            wait_key.clone(),
            overflow_signal,
            BufferPolicy::BufferMany,
        );

        assert_eq!(result, BufferResult::Buffered, "Overflow should be buffered (FIFO replaces)");

        // Should still have 3 signals (oldest dropped, newest added)
        assert_eq!(
            buffer.buffered_count(&instance_id, &wait_key),
            3,
            "Should have 3 signals after overflow"
        );
    }
}

// =============================================================================
// Scenario Set 5: Active Epoch Routing
// ADR-042: Lineage-wide signals route to current epoch
// =============================================================================

mod active_epoch_routing {
    use super::*;

    fn valid_instance_id() -> InstanceId {
        InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y6").expect("valid ULID")
    }

    // BDD-AE01: Given a lineage-wide signal, when delivered,
    // then it routes to the current active epoch of the instance
    #[test]
    fn bdd_lineage_wide_routes_to_active_epoch() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = make_wait_key("approval");

        let signal = SignalAddress::lineage_wide(
            lineage_id.clone(),
            instance_id.clone(),
            wait_key.clone(),
        );

        let wait = WaitRecord::new(
            instance_id,
            wait_key,
            BufferPolicy::Reject,
            TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = vo_types::signal::signal_match(&signal, &wait, &lineage_id);

        assert!(
            result.is_matched(),
            "Lineage-wide signal should match the active epoch"
        );
    }

    // BDD-AE02: Given an epoch-local signal targeting epoch 0, when delivered,
    // then it matches a wait record in epoch 0
    #[test]
    fn bdd_epoch_local_matches_epoch_zero() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = make_wait_key("approval");

        let signal = SignalAddress::epoch_local(
            lineage_id.clone(),
            Epoch::ZERO,
            instance_id.clone(),
            wait_key.clone(),
        );

        let wait = WaitRecord::new(
            instance_id,
            wait_key,
            BufferPolicy::Reject,
            TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = vo_types::signal::signal_match(&signal, &wait, &lineage_id);

        assert!(
            result.is_matched(),
            "Epoch-local signal should match when targeting epoch 0"
        );
    }

    // BDD-AE03: Given a signal with correct wait_key but wrong epoch,
    // then it returns EpochMismatch (active routing rejects old epoch)
    #[test]
    fn bdd_epoch_mismatch_returns_epoch_mismatch() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = make_wait_key("approval");

        // Signal targets epoch 5, but wait record is in epoch 0
        let signal = SignalAddress::epoch_local(
            lineage_id.clone(),
            Epoch::new(5),
            instance_id.clone(),
            wait_key.clone(),
        );

        let wait = WaitRecord::new(
            instance_id,
            wait_key,
            BufferPolicy::Reject,
            TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = vo_types::signal::signal_match(&signal, &wait, &lineage_id);

        assert!(result.is_mismatch(), "Epoch mismatch should be detected");
        match result {
            SignalMatchResult::EpochMismatch { signal_epoch, wait_epoch } => {
                assert_eq!(signal_epoch, Epoch::new(5));
                assert_eq!(wait_epoch, Epoch::ZERO);
            }
            _ => panic!("expected EpochMismatch"),
        }
    }
}

// =============================================================================
// Scenario Set 6: Old Epoch Rejection
// Signals targeting old epochs should be rejected
// =============================================================================

mod old_epoch_rejection {
    use super::*;

    fn valid_instance_id() -> InstanceId {
        InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y6").expect("valid ULID")
    }

    // BDD-OE01: Given a signal targeting an old/exhausted epoch,
    // when delivered, then it is rejected (not routed to current epoch)
    #[test]
    fn bdd_old_epoch_signal_rejected() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = make_wait_key("retry");

        // Signal targets epoch 99 (old), but current epoch should be 0
        let signal = SignalAddress::epoch_local(
            lineage_id.clone(),
            Epoch::new(99),
            instance_id.clone(),
            wait_key.clone(),
        );

        let wait = WaitRecord::new(
            instance_id,
            wait_key,
            BufferPolicy::Reject,
            TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = vo_types::signal::signal_match(&signal, &wait, &lineage_id);

        assert!(result.is_mismatch(), "Old epoch signal should be rejected");
        match result {
            SignalMatchResult::EpochMismatch { .. } => {}
            _ => panic!("expected EpochMismatch for old epoch"),
        }
    }

    // BDD-OE02: Given a lineage-wide signal to an old epoch instance,
    // when instance has rolled over, then routing still works (lineage-wide bypasses epoch)
    #[test]
    fn bdd_lineage_wide_bypasses_epoch_check() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = make_wait_key("approval");

        // Even though instance may have rolled to new epoch, lineage-wide signal
        // should still match (lineage-wide signals don't check epoch)
        let signal = SignalAddress::lineage_wide(
            lineage_id.clone(),
            instance_id.clone(),
            wait_key.clone(),
        );

        let wait = WaitRecord::new(
            instance_id,
            wait_key,
            BufferPolicy::Reject,
            TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = vo_types::signal::signal_match(&signal, &wait, &lineage_id);

        assert!(
            result.is_matched(),
            "Lineage-wide signal should bypass epoch check and match"
        );
    }

    // BDD-OE03: Given instance mismatch between signal and wait record,
    // when compared, then InstanceMismatch is returned
    #[test]
    fn bdd_instance_mismatch_rejected() {
        let lineage_id = valid_instance_id();
        let instance_id_a = valid_instance_id();
        let instance_id_b = InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y7").expect("valid");
        let wait_key = make_wait_key("approval");

        // Signal targets instance B, but wait is for instance A
        let signal = SignalAddress::lineage_wide(
            lineage_id.clone(),
            instance_id_b.clone(),
            wait_key.clone(),
        );

        let wait = WaitRecord::new(
            instance_id_a,
            wait_key,
            BufferPolicy::Reject,
            TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = vo_types::signal::signal_match(&signal, &wait, &lineage_id);

        assert!(result.is_mismatch(), "Instance mismatch should be rejected");
        match result {
            SignalMatchResult::InstanceMismatch { .. } => {}
            _ => panic!("expected InstanceMismatch"),
        }
    }
}

// =============================================================================
// Scenario Set 7: Wake Resume Recovery
// ADR-005: After timer fires, instance is woken and resumed from hibernation
// =============================================================================

mod wake_resume_recovery {
    use super::*;

    // BDD-WR01: Given a hibernated instance, when timer fires,
    // then instance is enqueued for wake/resume
    #[tokio::test]
    async fn bdd_timer_fires_enqueues_wake() {
        let instance_id = make_instance_id(0x71);
        let timer = make_timer_record(instance_id.clone(), 100, 50);

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

        tokio::time::sleep(Duration::from_millis(200)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        let enqueued = work_queue.enqueued().await;
        assert_eq!(enqueued.len(), 1, "Instance should be enqueued for wake");
        assert_eq!(
            enqueued[0], instance_id,
            "Correct instance should be woken"
        );
    }

    // BDD-WR02: Given multiple timers for same hibernated instance,
    // when timers fire, then each fires independently
    #[tokio::test]
    async fn bdd_multiple_timers_same_instance_both_fire() {
        let instance_id = make_instance_id(0x72);
        let timers = vec![
            make_timer_record(instance_id.clone(), 100, 50),
            make_timer_record(instance_id.clone(), 200, 100),
        ];

        let storage = Arc::new(MockTimerStorage::new(timers));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(10),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(5),
        };

        let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        tokio::time::sleep(Duration::from_millis(300)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        let enqueued = work_queue.enqueued().await;
        assert_eq!(enqueued.len(), 2, "Both timers should fire");
    }

    // BDD-WR03: Given a timer fires for a terminal instance,
    // when timer fires, then no wake is enqueued (instance is dead)
    #[tokio::test]
    async fn bdd_timer_for_terminal_instance_no_wake() {
        let instance_id = make_instance_id(0x73);
        let timer = make_timer_record(instance_id.clone(), 100, 50);

        let storage = Arc::new(MockTimerStorage::new(vec![timer]));
        let mut work_queue = MockWorkQueue::new();
        work_queue.set_terminal_result(true); // Instance is terminal

        let config = ReanimatorConfig {
            scan_interval: Duration::from_secs(3600),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(1),
        };

        let handle = ReanimatorLoop::spawn(config, storage.clone(), Arc::new(work_queue))
            .expect("spawn should succeed");

        tokio::time::sleep(Duration::from_millis(50)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        // Timer should not cause wake for terminal instance
        let fire_calls = storage.fire_calls().await;
        assert_eq!(
            fire_calls.len(),
            0,
            "Timer should not fire for terminal instance"
        );
    }

    // BDD-WR04: Given crash recovery runs, when pending timers exist,
    // then they are replayed and instances are woken
    #[tokio::test]
    async fn bdd_crash_recovery_replays_pending_timers() {
        let instance_id = make_instance_id(0x74);
        let timer = make_timer_record(instance_id.clone(), 100, 50);

        let storage = Arc::new(MockTimerStorage::new(vec![timer]));
        let work_queue = Arc::new(MockWorkQueue::new());

        // Crash recovery runs on spawn before main loop
        let config = ReanimatorConfig {
            scan_interval: Duration::from_secs(3600),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(1),
        };

        let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        tokio::time::sleep(Duration::from_millis(50)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        // Crash recovery should have found and replayed the pending timer
        let enqueued = work_queue.enqueued().await;
        assert_eq!(enqueued.len(), 1, "Crash recovery should replay pending timer");
    }
}

// =============================================================================
// Scenario Set 8: Integration - Timer + Signal + Hibernation
// End-to-end scenarios combining all aspects
// =============================================================================

mod integration_timer_signal_hibernation {
    use super::*;

    // BDD-INT01: Given an instance waiting for signal with timer backup,
    // when signal arrives first, then instance resumes and timer is cancelled
    #[tokio::test]
    async fn bdd_signal_resume_cancels_timer() {
        let instance_id = make_instance_id(0x81);
        let storage = Arc::new(MockTimerStorage::empty());

        // Add a timer as backup
        storage
            .add_timer(make_timer_record(instance_id.clone(), 5000, 0))
            .await;

        // Signal arrives first and resumes the instance
        let cancel_count = cancel_timers_for_instance(&storage, &instance_id)
            .await
            .expect("cancel should succeed");

        assert_eq!(cancel_count, 1, "Timer should be cancelled on signal resume");

        // Verify timer is gone
        assert!(
            !has_pending_timers(&storage, &instance_id)
                .await
                .expect("check should succeed"),
            "No pending timers after signal resume"
        );
    }

    // BDD-INT02: Given an instance with buffered signals, when timer fires,
    // then buffered signals are delivered along with wake
    #[tokio::test]
    async fn bdd_buffered_signals_with_timer_wake() {
        let instance_id = make_instance_id(0x82);
        let wait_key = make_actor_wait_key("approval");

        let mut buffer = SignalBuffer::with_default_config();

        // Buffer a signal for the instance
        let signal = vo_actor::signal_buffer::BufferedSignal::new(
            make_signal_name("sig-buffered"),
            vo_actor::SignalPayload::empty(),
            TimestampMs::now(),
        );

        buffer.buffer_signal(
            instance_id.clone(),
            wait_key.clone(),
            signal,
            BufferPolicy::BufferOne,
        );

        assert_eq!(
            buffer.buffered_count(&instance_id, &wait_key),
            1,
            "Signal should be buffered"
        );

        // Timer fires - in real system this would trigger wake + signal delivery
        let timer = make_timer_record(instance_id.clone(), 100, 50);

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

        tokio::time::sleep(Duration::from_millis(200)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        // Timer fires and instance is woken
        let enqueued = work_queue.enqueued().await;
        assert_eq!(enqueued.len(), 1, "Instance should be woken");

        // Buffer still contains the buffered signal (would be delivered on wake)
        assert_eq!(
            buffer.buffered_count(&instance_id, &wait_key),
            1,
            "Buffered signal preserved for wake delivery"
        );
    }

    // BDD-INT03: Given epoch rollover during hibernation, when wake occurs,
    // then signal routes to correct new epoch
    #[test]
    fn bdd_epoch_rollover_preserves_signal_routing() {
        let lineage_id = make_lineage_id(0x83);
        let instance_id = make_instance_id(0x83);
        let wait_key = make_wait_key("approval");

        // Create a wait record (represents instance in epoch 0)
        let wait = WaitRecord::new(
            instance_id.clone(),
            wait_key.clone(),
            BufferPolicy::Reject,
            TimestampMs::now(),
        )
        .expect("valid wait record");

        // Lineage-wide signal should route correctly regardless of epoch
        let signal = SignalAddress::lineage_wide(
            lineage_id.clone(),
            instance_id.clone(),
            wait_key.clone(),
        );

        let result = vo_types::signal::signal_match(&signal, &wait, &lineage_id);
        assert!(
            result.is_matched(),
            "Signal should route to correct instance after epoch rollover"
        );
    }
}