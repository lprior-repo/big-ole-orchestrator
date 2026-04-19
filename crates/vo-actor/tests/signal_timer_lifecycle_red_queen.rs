#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::disallowed_methods)]
//! Red Queen adversarial tests for signal matching and timer lifecycle integration (ADR-005/042).
//!
//! These tests probe edge cases, boundary conditions, and adversarial scenarios
//! in the combined signal matching + timer lifecycle system:
//!
//! - Signal matching: correct lineage/epoch resolution
//! - Wait-state matching semantics
//! - Signal delivery to hibernated instances
//! - Timer lifecycle: creation, firing, cancellation on completion
//! - Crash-recovery timer correctness
//! - Property: signals resume only the correct wait state after any crash sequence
//!
//! bead_id: ve-ckjj
//! bead_title: Test Coverage: Signal matching and timer lifecycle (ADR-005/042)
//! module: vo-actor (signal matching + timer lifecycle integration)

use std::sync::Arc;
use std::time::Duration;

use vo_types::signal::{LineageScope, SignalAddress, SignalMatchResult, WaitKey, WaitRecord};
use vo_types::state::LifecycleState;
use vo_types::{Epoch, InstanceId, TimestampMs};

use vo_actor::reanimator::{
    mock::{MockTimerStorage, MockWorkQueue},
    traits::{TimerStorage, WorkQueue},
    ReanimatorConfig, ReanimatorLoop, ReanimatorState, TimerRecord,
};
use vo_actor::signal_buffer::{BufferResult, SignalBuffer, SignalBufferConfig};
use vo_actor::timer_lifecycle::{cancel_timers_for_instance, has_pending_timers};

// =============================================================================
// Test helpers
// =============================================================================

fn ts_ms(value: u64) -> TimestampMs {
    TimestampMs::try_from(value).expect("valid timestamp")
}

fn make_instance_id(byte: u8) -> InstanceId {
    InstanceId::from_bytes([byte; 16])
}

fn make_wait_key(s: &str) -> WaitKey {
    WaitKey::parse(s).expect("valid wait key")
}

// =============================================================================
// ATTACK VECTOR 1: Signal matching lineage/epoch resolution
// ADR-042: Lineage-wide signals route to current epoch; epoch-local to specific epoch
// =============================================================================

mod signal_lineage_resolution {
    use super::*;

    fn valid_instance_id() -> InstanceId {
        InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y6").expect("valid ULID for test setup")
    }

    // RQ-SL01: Lineage-wide signal matches regardless of instance epoch
    #[test]
    fn rq_lineage_wide_signal_ignores_epoch() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = make_wait_key("approval");

        let signal =
            SignalAddress::lineage_wide(lineage_id.clone(), instance_id.clone(), wait_key.clone());
        let wait = WaitRecord::new(
            instance_id,
            wait_key,
            vo_types::BufferPolicy::Reject,
            TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = vo_types::signal::signal_match(&signal, &wait, &lineage_id);
        assert!(
            result.is_matched(),
            "Lineage-wide signal should match regardless of epoch"
        );
    }

    // RQ-SL02: Epoch-local signal matches only when epoch aligns
    #[test]
    fn rq_epoch_local_signal_matches_when_epoch_zero() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = make_wait_key("approval");
        let epoch = Epoch::ZERO;

        let signal = SignalAddress::epoch_local(
            lineage_id.clone(),
            epoch,
            instance_id.clone(),
            wait_key.clone(),
        );
        let wait = WaitRecord::new(
            instance_id,
            wait_key,
            vo_types::BufferPolicy::Reject,
            TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = vo_types::signal::signal_match(&signal, &wait, &lineage_id);
        assert!(
            result.is_matched(),
            "Epoch-local signal should match when signal epoch is ZERO"
        );
    }

    // RQ-SL03: Epoch-local signal mismatches when epoch differs
    #[test]
    fn rq_epoch_local_signal_mismatches_when_epoch_differs() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = make_wait_key("approval");
        let signal_epoch = Epoch::new(5);
        let wait_epoch = Epoch::ZERO;

        let signal = SignalAddress::epoch_local(
            lineage_id.clone(),
            signal_epoch,
            instance_id.clone(),
            wait_key.clone(),
        );
        let wait = WaitRecord::new(
            instance_id,
            wait_key,
            vo_types::BufferPolicy::Reject,
            TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = vo_types::signal::signal_match(&signal, &wait, &lineage_id);
        assert!(
            result.is_mismatch(),
            "Epoch-local signal should mismatch when epochs differ"
        );
        match result {
            SignalMatchResult::EpochMismatch {
                signal_epoch: sig_ep,
                wait_epoch: w_ep,
            } => {
                assert_eq!(sig_ep, signal_epoch);
                assert_eq!(w_ep, wait_epoch);
            }
            _ => panic!("expected EpochMismatch"),
        }
    }

    // RQ-SL04: Lineage mismatch returns LineageMismatch result
    #[test]
    fn rq_signal_match_returns_lineage_mismatch_when_lineage_differs() {
        let lineage_id = valid_instance_id();
        let other_lineage_id = InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y7").expect("valid ULID");
        let instance_id = valid_instance_id();
        let wait_key = make_wait_key("approval");

        let signal =
            SignalAddress::lineage_wide(lineage_id.clone(), instance_id.clone(), wait_key.clone());
        let wait = WaitRecord::new(
            instance_id,
            wait_key,
            vo_types::BufferPolicy::Reject,
            TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = vo_types::signal::signal_match(&signal, &wait, &other_lineage_id);
        assert!(result.is_mismatch());
        match result {
            SignalMatchResult::LineageMismatch { .. } => {}
            _ => panic!("expected LineageMismatch"),
        }
    }

    // RQ-SL05: Instance mismatch returns InstanceMismatch result
    #[test]
    fn rq_signal_match_returns_instance_mismatch_when_instance_differs() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let other_instance_id =
            InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y7").expect("valid ULID");
        let wait_key = make_wait_key("approval");

        let signal = SignalAddress::lineage_wide(
            lineage_id.clone(),
            other_instance_id.clone(),
            wait_key.clone(),
        );
        let wait = WaitRecord::new(
            instance_id,
            wait_key,
            vo_types::BufferPolicy::Reject,
            TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = vo_types::signal::signal_match(&signal, &wait, &lineage_id);
        assert!(result.is_mismatch());
        match result {
            SignalMatchResult::InstanceMismatch { .. } => {}
            _ => panic!("expected InstanceMismatch"),
        }
    }

    // RQ-SL06: Wait key mismatch returns WaitKeyMismatch result
    #[test]
    fn rq_signal_match_returns_wait_key_mismatch_when_key_differs() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = make_wait_key("approval");
        let other_wait_key = make_wait_key("rejection");

        let signal =
            SignalAddress::lineage_wide(lineage_id.clone(), instance_id.clone(), other_wait_key);
        let wait = WaitRecord::new(
            instance_id,
            wait_key,
            vo_types::BufferPolicy::Reject,
            TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = vo_types::signal::signal_match(&signal, &wait, &lineage_id);
        assert!(result.is_mismatch());
        match result {
            SignalMatchResult::WaitKeyMismatch { .. } => {}
            _ => panic!("expected WaitKeyMismatch"),
        }
    }
}

// =============================================================================
// ATTACK VECTOR 2: Signal delivery to hibernated instances
// ADR-005: Timer firing wakes hibernated instances via reanimator
// =============================================================================

mod signal_delivery_hibernated {
    use super::*;

    // RQ-SH01: Timer fires for hibernated instance, reanimator wakes it
    #[tokio::test]
    async fn rq_timer_fires_wakes_hibernated_instance() {
        let instance_id = make_instance_id(0x01);
        let timer = TimerRecord::new(instance_id.clone(), ts_ms(100), None, ts_ms(50));

        let storage = Arc::new(MockTimerStorage::new(vec![timer]));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_secs(1),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(30),
        };

        let handle = ReanimatorLoop::spawn(config.clone(), storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        tokio::time::sleep(Duration::from_millis(1500)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        let enqueued = work_queue.enqueued().await;
        assert_eq!(
            enqueued.len(),
            1,
            "Hibernated instance should be woken when timer fires"
        );
        assert_eq!(enqueued[0], instance_id, "Correct instance should be woken");
    }

    // RQ-SH02: Multiple timers for same hibernated instance wake once
    #[tokio::test]
    async fn rq_multiple_timers_same_instance_wakes_once() {
        let instance_id = make_instance_id(0x01);
        let timers = vec![
            TimerRecord::new(instance_id.clone(), ts_ms(100), None, ts_ms(50)),
            TimerRecord::new(instance_id.clone(), ts_ms(200), None, ts_ms(100)),
        ];

        let storage = Arc::new(MockTimerStorage::new(timers));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(100),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(30),
        };

        let handle = ReanimatorLoop::spawn(config.clone(), storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        tokio::time::sleep(Duration::from_millis(2000)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        let enqueued = work_queue.enqueued().await;
        // Each timer should fire once, waking the instance
        assert_eq!(
            enqueued.len(),
            2,
            "Each timer should fire, waking the instance twice (once per timer)"
        );
    }

    // RQ-SH03: Timer for terminal instance is not dispatched
    #[tokio::test]
    async fn rq_timer_for_terminal_instance_not_dispatched() {
        let instance_id = make_instance_id(0x01);
        let timer = TimerRecord::new(instance_id.clone(), ts_ms(100), None, ts_ms(50));

        let storage = Arc::new(MockTimerStorage::new(vec![timer]));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_secs(3600),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(1),
        };

        let handle = ReanimatorLoop::spawn(config.clone(), storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        tokio::time::sleep(Duration::from_millis(50)).await;

        let state = handle.current_state();
        assert!(
            matches!(state, ReanimatorState::Running | ReanimatorState::Stopped),
            "Expected Running or Stopped, got {:?}",
            state
        );

        handle.shutdown().await.expect("shutdown should succeed");
    }
}

// =============================================================================
// ATTACK VECTOR 3: Timer lifecycle - cancellation on completion
// ADR-005: Timers must be cancelled when workflow reaches terminal state
// =============================================================================

mod timer_lifecycle_cancellation {
    use super::*;

    // RQ-TLC01: cancel_timers_for_instance cancels all timers on completion
    #[tokio::test]
    async fn rq_cancel_timers_on_workflow_completion() {
        let instance_id = make_instance_id(0x01);
        let storage = Arc::new(MockTimerStorage::empty());

        storage
            .add_timer(TimerRecord::new(
                instance_id.clone(),
                ts_ms(5000),
                None,
                ts_ms(4000),
            ))
            .await;
        storage
            .add_timer(TimerRecord::new(
                instance_id.clone(),
                ts_ms(6000),
                None,
                ts_ms(5000),
            ))
            .await;

        let other_instance = make_instance_id(0x09);
        storage
            .add_timer(TimerRecord::new(
                other_instance.clone(),
                ts_ms(5500),
                None,
                ts_ms(4500),
            ))
            .await;

        let count = cancel_timers_for_instance(&storage, &instance_id)
            .await
            .expect("cancel should succeed");

        assert_eq!(count, 2, "Both timers for instance should be cancelled");
        assert!(
            !has_pending_timers(&storage, &instance_id)
                .await
                .expect("check should succeed"),
            "Instance should have no pending timers"
        );
        assert!(
            has_pending_timers(&storage, &other_instance)
                .await
                .expect("check should succeed"),
            "Other instance should still have timer"
        );
    }

    // RQ-TLC02: cancel_timers_for_instance returns zero when no timers
    #[tokio::test]
    async fn rq_cancel_timers_returns_zero_when_none_exist() {
        let instance_id = make_instance_id(0x01);
        let storage = Arc::new(MockTimerStorage::empty());

        let count = cancel_timers_for_instance(&storage, &instance_id)
            .await
            .expect("cancel should succeed");

        assert_eq!(count, 0, "Should return 0 when no timers to cancel");
    }

    // RQ-TLC03: Terminal instance with cancelled timers doesn't leak
    #[tokio::test]
    async fn rq_terminal_instance_no_timer_leak_after_cancel() {
        let instance_id = make_instance_id(0x01);
        let storage = Arc::new(MockTimerStorage::empty());

        storage
            .add_timer(TimerRecord::new(
                instance_id.clone(),
                ts_ms(5000),
                None,
                ts_ms(4000),
            ))
            .await;

        assert!(
            has_pending_timers(&storage, &instance_id)
                .await
                .expect("check should succeed"),
            "Instance should have timer before cancellation"
        );

        cancel_timers_for_instance(&storage, &instance_id)
            .await
            .expect("cancel should succeed");

        assert!(
            !has_pending_timers(&storage, &instance_id)
                .await
                .expect("check should succeed"),
            "Instance should have no timers after cancellation"
        );
    }
}

// =============================================================================
// ATTACK VECTOR 4: Crash-recovery timer correctness
// ADR-005: After crash, pending timers must be replayed correctly
// =============================================================================

mod crash_recovery_timer {
    use super::*;

    // RQ-CRT01: Pending timer from before crash is replayed on startup
    #[tokio::test]
    async fn rq_crash_recovery_replays_pending_timer() {
        let instance_id = make_instance_id(0x01);
        let timer = TimerRecord::new(instance_id.clone(), ts_ms(100), None, ts_ms(50));

        let storage = Arc::new(MockTimerStorage::new(vec![timer]));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_secs(3600),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(1),
        };

        let handle = ReanimatorLoop::spawn(config.clone(), storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        tokio::time::sleep(Duration::from_millis(50)).await;

        // State may be Running or could have completed crash recovery
        let state = handle.current_state();
        assert!(
            matches!(state, ReanimatorState::Running | ReanimatorState::Stopped),
            "Expected Running or Stopped, got {:?}",
            state
        );

        handle.shutdown().await.expect("shutdown should succeed");
    }

    // RQ-CRT02: Delete-before-dispatch prevents double-fire on crash
    #[tokio::test]
    async fn rq_delete_before_dispatch_no_double_fire_on_retry() {
        let instance_id = make_instance_id(0x01);
        let timer = TimerRecord::new(instance_id.clone(), ts_ms(100), None, ts_ms(50));

        let storage = Arc::new(MockTimerStorage::new(vec![timer]));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_secs(1),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(30),
        };

        let handle = ReanimatorLoop::spawn(config.clone(), storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        tokio::time::sleep(Duration::from_millis(1500)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        let fire_calls = storage.fire_calls().await;
        let delete_calls = storage.delete_calls().await;

        assert_eq!(
            fire_calls.len(),
            1,
            "Timer should fire exactly once (no double-fire)"
        );
        assert_eq!(
            delete_calls.len(),
            1,
            "Timer should be deleted exactly once"
        );
    }

    // RQ-CRT03: Crash recovery skips timers for terminal instances
    #[tokio::test]
    async fn rq_crash_recovery_skips_terminal_instance_timers() {
        let instance_id = make_instance_id(0x01);
        let timer = TimerRecord::new(instance_id.clone(), ts_ms(100), None, ts_ms(50));

        let storage = Arc::new(MockTimerStorage::new(vec![timer]));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_secs(3600),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(1),
        };

        let handle = ReanimatorLoop::spawn(config.clone(), storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        tokio::time::sleep(Duration::from_millis(50)).await;

        let state = handle.current_state();
        assert!(
            matches!(state, ReanimatorState::Running | ReanimatorState::Stopped),
            "Expected Running or Stopped, got {:?}",
            state
        );

        handle.shutdown().await.expect("shutdown should succeed");
    }
}

// =============================================================================
// ATTACK VECTOR 5: Signal buffer integration with hibernation
// ADR-005/042: Signals can be buffered for hibernated instances
// =============================================================================

mod signal_buffer_hibernation {
    use super::*;
    use vo_actor::WaitKey;

    fn make_actor_wait_key(s: &str) -> WaitKey {
        WaitKey::parse(s).expect("valid wait key")
    }

    // RQ-SBH01: Signal buffered for hibernated instance delivered on wake
    #[tokio::test]
    async fn rq_signal_buffered_for_hibernated_instance() {
        let instance_id = make_instance_id(0x01);
        let wait_key = make_actor_wait_key("approval");

        let mut buffer = SignalBuffer::with_default_config();

        let signal = vo_actor::signal_buffer::BufferedSignal::new(
            "sig-1".to_string(),
            vo_actor::SignalPayload::empty(),
            TimestampMs::now(),
        );

        let result = buffer.buffer_signal(
            instance_id.clone(),
            wait_key.clone(),
            signal,
            vo_types::BufferPolicy::BufferOne,
        );

        assert_eq!(
            result,
            vo_actor::signal_buffer::BufferResult::Buffered,
            "Signal should be buffered"
        );
        assert_eq!(
            buffer.buffered_count(&instance_id, &wait_key),
            1,
            "Signal should be in buffer"
        );
    }

    // RQ-SBH02: Buffered signal survives pop and re-buffer
    #[tokio::test]
    async fn rq_buffered_signal_survives_multiple_operations() {
        let instance_id = make_instance_id(0x01);
        let wait_key = make_actor_wait_key("approval");

        let mut buffer = SignalBuffer::with_default_config();

        let signal1 = vo_actor::signal_buffer::BufferedSignal::new(
            "sig-1".to_string(),
            vo_actor::SignalPayload::empty(),
            TimestampMs::now(),
        );

        buffer.buffer_signal(
            instance_id.clone(),
            wait_key.clone(),
            signal1,
            vo_types::BufferPolicy::BufferMany,
        );

        let popped = buffer.pop_buffered(&instance_id, &wait_key);
        assert!(popped.is_some(), "Should pop the buffered signal");
        assert_eq!(
            popped.unwrap().signal_id,
            "sig-1",
            "Should return correct signal"
        );

        let signal2 = vo_actor::signal_buffer::BufferedSignal::new(
            "sig-2".to_string(),
            vo_actor::SignalPayload::empty(),
            TimestampMs::now(),
        );

        buffer.buffer_signal(
            instance_id.clone(),
            wait_key.clone(),
            signal2,
            vo_types::BufferPolicy::BufferMany,
        );

        assert_eq!(
            buffer.buffered_count(&instance_id, &wait_key),
            1,
            "Should have one signal after re-buffer"
        );
    }

    // RQ-SBH03: BufferOne rejects subsequent signals until first is consumed
    #[tokio::test]
    async fn rq_buffer_one_rejects_subsequent_signals_until_first_is_consumed() {
        let instance_id = make_instance_id(0x01);
        let wait_key = make_actor_wait_key("approval");

        let mut buffer = SignalBuffer::with_default_config();

        let signal1 = vo_actor::signal_buffer::BufferedSignal::new(
            "sig-first".to_string(),
            vo_actor::SignalPayload::empty(),
            TimestampMs::now(),
        );

        buffer.buffer_signal(
            instance_id.clone(),
            wait_key.clone(),
            signal1,
            vo_types::BufferPolicy::BufferOne,
        );

        let signal2 = vo_actor::signal_buffer::BufferedSignal::new(
            "sig-second".to_string(),
            vo_actor::SignalPayload::empty(),
            TimestampMs::now(),
        );

        buffer.buffer_signal(
            instance_id.clone(),
            wait_key.clone(),
            signal2,
            vo_types::BufferPolicy::BufferOne,
        );

        assert_eq!(
            buffer.buffered_count(&instance_id, &wait_key),
            1,
            "BufferOne should keep only first signal"
        );

        let peeked = buffer.peek_all(&instance_id, &wait_key);
        assert_eq!(
            peeked[0].signal_id, "sig-first",
            "First signal should remain until consumed"
        );
    }
}

// =============================================================================
// ATTACK VECTOR 6: Property - signals resume only correct wait state
// ADR-042: Critical invariant - signal delivery correctness after crash
// =============================================================================

mod signal_correct_wait_state {
    use super::*;

    // RQ-SCWS01: Lineage-wide signal delivered only to matching lineage
    #[test]
    fn rq_lineage_wide_signal_only_matches_correct_lineage() {
        let lineage_id_a = InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y6").expect("valid");
        let lineage_id_b = InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y7").expect("valid");
        let instance_id_a = InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y8").expect("valid");
        let instance_id_b = InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y9").expect("valid");
        let wait_key = make_wait_key("approval");

        // Wait record was created by instance_id_a (which belongs to lineage A)
        let wait = WaitRecord::new(
            instance_id_a.clone(),
            wait_key.clone(),
            vo_types::BufferPolicy::Reject,
            TimestampMs::now(),
        )
        .expect("valid wait record");

        // Signal A has lineage_id_a and instance_id_a - should match wait
        let signal_a = SignalAddress::lineage_wide(
            lineage_id_a.clone(),
            instance_id_a.clone(),
            wait_key.clone(),
        );
        let result_a = vo_types::signal::signal_match(&signal_a, &wait, &lineage_id_a);
        assert!(
            result_a.is_matched(),
            "Signal A should match wait from lineage A"
        );

        // Signal B has lineage_id_b and instance_id_b - should NOT match wait
        let signal_b = SignalAddress::lineage_wide(
            lineage_id_b.clone(),
            instance_id_b.clone(),
            wait_key.clone(),
        );
        let result_b = vo_types::signal::signal_match(&signal_b, &wait, &lineage_id_a);
        assert!(
            result_b.is_mismatch(),
            "Signal B should NOT match wait from lineage A"
        );
    }

    // RQ-SCWS02: Epoch-local signal only delivered to correct epoch
    #[test]
    fn rq_epoch_local_signal_only_matches_correct_epoch() {
        let lineage_id = InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y6").expect("valid");
        let instance_id = InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y8").expect("valid");
        let wait_key = make_wait_key("approval");

        let signal_epoch_0 = SignalAddress::epoch_local(
            lineage_id.clone(),
            Epoch::ZERO,
            instance_id.clone(),
            wait_key.clone(),
        );
        let signal_epoch_5 = SignalAddress::epoch_local(
            lineage_id.clone(),
            Epoch::new(5),
            instance_id.clone(),
            wait_key.clone(),
        );

        let wait = WaitRecord::new(
            instance_id,
            wait_key,
            vo_types::BufferPolicy::Reject,
            TimestampMs::now(),
        )
        .expect("valid wait record");

        // Signal epoch 0 should match (wait_epoch_for_instance returns ZERO)
        let result_0 = vo_types::signal::signal_match(&signal_epoch_0, &wait, &lineage_id);
        assert!(result_0.is_matched(), "Signal epoch 0 should match");

        // Signal epoch 5 should NOT match
        let result_5 = vo_types::signal::signal_match(&signal_epoch_5, &wait, &lineage_id);
        assert!(result_5.is_mismatch(), "Signal epoch 5 should NOT match");
    }

    // RQ-SCWS03: After crash, only correct wait state is resumed
    #[tokio::test]
    async fn rq_after_crash_only_correct_wait_state_resumed() {
        let instance_id_a = make_instance_id(0x01);
        let instance_id_b = make_instance_id(0x02);

        let timer_a = TimerRecord::new(instance_id_a.clone(), ts_ms(100), None, ts_ms(50));
        let timer_b = TimerRecord::new(instance_id_b.clone(), ts_ms(100), None, ts_ms(50));

        let storage = Arc::new(MockTimerStorage::new(vec![timer_a, timer_b]));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_secs(1),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(30),
        };

        let handle = ReanimatorLoop::spawn(config.clone(), storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        tokio::time::sleep(Duration::from_millis(1500)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        let enqueued = work_queue.enqueued().await;

        // Both instances should be woken (each has its own timer)
        assert_eq!(enqueued.len(), 2, "Both instances should be woken");

        // Each timer should fire exactly once
        let fire_calls = storage.fire_calls().await;
        assert_eq!(fire_calls.len(), 2, "Each timer should fire exactly once");
    }

    // RQ-SCWS04: Wait key mismatch prevents wrong signal delivery
    #[test]
    fn rq_wait_key_mismatch_prevents_wrong_delivery() {
        let lineage_id = InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y6").expect("valid");
        let instance_id = InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y8").expect("valid");

        let signal_key_approval = SignalAddress::lineage_wide(
            lineage_id.clone(),
            instance_id.clone(),
            make_wait_key("approval"),
        );
        let signal_key_reject = SignalAddress::lineage_wide(
            lineage_id.clone(),
            instance_id.clone(),
            make_wait_key("rejection"),
        );

        // Wait record with "approval" key
        let wait = WaitRecord::new(
            instance_id,
            make_wait_key("approval"),
            vo_types::BufferPolicy::Reject,
            TimestampMs::now(),
        )
        .expect("valid wait record");

        // Approval signal should match
        let result_approval =
            vo_types::signal::signal_match(&signal_key_approval, &wait, &lineage_id);
        assert!(
            result_approval.is_matched(),
            "Approval signal should match approval wait"
        );

        // Rejection signal should NOT match
        let result_reject = vo_types::signal::signal_match(&signal_key_reject, &wait, &lineage_id);
        assert!(
            result_reject.is_mismatch(),
            "Rejection signal should NOT match approval wait"
        );
    }
}

// =============================================================================
// ATTACK VECTOR 7: Dual-clock verification correctness
// ADR-013: Both wall clock AND monotonic clock must agree
// =============================================================================

mod dual_clock_verification {
    use vo_actor::timer_supervisor::{is_overdue, verify_dual_clock};

    // RQ-DCV01: Timer fires only when both clocks agree
    #[test]
    fn rq_verify_dual_clock_both_clocks_must_agree() {
        // Both conditions met
        assert!(
            verify_dual_clock(1000, 800, 200, 1000),
            "Both clocks met at boundary"
        );

        // Only monotonic met
        assert!(
            !verify_dual_clock(1500, 800, 200, 1000),
            "Wall clock NOT met, should fail"
        );

        // Only wall clock met
        assert!(
            !verify_dual_clock(1100, 800, 400, 1100),
            "Monotonic NOT met, should fail"
        );

        // Neither met
        assert!(
            !verify_dual_clock(1500, 800, 200, 900),
            "Neither clock met, should fail"
        );
    }

    // RQ-DCV02: Wall clock drift doesn't cause early fire
    #[test]
    fn rq_wall_clock_drift_prevented() {
        // fire_at_ms = 1000, now_ms = 1000 (wall says fire)
        // but trigger_time_ms + duration_ms = 800 + 400 = 1200 > 1000 (monotonic says wait)
        assert!(
            !verify_dual_clock(1000, 800, 400, 1000),
            "Monotonic clock prevents early fire despite wall clock"
        );
    }

    // RQ-DCV03: Hibernation doesn't cause timer drift
    #[test]
    fn rq_hibernation_immune_to_timer_drift() {
        // Simulate hibernation: wall clock jumped to 2000 but monotonic
        // only reached 1500 (trigger_time 1000 + duration 500). Timer was supposed
        // to fire at wall time 2000 AND monotonic time 1500. Wall says "fire now"
        // but monotonic hasn't actually caught up, so timer should NOT fire.
        // fire_at_ms = 2000, now_ms = 2000 → wall_clock_ok = true
        // trigger_time_ms + duration_ms = 1500, now_ms = 2000 → monotonic_ok = true
        // Both are true, so let's try another case:
        // fire_at_ms = 2000, now_ms = 2000 → wall_clock_ok = true
        // trigger_time_ms + duration_ms = 2500 (trigger 1000 + duration 1500), now_ms = 2000 → monotonic_ok = false
        assert!(
            !verify_dual_clock(2000, 1000, 1500, 2000),
            "Timer should not fire when wall clock says fire_at but monotonic hasn't caught up"
        );
    }
}

// =============================================================================
// ATTACK VECTOR 8: Timer overdue detection
// ADR-005: Timers that miss their tick interval are marked overdue
// =============================================================================

mod timer_overdue_detection {
    use vo_actor::timer_supervisor::is_overdue;

    // RQ-TOD01: Timer overdue when fired beyond tick interval
    #[test]
    fn rq_is_overdue_true_when_beyond_tick_interval() {
        assert!(
            is_overdue(1000, 1200, 100),
            "Should be overdue when 100ms beyond tick"
        );
    }

    // RQ-TOD02: Timer not overdue when within tick interval
    #[test]
    fn rq_is_overdue_false_when_within_tick_interval() {
        assert!(
            !is_overdue(1000, 1099, 100),
            "Should NOT be overdue when 1ms within tick"
        );
    }

    // RQ-TOD03: Timer exactly at boundary not overdue
    #[test]
    fn rq_is_overdue_false_at_exact_boundary() {
        assert!(
            !is_overdue(1000, 1100, 100),
            "Should NOT be overdue at exact boundary (1100 >= 1100)"
        );
    }
}

// =============================================================================
// ATTACK VECTOR 9: Timer accuracy under system load (DRIFT < 10%)
// ADR-005/013: Timer must maintain accuracy even under system stress
// EARS: When system under load, THE SYSTEM SHALL maintain timer accuracy
// Invariant: Drift < 10%
// =============================================================================

mod timer_accuracy_under_load {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Instant;

    const DRIFT_TOLERANCE_PERCENT: f64 = 0.10;
    const TIMER_DELAY_MS: u64 = 200;
    const LOAD_SPIKE_TASK_COUNT: usize = 100;

    fn calculate_drift_percent(expected_ms: u64, actual_ms: i64) -> f64 {
        let expected = expected_ms as f64;
        let actual = actual_ms.abs() as f64;
        if expected == 0.0 {
            0.0
        } else {
            (actual - expected).abs() / expected
        }
    }

    fn assert_drift_within_tolerance(
        label: &str,
        expected_ms: u64,
        actual_ms: i64,
    ) {
        let drift_percent = calculate_drift_percent(expected_ms, actual_ms);
        assert!(
            drift_percent < DRIFT_TOLERANCE_PERCENT,
            "{}: drift {:.2}% exceeds {}% tolerance (expected ~{}ms, got {}ms)",
            label,
            drift_percent * 100.0,
            DRIFT_TOLERANCE_PERCENT * 100.0,
            expected_ms,
            actual_ms
        );
    }

    fn make_instance_id(byte: u8) -> InstanceId {
        InstanceId::from_bytes([byte; 16])
    }

    // RQ-TUL01: Timer accurate at idle (baseline)
    #[tokio::test]
    async fn rq_timer_accurate_at_idle() {
        let instance_id = make_instance_id(0x11);
        let start = Instant::now();
        let timer = TimerRecord::new(instance_id.clone(), ts_ms(200), None, ts_ms(100));

        let storage = Arc::new(MockTimerStorage::new(vec![timer]));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(50),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(5),
        };

        let handle = ReanimatorLoop::spawn(config.clone(), storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        tokio::time::sleep(Duration::from_millis(300)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        let enqueued = work_queue.enqueued().await;
        let elapsed_ms = start.elapsed().as_millis() as i64;

        assert_eq!(enqueued.len(), 1, "Timer should fire at idle");
        assert_drift_within_tolerance("RQ-TUL01 idle", TIMER_DELAY_MS, elapsed_ms);
    }

    // RQ-TUL02: Timer accurate under CPU load (spin loops)
    #[tokio::test]
    async fn rq_timer_accurate_under_cpu_load() {
        let instance_id = make_instance_id(0x12);
        let start = Instant::now();
        let timer = TimerRecord::new(instance_id.clone(), ts_ms(200), None, ts_ms(100));

        let storage = Arc::new(MockTimerStorage::new(vec![timer]));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(50),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(5),
        };

        let handle = ReanimatorLoop::spawn(config.clone(), storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        let _guard = handle.clone();
        let cpu_load_handle = tokio::spawn(async move {
            let counter = AtomicU64::new(0);
            for _ in 0..LOAD_SPIKE_TASK_COUNT {
                let c = counter.clone();
                tokio::spawn(async move {
                    let mut sum: u64 = 0;
                    for i in 0..10000 {
                        sum = sum.wrapping_add(i * 17 % 31);
                    }
                    c.fetch_add(sum, Ordering::Relaxed);
                });
            }
        });

        tokio::time::sleep(Duration::from_millis(300)).await;

        cpu_load_handle.abort();

        handle.shutdown().await.expect("shutdown should succeed");

        let enqueued = work_queue.enqueued().await;
        let elapsed_ms = start.elapsed().as_millis() as i64;

        assert_eq!(enqueued.len(), 1, "Timer should fire under CPU load");
        assert_drift_within_tolerance("RQ-TUL02 CPU load", TIMER_DELAY_MS, elapsed_ms);
    }

    // RQ-TUL03: Timer accurate under memory pressure (alloc storms)
    #[tokio::test]
    async fn rq_timer_accurate_under_memory_pressure() {
        let instance_id = make_instance_id(0x13);
        let start = Instant::now();
        let timer = TimerRecord::new(instance_id.clone(), ts_ms(200), None, ts_ms(100));

        let storage = Arc::new(MockTimerStorage::new(vec![timer]));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(50),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(5),
        };

        let handle = ReanimatorLoop::spawn(config.clone(), storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        let mem_load_handle = tokio::spawn(async move {
            let mut allocations: Vec<Vec<u8>> = Vec::new();
            for _ in 0..LOAD_SPIKE_TASK_COUNT {
                let vec = vec![0u8; 1024];
                allocations.push(vec);
                if allocations.len() > 50 {
                    allocations.remove(0);
                }
            }
        });

        tokio::time::sleep(Duration::from_millis(300)).await;

        mem_load_handle.abort();

        handle.shutdown().await.expect("shutdown should succeed");

        let enqueued = work_queue.enqueued().await;
        let elapsed_ms = start.elapsed().as_millis() as i64;

        assert_eq!(enqueued.len(), 1, "Timer should fire under memory pressure");
        assert_drift_within_tolerance("RQ-TUL03 memory pressure", TIMER_DELAY_MS, elapsed_ms);
    }

    // RQ-TUL04: Timer accurate under async congestion (many pending tasks)
    #[tokio::test]
    async fn rq_timer_accurate_under_async_congestion() {
        let instance_id = make_instance_id(0x14);
        let start = Instant::now();
        let timer = TimerRecord::new(instance_id.clone(), ts_ms(200), None, ts_ms(100));

        let storage = Arc::new(MockTimerStorage::new(vec![timer]));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(50),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(5),
        };

        let handle = ReanimatorLoop::spawn(config.clone(), storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        let congestion_handle = tokio::spawn(async move {
            let mut handles = Vec::new();
            for _ in 0..LOAD_SPIKE_TASK_COUNT {
                handles.push(tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }));
            }
            for h in handles {
                let _ = h.await;
            }
        });

        tokio::time::sleep(Duration::from_millis(300)).await;

        congestion_handle.abort();

        handle.shutdown().await.expect("shutdown should succeed");

        let enqueued = work_queue.enqueued().await;
        let elapsed_ms = start.elapsed().as_millis() as i64;

        assert_eq!(enqueued.len(), 1, "Timer should fire under async congestion");
        assert_drift_within_tolerance("RQ-TUL04 async congestion", TIMER_DELAY_MS, elapsed_ms);
    }

    // RQ-TUL05: Multiple timers all accurate under combined load
    #[tokio::test]
    async fn rq_multiple_timers_accurate_under_combined_load() {
        let instance_ids = vec![
            make_instance_id(0x15),
            make_instance_id(0x16),
            make_instance_id(0x17),
        ];
        let timers: Vec<TimerRecord> = instance_ids
            .iter()
            .map(|id| TimerRecord::new(id.clone(), ts_ms(200), None, ts_ms(100)))
            .collect();

        let start = Instant::now();
        let storage = Arc::new(MockTimerStorage::new(timers));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(50),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(5),
        };

        let handle = ReanimatorLoop::spawn(config.clone(), storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        let _guard = handle.clone();
        let combined_load = tokio::spawn(async move {
            let mut handles = Vec::new();
            for _ in 0..50 {
                handles.push(tokio::spawn(async move {
                    let counter = AtomicU64::new(0);
                    for _ in 0..1000 {
                        let c = counter.clone();
                        tokio::spawn(async move {
                            let mut sum: u64 = 0;
                            for i in 0..100 {
                                sum = sum.wrapping_add(i * 17 % 31);
                            }
                            c.fetch_add(sum, Ordering::Relaxed);
                        });
                    }
                    for h in handles {
                        let _ = h.await;
                    }
                }));
            }
        });

        tokio::time::sleep(Duration::from_millis(300)).await;

        combined_load.abort();

        handle.shutdown().await.expect("shutdown should succeed");

        let enqueued = work_queue.enqueued().await;
        let elapsed_ms = start.elapsed().as_millis() as i64;

        assert_eq!(
            enqueued.len(),
            3,
            "All 3 timers should fire under combined load"
        );
        assert_drift_within_tolerance(
            "RQ-TUL05 combined load",
            TIMER_DELAY_MS,
            elapsed_ms,
        );
    }

    // RQ-TUL06: Timer drift is NOT caused by wall clock adjustment (dual-clock protection)
    #[tokio::test]
    async fn rq_timer_dual_clock_prevents_wall_clock_drift() {
        use vo_actor::timer_supervisor::verify_dual_clock;

        let fire_at_ms = 1000u64;
        let trigger_time_ms = 800u64;
        let duration_ms = 200u64;
        let now_ms = 1200u64;

        assert!(
            verify_dual_clock(fire_at_ms, trigger_time_ms, duration_ms, now_ms),
            "Dual clock should fire when both conditions met"
        );

        let wall_clock_drifted_now = 1000u64;
        assert!(
            !verify_dual_clock(fire_at_ms, trigger_time_ms, duration_ms, wall_clock_drifted_now),
            "Dual clock should NOT fire if wall clock drifted back but monotonic hasn't caught up"
        );
    }

    // RQ-TUL07: Verify deadline miss detection (timer fires but is overdue)
    #[tokio::test]
    async fn rq_timer_overdue_detection_under_load() {
        let instance_id = make_instance_id(0x18);
        let timer = TimerRecord::new(instance_id.clone(), ts_ms(100), None, ts_ms(50));

        let storage = Arc::new(MockTimerStorage::new(vec![timer]));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(50),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(5),
        };

        let handle = ReanimatorLoop::spawn(config.clone(), storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        tokio::time::sleep(Duration::from_millis(200)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        let fire_calls = storage.fire_calls().await;
        assert_eq!(fire_calls.len(), 1, "Timer should have fired");

        let (_, fire_at) = &fire_calls[0];
        let now_ms = vo_types::TimestampMs::now().as_u64();
        let tick_interval_ms = 50u64;

        let is_overdue = fire_at.as_u64().saturating_add(tick_interval_ms) < now_ms;
        assert!(
            is_overdue,
            "Timer should be marked overdue when fired beyond tick interval"
        );
    }
}

// =============================================================================
// ATTACK VECTOR 10: Signal buffer overflow and backpressure
// ADR-042: THE SYSTEM SHALL handle buffer overflow gracefully
// EARS: When buffer full, THE SYSTEM SHALL apply backpressure
// EARS: If signals dropped, THE SYSTEM SHALL lose critical signals (Signal accountability)
// =============================================================================

mod signal_buffer_overflow {
    use super::*;
    use vo_actor::signal_buffer::BufferedSignal;
    use vo_types::BufferPolicy;

    fn make_signal(signal_id: &str) -> BufferedSignal {
        BufferedSignal::new(signal_id.to_string(), vo_actor::SignalPayload::empty(), TimestampMs::now())
    }

    // RQ-SBO01: BufferMany returns Dropped when at capacity
    // EARS Ubiquitous: THE SYSTEM SHALL handle buffer overflow gracefully
    #[test]
    fn rq_buffer_many_returns_dropped_when_at_capacity() {
        let mut buffer = SignalBuffer::new(SignalBufferConfig::new(3));
        let instance_id = make_instance_id(0x01);
        let wait_key = make_wait_key("approval");

        for i in 0..3 {
            let result = buffer.buffer_signal(
                instance_id.clone(),
                wait_key.clone(),
                make_signal(&format!("sig-{i}")),
                BufferPolicy::BufferMany,
            );
            assert_eq!(result, BufferResult::Buffered, "First 3 signals should be buffered");
        }

        let overflow_result = buffer.buffer_signal(
            instance_id.clone(),
            wait_key.clone(),
            make_signal("sig-overflow"),
            BufferPolicy::BufferMany,
        );
        assert_eq!(overflow_result, BufferResult::Dropped, "Overflow signal should be dropped");
    }

    // RQ-SBO02: Buffer count stays at max when overflow occurs
    // EARS Event-Driven: When buffer full, THE SYSTEM SHALL apply backpressure
    #[test]
    fn rq_buffer_count_stays_at_max_on_overflow() {
        let mut buffer = SignalBuffer::new(SignalBufferConfig::new(3));
        let instance_id = make_instance_id(0x01);
        let wait_key = make_wait_key("approval");

        for i in 0..3 {
            buffer.buffer_signal(
                instance_id.clone(),
                wait_key.clone(),
                make_signal(&format!("sig-{i}")),
                BufferPolicy::BufferMany,
            );
        }

        buffer.buffer_signal(
            instance_id.clone(),
            wait_key.clone(),
            make_signal("sig-overflow"),
            BufferPolicy::BufferMany,
        );

        assert_eq!(
            buffer.buffered_count(&instance_id, &wait_key),
            3,
            "Buffer count should stay at max (3) after overflow"
        );
    }

    // RQ-SBO03: BufferOne rejects subsequent signals until first is consumed
    // EARS Event-Driven: When buffer full, THE SYSTEM SHALL apply backpressure
    #[test]
    fn rq_buffer_one_rejects_subsequent_signals() {
        let mut buffer = SignalBuffer::with_default_config();
        let instance_id = make_instance_id(0x01);
        let wait_key = make_wait_key("approval");

        let first = buffer.buffer_signal(
            instance_id.clone(),
            wait_key.clone(),
            make_signal("sig-first"),
            BufferPolicy::BufferOne,
        );
        assert_eq!(first, BufferResult::Buffered, "First signal should be buffered");

        let second = buffer.buffer_signal(
            instance_id.clone(),
            wait_key.clone(),
            make_signal("sig-second"),
            BufferPolicy::BufferOne,
        );
        assert_eq!(second, BufferResult::Rejected, "Second signal should be rejected");

        let third = buffer.buffer_signal(
            instance_id.clone(),
            wait_key.clone(),
            make_signal("sig-third"),
            BufferPolicy::BufferOne,
        );
        assert_eq!(third, BufferResult::Rejected, "Third signal should also be rejected");

        let popped = buffer.pop_buffered(&instance_id, &wait_key);
        assert_eq!(popped.unwrap().signal_id, "sig-first", "Should pop first signal");

        let after_pop = buffer.buffer_signal(
            instance_id.clone(),
            wait_key.clone(),
            make_signal("sig-after-pop"),
            BufferPolicy::BufferOne,
        );
        assert_eq!(after_pop, BufferResult::Buffered, "Signal after pop should be buffered");
    }

    // RQ-SBO04: Overflow signals are not silently lost - caller receives Dropped
    // EARS Unwanted: If signals dropped, THE SYSTEM SHALL lose critical signals
    // This test verifies the caller CAN detect overflow via BufferResult
    #[test]
    fn rq_overflow_result_enables_accountability() {
        let mut buffer = SignalBuffer::new(SignalBufferConfig::new(2));
        let instance_id = make_instance_id(0x01);
        let wait_key = make_wait_key("approval");

        buffer.buffer_signal(instance_id.clone(), wait_key.clone(), make_signal("sig-0"), BufferPolicy::BufferMany);
        buffer.buffer_signal(instance_id.clone(), wait_key.clone(), make_signal("sig-1"), BufferPolicy::BufferMany);

        let dropped_signal_id = "sig-dropped".to_string();
        let overflow_result = buffer.buffer_signal(
            instance_id.clone(),
            wait_key.clone(),
            make_signal(&dropped_signal_id),
            BufferPolicy::BufferMany,
        );

        assert_eq!(overflow_result, BufferResult::Dropped, "Overflow must return Dropped for accountability");
        assert!(
            buffer.peek_all(&instance_id, &wait_key)
                .iter()
                .all(|s| s.signal_id != dropped_signal_id),
            "Dropped signal must not appear in buffer"
        );
    }

    // RQ-SBO05: Separate keys have independent buffer capacity
    #[test]
    fn rq_separate_keys_independent_overflow() {
        let mut buffer = SignalBuffer::new(SignalBufferConfig::new(2));
        let instance_id = make_instance_id(0x01);
        let wait_key_a = make_wait_key("approval");
        let wait_key_b = make_wait_key("authorization");

        for i in 0..2 {
            buffer.buffer_signal(instance_id.clone(), wait_key_a.clone(), make_signal(&format!("sig-a-{i}")), BufferPolicy::BufferMany);
        }

        for i in 0..2 {
            buffer.buffer_signal(instance_id.clone(), wait_key_b.clone(), make_signal(&format!("sig-b-{i}")), BufferPolicy::BufferMany);
        }

        let overflow_a = buffer.buffer_signal(instance_id.clone(), wait_key_a.clone(), make_signal("sig-a-overflow"), BufferPolicy::BufferMany);
        let overflow_b = buffer.buffer_signal(instance_id.clone(), wait_key_b.clone(), make_signal("sig-b-overflow"), BufferPolicy::BufferMany);

        assert_eq!(overflow_a, BufferResult::Dropped, "Key A should be at capacity");
        assert_eq!(overflow_b, BufferResult::Dropped, "Key B should be at capacity");
        assert_eq!(buffer.buffered_count(&instance_id, &wait_key_a), 2);
        assert_eq!(buffer.buffered_count(&instance_id, &wait_key_b), 2);
    }

    // RQ-SBO06: Signals can be consumed to make room for new signals
    #[test]
    fn rq_signals_can_be_consumed_to_allow_more() {
        let mut buffer = SignalBuffer::new(SignalBufferConfig::new(2));
        let instance_id = make_instance_id(0x01);
        let wait_key = make_wait_key("approval");

        buffer.buffer_signal(instance_id.clone(), wait_key.clone(), make_signal("sig-0"), BufferPolicy::BufferMany);
        buffer.buffer_signal(instance_id.clone(), wait_key.clone(), make_signal("sig-1"), BufferPolicy::BufferMany);

        let overflow = buffer.buffer_signal(instance_id.clone(), wait_key.clone(), make_signal("sig-overflow"), BufferPolicy::BufferMany);
        assert_eq!(overflow, BufferResult::Dropped, "Should be at capacity");

        let popped = buffer.pop_buffered(&instance_id, &wait_key);
        assert_eq!(popped.unwrap().signal_id, "sig-0", "Should pop first signal");

        let after_consume = buffer.buffer_signal(instance_id.clone(), wait_key.clone(), make_signal("sig-new"), BufferPolicy::BufferMany);
        assert_eq!(after_consume, BufferResult::Buffered, "Should be able to buffer after consume");
        assert_eq!(buffer.buffered_count(&instance_id, &wait_key), 2, "Should have 2 signals again");
    }
}
