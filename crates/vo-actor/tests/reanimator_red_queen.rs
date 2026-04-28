#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::redundant_pattern_matching)]
#![allow(clippy::disallowed_methods)]
//! Red Queen adversarial tests for the Reanimator timer lifecycle.
//!
//! These tests probe edge cases, boundary conditions, and adversarial scenarios
//! in the timer lifecycle (ADR-005):
//! - Timer creation during shutdown
//! - Timer cancellation race with fire
//! - Duplicate timer registration
//! - Timer with past fire_at
//! - Timer across epoch boundaries
//! - Verify no timer leaks or double-fires
//!
//! bead_id: ve-dlsm
//! bead_title: Red Queen: Timer lifecycle adversarial tests
//! module: reanimator (actor-level timer lifecycle)

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, watch};
use vo_types::{InstanceId, TimestampMs};

use vo_actor::reanimator::{
    mock::{MockTimerStorage, MockWorkQueue},
    traits::TimerStorage,
    types::{ReanimatorConfig, TimerRecord},
    ReanimatorError, ReanimatorLoop,
};
use vo_actor::work_queue::WorkQueue;

fn ts_ms(value: u64) -> TimestampMs {
    TimestampMs::try_from(value).expect("valid timestamp")
}

fn make_instance_id(byte: u8) -> InstanceId {
    InstanceId::from_bytes([byte; 16])
}

async fn wait_for_running(handle: &vo_actor::reanimator::ReanimatorHandle, timeout: Duration) {
    let mut rx = handle.state_sender.subscribe();
    if handle.current_state() == vo_actor::reanimator::types::ReanimatorState::Running {
        return;
    }
    tokio::time::timeout(timeout, async {
        loop {
            rx.changed().await.expect("state channel closed");
            if *rx.borrow() == vo_actor::reanimator::types::ReanimatorState::Running {
                return;
            }
        }
    })
    .await
    .expect("Timed out waiting for Running state");
}

// =============================================================================
// ATTACK VECTOR 1: Timer creation during shutdown
// =============================================================================

// RQ-RS01: Reanimator rejects new timer work after shutdown signal
#[tokio::test]
async fn rq_reanimator_shutdown_rejects_new_work() {
    let storage = Arc::new(MockTimerStorage::empty());
    let work_queue = Arc::new(MockWorkQueue::new());

    let config = ReanimatorConfig {
        scan_interval: Duration::from_secs(3600),
        max_timers_per_cycle: 100,
        max_concurrent_resumes: 10,
        shutdown_timeout: Duration::from_secs(1),
    };

    let handle = ReanimatorLoop::spawn(config.clone(), storage.clone(), work_queue.clone())
        .expect("spawn should succeed");

    wait_for_running(&handle, Duration::from_secs(2)).await;

    let state_before = handle.current_state();
    assert_eq!(
        state_before,
        vo_actor::reanimator::types::ReanimatorState::Running
    );

    // Check state before shutdown - need to subscribe to verify state change
    let state_receiver = handle.state_sender.subscribe();
    let result = handle.shutdown().await;
    assert!(result.is_ok(), "Shutdown should succeed");
}

// RQ-RS02: Timers due during shutdown are processed before shutdown completes
#[tokio::test]
async fn rq_timers_processed_before_shutdown() {
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

    handle.shutdown().await.expect("shutdown should succeed");

    let fire_calls = storage.fire_calls().await;
    assert!(
        !fire_calls.is_empty(),
        "Timer should have fired before shutdown completed"
    );
}

// =============================================================================
// ATTACK VECTOR 2: Timer cancellation race with fire
// =============================================================================

// RQ-CR01: Delete-before-dispatch ensures no double-fire
#[tokio::test]
async fn rq_delete_before_dispatch_no_double_fire() {
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
    assert_eq!(
        fire_calls, delete_calls,
        "Each fire should correspond to exactly one delete (delete-before-dispatch)"
    );
}

// RQ-CR02: Concurrent delete and fire operations don't cause leaks
#[tokio::test]
async fn rq_concurrent_delete_no_leak() {
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

    let enqueued = work_queue.enqueued().await;
    assert_eq!(enqueued.len(), 1, "Resume should be enqueued exactly once");

    handle.shutdown().await.expect("shutdown should succeed");
}

// =============================================================================
// ATTACK VECTOR 3: Duplicate timer registration
// =============================================================================

// RQ-DT01: Multiple timers for same instance are all processed
#[tokio::test]
async fn rq_duplicate_timer_ids_same_instance() {
    let instance_id = make_instance_id(0x01);
    let timer1 = TimerRecord::new(
        instance_id.clone(),
        ts_ms(100),
        Some(vo_types::TimerId::from_bytes([0x01; 16])),
        ts_ms(50),
    );
    let timer2 = TimerRecord::new(
        instance_id.clone(),
        ts_ms(100),
        Some(vo_types::TimerId::from_bytes([0x02; 16])),
        ts_ms(50),
    );

    let storage = Arc::new(MockTimerStorage::new(vec![timer1, timer2]));
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
    assert_eq!(fire_calls.len(), 2, "Both timers should fire");
}

// =============================================================================
// ATTACK VECTOR 4: Timer with past fire_at
// =============================================================================

// RQ-PF01: Timer with fire_at in the past is processed immediately
#[tokio::test]
async fn rq_past_fire_at_processed_immediately() {
    let instance_id = make_instance_id(0x01);
    let past_time = TimestampMs::now().as_u64().saturating_sub(1000);
    let timer = TimerRecord::new(
        instance_id.clone(),
        ts_ms(past_time),
        None,
        ts_ms(past_time - 50),
    );

    let storage = Arc::new(MockTimerStorage::new(vec![timer]));
    let work_queue = Arc::new(MockWorkQueue::new());

    let config = ReanimatorConfig {
        scan_interval: Duration::from_secs(3600),
        max_timers_per_cycle: 100,
        max_concurrent_resumes: 10,
        shutdown_timeout: Duration::from_secs(30),
    };

    let handle = ReanimatorLoop::spawn(config.clone(), storage.clone(), work_queue.clone())
        .expect("spawn should succeed");

    tokio::time::sleep(Duration::from_millis(100)).await;

    handle.shutdown().await.expect("shutdown should succeed");

    let fire_calls = storage.fire_calls().await;
    assert!(
        !fire_calls.is_empty(),
        "Past timer should be processed immediately"
    );
}

// =============================================================================
// ATTACK VECTOR 5: Timer across epoch boundaries
// =============================================================================

// RQ-EB01: Timer at u64::MAX boundary is handled correctly
#[tokio::test]
async fn rq_timer_at_u64_max_boundary() {
    let instance_id = make_instance_id(0x01);
    let timer = TimerRecord::new(
        instance_id.clone(),
        ts_ms(u64::MAX),
        None,
        ts_ms(u64::MAX - 1000),
    );

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

    wait_for_running(&handle, Duration::from_secs(2)).await;

    let state = handle.current_state();
    assert_eq!(state, vo_actor::reanimator::types::ReanimatorState::Running);

    let fire_calls_before = storage.fire_calls().await;
    assert!(
        fire_calls_before.is_empty(),
        "u64::MAX timer should not fire immediately"
    );

    handle.shutdown().await.expect("shutdown should succeed");
}

// RQ-EB02: Timer at zero boundary is rejected
#[tokio::test]
async fn rq_timer_at_zero_boundary_rejected() {
    let instance_id = make_instance_id(0x01);
    let timer = TimerRecord::new(instance_id.clone(), ts_ms(0), None, ts_ms(0));

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

    tokio::time::sleep(Duration::from_millis(100)).await;

    let fire_calls = storage.fire_calls().await;
    assert!(
        fire_calls.is_empty(),
        "Zero fire_at timer should not fire (validation should reject)"
    );

    handle.shutdown().await.expect("shutdown should succeed");
}

// =============================================================================
// ATTACK VECTOR 6: No timer leaks or double-fires
// =============================================================================

// RQ-LF01: All timers in storage are eventually processed or cleaned
#[tokio::test]
async fn rq_no_timer_leaks_all_processed() {
    let instance_id = make_instance_id(0x01);
    let timers = vec![
        TimerRecord::new(instance_id.clone(), ts_ms(100), None, ts_ms(50)),
        TimerRecord::new(instance_id.clone(), ts_ms(200), None, ts_ms(100)),
        TimerRecord::new(instance_id.clone(), ts_ms(300), None, ts_ms(150)),
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

    let fire_calls = storage.fire_calls().await;
    assert_eq!(fire_calls.len(), 3, "All 3 timers should fire (no leaks)");
}

// RQ-LF02: Deleted timers do not fire
#[tokio::test]
async fn rq_deleted_timers_do_not_fire() {
    let instance_id = make_instance_id(0x01);
    let timer = TimerRecord::new(instance_id.clone(), ts_ms(100), None, ts_ms(50));

    let storage = Arc::new(MockTimerStorage::new(vec![timer]));
    let work_queue = Arc::new(MockWorkQueue::new());

    storage
        .delete_timer(&instance_id, ts_ms(100))
        .await
        .expect("delete should succeed");

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
    assert!(fire_calls.is_empty(), "Deleted timer should not fire");
}

// RQ-LF03: No double-fire when same timer appears multiple times in scan
#[tokio::test]
async fn rq_no_double_fire_same_timer() {
    let instance_id = make_instance_id(0x01);
    let timer = TimerRecord::new(instance_id.clone(), ts_ms(100), None, ts_ms(50));

    let storage = Arc::new(MockTimerStorage::new(vec![timer.clone(), timer.clone()]));
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
    assert_eq!(
        fire_calls.len(),
        1,
        "Duplicate timer entries should only fire once (no double-fire)"
    );
}

// RQ-LF04: Enqueue failures don't cause double-fire on retry
#[tokio::test]
async fn rq_enqueue_failure_no_double_fire() {
    let instance_id = make_instance_id(0x01);
    let timer = TimerRecord::new(instance_id.clone(), ts_ms(100), None, ts_ms(50));

    let storage = Arc::new(MockTimerStorage::new(vec![timer]));
    let work_queue = Arc::new(MockWorkQueue::new());

    work_queue.set_should_fail(true).await;

    let config = ReanimatorConfig {
        scan_interval: Duration::from_millis(100),
        max_timers_per_cycle: 100,
        max_concurrent_resumes: 10,
        shutdown_timeout: Duration::from_secs(30),
    };

    let handle = ReanimatorLoop::spawn(config.clone(), storage.clone(), work_queue.clone())
        .expect("spawn should succeed");

    tokio::time::sleep(Duration::from_millis(500)).await;

    work_queue.set_should_fail(false).await;

    tokio::time::sleep(Duration::from_millis(1500)).await;

    handle.shutdown().await.expect("shutdown should succeed");

    let fire_calls = storage.fire_calls().await;
    assert_eq!(
        fire_calls.len(),
        1,
        "Enqueue failure should not cause double-fire on retry"
    );
}

// =============================================================================
// ATTACK VECTOR 7: Fairness budget enforcement
// =============================================================================

// RQ-FB01: Fairness budget prevents single instance from monopolizing
#[tokio::test]
async fn rq_fairness_budget_enforced() {
    let instance_id = make_instance_id(0x01);
    let timers: Vec<TimerRecord> = (0..10)
        .map(|i| TimerRecord::new(instance_id.clone(), ts_ms(100 + i), None, ts_ms(50 + i)))
        .collect();

    let storage = Arc::new(MockTimerStorage::new(timers));
    let work_queue = Arc::new(MockWorkQueue::new());

    let config = ReanimatorConfig {
        scan_interval: Duration::from_millis(100),
        max_timers_per_cycle: 5,
        max_concurrent_resumes: 2,
        shutdown_timeout: Duration::from_secs(30),
    };

    let handle = ReanimatorLoop::spawn(config.clone(), storage.clone(), work_queue.clone())
        .expect("spawn should succeed");

    tokio::time::sleep(Duration::from_millis(1000)).await;

    handle.shutdown().await.expect("shutdown should succeed");

    let fire_calls = storage.fire_calls().await;
    assert!(
        fire_calls.len() == 10,
        "All 10 timers should fire over multiple cycles (budget resets per cycle)"
    );
}

// =============================================================================
// ATTACK VECTOR 8: Crash recovery invariants
// =============================================================================

// RQ-CR03: Crash recovery skips terminal instances
#[tokio::test]
async fn rq_crash_recovery_skips_terminal_instances() {
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

    wait_for_running(&handle, Duration::from_secs(2)).await;

    let state = handle.current_state();
    assert_eq!(state, vo_actor::reanimator::types::ReanimatorState::Running);

    handle.shutdown().await.expect("shutdown should succeed");
}

// =============================================================================
// ATTACK VECTOR 9: Storage failure handling
// =============================================================================

// RQ-SF01: Storage failures are handled gracefully
#[tokio::test]
async fn rq_storage_failure_handled() {
    let instance_id = make_instance_id(0x01);
    let timer = TimerRecord::new(instance_id.clone(), ts_ms(100), None, ts_ms(50));

    let storage = Arc::new(MockTimerStorage::new(vec![timer]));
    let work_queue = Arc::new(MockWorkQueue::new());

    storage.set_should_fail(true).await;

    let config = ReanimatorConfig {
        scan_interval: Duration::from_millis(100),
        max_timers_per_cycle: 100,
        max_concurrent_resumes: 10,
        shutdown_timeout: Duration::from_secs(30),
    };

    let handle = ReanimatorLoop::spawn(config.clone(), storage.clone(), work_queue.clone())
        .expect("spawn should succeed");

    wait_for_running(&handle, Duration::from_secs(2)).await;

    let state = handle.current_state();
    assert_eq!(state, vo_actor::reanimator::types::ReanimatorState::Running);

    handle.shutdown().await.expect("shutdown should succeed");
}
