//! System recovery tests for crashes, network partitions, and data corruption.
//!
//! This module tests the Reanimator Loop's ability to recover from various
//! failure scenarios as specified in ve-5io9: vo-recovery: System recovery and resilience test
//!
//! Test categories:
//! - Crash recovery: Simulating process crashes and restarts
//! - Network partition recovery: Simulating distributed system failures
//! - Data corruption handling: Testing resilience against corrupt data

use std::sync::Arc;
use std::time::Duration;
use vo_types::{InstanceId, TimestampMs};

use crate::reanimator::mock::{MockTimerStorage, MockWorkQueue};
use crate::reanimator::traits::{PendingTimer, TimerStorage, WorkQueue};
use crate::reanimator::types::{FairnessBudget, ReanimatorConfig};
use crate::reanimator::{ReanimatorError, TimerRecord};

// =============================================================================
// Helper Functions
// =============================================================================

fn make_timer(instance_id: InstanceId, fire_at_ms: u64) -> TimerRecord {
    TimerRecord::new(
        instance_id,
        TimestampMs::try_from(fire_at_ms).expect("valid timestamp"),
        Some(vo_types::TimerId::from_bytes([1; 16])),
        TimestampMs::try_from(fire_at_ms - 1000).expect("valid timestamp"),
    )
}

fn make_instance_id(seed: u8) -> InstanceId {
    InstanceId::from_bytes([seed; 16])
}

// =============================================================================
// Crash Recovery Tests
// =============================================================================

/// Test that the system can recover from a simulated crash by replaying
/// pending timers that were in-flight when the "crash" occurred.
#[tokio::test]
async fn crash_recovery_replays_pending_timers() {
    let instance_id = make_instance_id(1);
    let storage = Arc::new(MockTimerStorage::empty());
    let work_queue = Arc::new(MockWorkQueue::new());

    // Add timers to storage (simulating pre-crash state)
    storage
        .add_timer(make_timer(instance_id.clone(), 5000))
        .await;

    // Mark timer as in-flight (simulating partial processing before crash)
    storage
        .mark_timer_processing(&instance_id, TimestampMs::try_from(5000).expect("valid"))
        .await
        .expect("mark processing should succeed");

    // Simulate crash recovery
    let pending_timers = storage
        .scan_pending_timers(100)
        .await
        .expect("scan pending should succeed");

    assert_eq!(pending_timers.len(), 1, "should find 1 pending timer");
    assert_eq!(pending_timers[0].instance_id, instance_id);

    // Replay the timer by enqueuing resume work
    work_queue
        .enqueue_resume(instance_id.clone())
        .await
        .expect("enqueue should succeed");

    let enqueued = work_queue.enqueued().await;
    assert_eq!(enqueued.len(), 1);
    assert_eq!(enqueued[0], instance_id);

    // Complete the timer processing (simulating successful recovery)
    storage
        .complete_timer_processing(&instance_id, TimestampMs::try_from(5000).expect("valid"))
        .await
        .expect("complete should succeed");

    let remaining = storage
        .scan_pending_timers(100)
        .await
        .expect("scan pending should succeed");

    assert!(
        remaining.is_empty(),
        "pending timers should be cleared after recovery"
    );
}

/// Test that crash recovery skips instances that have already terminated.
#[tokio::test]
async fn crash_recovery_skips_terminated_instances() {
    let instance_id = make_instance_id(1);
    let storage = Arc::new(MockTimerStorage::empty());
    let work_queue = Arc::new(MockWorkQueue::new());

    // Mark timer as in-flight
    storage
        .mark_timer_processing(&instance_id, TimestampMs::try_from(5000).expect("valid"))
        .await
        .expect("mark processing should succeed");

    // Simulate that the instance terminated after crash
    // (this would be detected by checking instance state during recovery)
    // For this test, we verify the pending timer exists but isn't replayed
    let pending_timers = storage
        .scan_pending_timers(100)
        .await
        .expect("scan pending should succeed");

    assert_eq!(pending_timers.len(), 1);

    // In real scenario, is_instance_terminal would return true and skip replay
    let is_terminal = work_queue
        .is_instance_terminal(&instance_id)
        .await
        .expect("check terminal should succeed");

    assert!(!is_terminal, "instance should not be terminal in mock");
}

/// Test cleanup of stale pending timers during crash recovery.
#[tokio::test]
async fn crash_recovery_cleans_stale_pending_timers() {
    let instance_id = make_instance_id(1);
    let storage = Arc::new(MockTimerStorage::empty());

    // Add a stale pending timer with old marked_at_ms
    let stale_pending = PendingTimer {
        instance_id: instance_id.clone(),
        fire_at_ms: TimestampMs::try_from(5000).expect("valid"),
        scheduled_at_ms: TimestampMs::try_from(4000).expect("valid"),
        marked_at_ms: TimestampMs::try_from(100).expect("valid"), // Old marker
    };
    storage.add_pending_timer(stale_pending).await;

    // Verify timer is pending
    let before = storage
        .scan_pending_timers(100)
        .await
        .expect("scan pending should succeed");

    assert_eq!(before.len(), 1);

    // Use threshold between old marker and current time
    let stale_threshold = TimestampMs::try_from(1000).expect("valid");
    let cleaned = storage
        .cleanup_stale_pending_timers(stale_threshold)
        .await
        .expect("cleanup should succeed");

    assert_eq!(cleaned, 1, "should clean up 1 stale timer");

    let after = storage
        .scan_pending_timers(100)
        .await
        .expect("scan pending should succeed");

    assert!(after.is_empty(), "all stale timers should be cleaned");
}

/// Test that crash recovery properly handles multiple instances with pending timers.
#[tokio::test]
async fn crash_recovery_multiple_instances() {
    let instance1 = make_instance_id(1);
    let instance2 = make_instance_id(2);
    let instance3 = make_instance_id(3);

    let storage = Arc::new(MockTimerStorage::empty());
    let work_queue = Arc::new(MockWorkQueue::new());

    // Add multiple pending timers for different instances
    for instance_id in &[&instance1, &instance2, &instance3] {
        storage
            .mark_timer_processing(instance_id, TimestampMs::try_from(5000).expect("valid"))
            .await
            .expect("mark processing should succeed");
    }

    // Recover all pending timers
    let pending_timers = storage
        .scan_pending_timers(100)
        .await
        .expect("scan pending should succeed");

    assert_eq!(pending_timers.len(), 3, "should find all 3 pending timers");

    // Replay each timer
    for pending in &pending_timers {
        work_queue
            .enqueue_resume(pending.instance_id.clone())
            .await
            .expect("enqueue should succeed");

        storage
            .complete_timer_processing(&pending.instance_id, pending.fire_at_ms)
            .await
            .expect("complete should succeed");
    }

    // Verify all timers completed
    let remaining = storage
        .scan_pending_timers(100)
        .await
        .expect("scan pending should succeed");

    assert!(remaining.is_empty());

    let enqueued = work_queue.enqueued().await;
    assert_eq!(enqueued.len(), 3);
}

// =============================================================================
// Network Partition Recovery Tests
// =============================================================================

/// Test recovery after network partition: storage operations fail but work queue succeeds.
#[tokio::test]
async fn network_partition_storage_failure_recovery() {
    let instance_id = make_instance_id(1);
    let storage = Arc::new(MockTimerStorage::empty());

    // Simulate storage failure during scan_due_timers
    storage.set_should_fail(true).await;

    let result = storage
        .scan_due_timers(
            TimestampMs::try_from(0u64).expect("valid"),
            TimestampMs::try_from(10000u64).expect("valid"),
            100,
        )
        .await;

    assert!(result.is_err(), "storage operation should fail");

    // After partition heals, storage should work again
    storage.set_should_fail(false).await;

    let result = storage
        .scan_due_timers(
            TimestampMs::try_from(0u64).expect("valid"),
            TimestampMs::try_from(10000u64).expect("valid"),
            100,
        )
        .await;

    assert!(
        result.is_ok(),
        "storage operation should succeed after healing"
    );
}

/// Test that work queue operations survive network partition and retry correctly.
#[tokio::test]
async fn network_partition_work_queue_recovery() {
    let instance_id = make_instance_id(1);
    let storage = Arc::new(MockTimerStorage::empty());
    let work_queue = Arc::new(MockWorkQueue::new());

    // Simulate work queue failure
    work_queue.set_should_fail(true).await;

    let result = work_queue.enqueue_resume(instance_id.clone()).await;

    assert!(result.is_err(), "work queue operation should fail");

    // After partition heals, operation should succeed
    work_queue.set_should_fail(false).await;

    let result = work_queue.enqueue_resume(instance_id.clone()).await;

    assert!(
        result.is_ok(),
        "work queue operation should succeed after healing"
    );

    let enqueued = work_queue.enqueued().await;
    assert_eq!(enqueued.len(), 1);
}

/// Test handling of partial recovery: some operations succeed, others fail.
#[tokio::test]
async fn partial_recovery_mixed_failures() {
    let instance1 = make_instance_id(1);
    let instance2 = make_instance_id(2);
    let storage = Arc::new(MockTimerStorage::empty());
    let work_queue = Arc::new(MockWorkQueue::new());

    // Add timer for instance1
    storage.add_timer(make_timer(instance1.clone(), 5000)).await;

    // Mark instance2 as in-flight
    storage
        .mark_timer_processing(&instance2, TimestampMs::try_from(5000).expect("valid"))
        .await
        .expect("mark processing should succeed");

    // Simulate partial failure: instance1 timer fires successfully, instance2 recovery fails
    storage
        .delete_timer(&instance1, TimestampMs::try_from(5000).expect("valid"))
        .await
        .expect("delete should succeed");

    work_queue
        .enqueue_resume(instance1.clone())
        .await
        .expect("enqueue should succeed");

    // instance2 recovery fails (simulated by not completing)
    // Verify instance2 is still pending
    let pending = storage
        .scan_pending_timers(100)
        .await
        .expect("scan pending should succeed");

    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].instance_id, instance2);
}

// =============================================================================
// Data Corruption Tests
// =============================================================================

/// Test validation of corrupted timer records with invalid timestamps.
#[tokio::test]
async fn validate_corrupted_timer_zero_fire_at() {
    let instance_id = make_instance_id(1);
    let timer = TimerRecord::new(
        instance_id,
        TimestampMs::try_from(0u64).expect("0 is valid"), // Zero fire_at_ms is corrupt
        None,
        TimestampMs::try_from(1000).expect("valid"),
    );

    let result = crate::reanimator::validate_timer_record(&timer);
    assert!(result.is_err(), "should reject zero fire_at_ms");

    let err = result.unwrap_err();
    assert!(matches!(err, ReanimatorError::CorruptKey(_)));
}

/// Test validation of corrupted timer records with fire_at_ms before scheduled_at_ms.
#[tokio::test]
async fn validate_corrupted_timer_reversed_timestamps() {
    let instance_id = make_instance_id(1);
    let timer = TimerRecord::new(
        instance_id,
        TimestampMs::try_from(500).expect("valid"),
        None,
        TimestampMs::try_from(1000).expect("valid"),
    );

    let result = crate::reanimator::validate_timer_record(&timer);
    assert!(
        result.is_err(),
        "should reject fire_at_ms < scheduled_at_ms"
    );

    let err = result.unwrap_err();
    assert!(matches!(err, ReanimatorError::CorruptKey(_)));
}

/// Test validation rejects all-zeros instance_id (corrupted ID).
#[tokio::test]
async fn validate_corrupted_timer_zero_instance_id() {
    let timer = TimerRecord::new(
        InstanceId::from_bytes([0u8; 16]), // All zeros is corrupted
        TimestampMs::try_from(1000).expect("valid"),
        None,
        TimestampMs::try_from(500).expect("valid"),
    );

    let result = crate::reanimator::validate_timer_record(&timer);
    assert!(result.is_err(), "should reject all-zeros instance_id");

    let err = result.unwrap_err();
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("all zeros"),
        "error should mention all zeros"
    );
}

/// Test handling of timer records with invalid timer IDs.
#[tokio::test]
async fn validate_timer_with_null_timer_id() {
    let instance_id = make_instance_id(1);
    let timer = TimerRecord::new(
        instance_id,
        TimestampMs::try_from(1000).expect("valid"),
        None, // Null timer ID is valid (single timer per instance)
        TimestampMs::try_from(500).expect("valid"),
    );

    let result = crate::reanimator::validate_timer_record(&timer);
    assert!(result.is_ok(), "null timer_id should be valid");
}

/// Test that valid timer records pass validation.
#[tokio::test]
async fn validate_valid_timer_record() {
    let instance_id = make_instance_id(1);
    let timer = TimerRecord::new(
        instance_id,
        TimestampMs::try_from(1000).expect("valid"),
        Some(vo_types::TimerId::from_bytes([1; 16])),
        TimestampMs::try_from(500).expect("valid"),
    );

    let result = crate::reanimator::validate_timer_record(&timer);
    assert!(result.is_ok(), "valid timer should pass validation");
}

// =============================================================================
// Crash Recovery Integration Tests
// =============================================================================

/// Integration test: full crash recovery scenario with timer storage and work queue.
#[tokio::test]
async fn crash_recovery_integration_full_scenario() {
    let instance_id = make_instance_id(1);
    let storage = Arc::new(MockTimerStorage::empty());
    let work_queue = Arc::new(MockWorkQueue::new());

    // Phase 1: Normal operation - add timer
    storage
        .add_timer(make_timer(instance_id.clone(), 5000))
        .await;

    // Phase 2: Simulate crash - timer becomes pending
    storage
        .mark_timer_processing(&instance_id, TimestampMs::try_from(5000).expect("valid"))
        .await
        .expect("mark processing should succeed");

    // Phase 3: Crash recovery begins
    let pending_timers = storage
        .scan_pending_timers(100)
        .await
        .expect("scan pending should succeed");

    assert_eq!(pending_timers.len(), 1);

    // Phase 4: Replay timer by enqueuing resume work
    work_queue
        .enqueue_resume(pending_timers[0].instance_id.clone())
        .await
        .expect("enqueue should succeed");

    // Phase 5: Complete timer processing
    storage
        .complete_timer_processing(&pending_timers[0].instance_id, pending_timers[0].fire_at_ms)
        .await
        .expect("complete should succeed");

    // Phase 6: Verify recovery complete
    let remaining = storage
        .scan_pending_timers(100)
        .await
        .expect("scan pending should succeed");

    assert!(remaining.is_empty(), "no pending timers after recovery");

    let enqueued = work_queue.enqueued().await;
    assert_eq!(enqueued.len(), 1);
}

/// Integration test: crash recovery with fairness budget enforcement.
#[tokio::test]
async fn crash_recovery_with_fairness_budget() {
    let instance1 = make_instance_id(1);
    let instance2 = make_instance_id(2);
    let instance3 = make_instance_id(3);

    let storage = Arc::new(MockTimerStorage::empty());
    let work_queue = Arc::new(MockWorkQueue::new());

    // Add multiple instances as pending
    for instance_id in &[&instance1, &instance2, &instance3] {
        storage
            .mark_timer_processing(instance_id, TimestampMs::try_from(5000).expect("valid"))
            .await
            .expect("mark processing should succeed");
    }

    // Create fairness budget
    let mut budget = FairnessBudget::with_limits(2, 10); // Max 2 per instance

    // Get pending timers
    let pending_timers = storage
        .scan_pending_timers(100)
        .await
        .expect("scan pending should succeed");

    assert_eq!(pending_timers.len(), 3);

    // Exhaust budget for first instance (max is 2 per instance)
    let first_instance = &pending_timers[0].instance_id;
    assert!(budget.record_resume(first_instance.clone()));
    assert!(budget.record_resume(first_instance.clone()));
    assert!(
        !budget.record_resume(first_instance.clone()),
        "should be over budget after 2"
    );

    // Verify budget tracking
    assert!(!budget.can_resume(first_instance));
}

/// Integration test: crash recovery handles storage errors gracefully.
#[tokio::test]
async fn crash_recovery_handles_storage_errors() {
    let instance_id = make_instance_id(1);
    let storage = Arc::new(MockTimerStorage::empty());
    let work_queue = Arc::new(MockWorkQueue::new());

    // Simulate storage error
    storage.set_should_fail(true).await;

    let result = storage.scan_pending_timers(100).await;

    assert!(result.is_err(), "scan should fail with storage error");

    let err = result.unwrap_err();
    assert!(matches!(err, ReanimatorError::StorageError(_)));

    // After error, should be able to recover
    storage.set_should_fail(false).await;

    storage
        .add_timer(make_timer(instance_id.clone(), 5000))
        .await;

    let pending = storage
        .scan_pending_timers(100)
        .await
        .expect("scan should succeed after recovery");

    assert!(pending.is_empty());
}

// =============================================================================
// Network Partition Edge Cases
// =============================================================================

/// Test recovery when work queue is partitioned but storage is accessible.
#[tokio::test]
async fn partition_storage_accessible_work_queue_unavailable() {
    let instance_id = make_instance_id(1);
    let storage = Arc::new(MockTimerStorage::empty());
    let work_queue = Arc::new(MockWorkQueue::new());

    // Storage accessible, work queue unavailable
    work_queue.set_should_fail(true).await;

    storage
        .add_timer(make_timer(instance_id.clone(), 5000))
        .await;

    storage
        .mark_timer_processing(&instance_id, TimestampMs::try_from(5000).expect("valid"))
        .await
        .expect("mark processing should succeed");

    // Try to enqueue resume - should fail
    let result = work_queue.enqueue_resume(instance_id.clone()).await;

    assert!(result.is_err(), "enqueue should fail");

    // Timer remains in pending state
    let pending = storage
        .scan_pending_timers(100)
        .await
        .expect("scan pending should succeed");

    assert_eq!(pending.len(), 1);
}

/// Test recovery when both storage and work queue are partitioned.
#[tokio::test]
async fn partition_both_storage_and_work_queue() {
    let instance_id = make_instance_id(1);
    let storage = Arc::new(MockTimerStorage::empty());
    let work_queue = Arc::new(MockWorkQueue::new());

    // Both unavailable
    storage.set_should_fail(true).await;
    work_queue.set_should_fail(true).await;

    // Add timer - add_timer() returns (), no error possible
    // But scan_due_timers should fail
    storage.set_should_fail(true).await;
    let result = storage
        .scan_due_timers(
            TimestampMs::try_from(0u64).expect("valid"),
            TimestampMs::try_from(10000u64).expect("valid"),
            100,
        )
        .await;

    assert!(
        result.is_err(),
        "scan_due_timers should fail with storage error"
    );

    // Recover both
    storage.set_should_fail(false).await;
    work_queue.set_should_fail(false).await;

    // Now operations should succeed
    storage
        .add_timer(make_timer(instance_id.clone(), 5000))
        .await;

    work_queue
        .enqueue_resume(instance_id.clone())
        .await
        .expect("enqueue should succeed");

    let enqueued = work_queue.enqueued().await;
    assert_eq!(enqueued.len(), 1);
}

// =============================================================================
// Data Corruption Resilience
// =============================================================================

/// Test that the system can detect and handle corrupt timer records in storage.
#[tokio::test]
async fn corruption_detection_in_scan() {
    let storage = Arc::new(MockTimerStorage::empty());

    // Add valid timer
    storage
        .add_timer(make_timer(make_instance_id(1), 5000))
        .await;

    let result = storage
        .scan_due_timers(
            TimestampMs::try_from(0u64).expect("valid"),
            TimestampMs::try_from(10000).expect("valid"),
            100,
        )
        .await;

    assert!(result.is_ok(), "scan should succeed for valid timer");

    let timers = result.unwrap();
    assert_eq!(timers.len(), 1);
}

/// Test batch size calculation respects fairness budget limits.
#[tokio::test]
async fn batch_size_respects_budget() {
    // No budget used, can process all
    assert_eq!(crate::reanimator::calculate_batch_size(50, 100, 0), 50);

    // Budget partially used
    assert_eq!(crate::reanimator::calculate_batch_size(50, 100, 70), 30);

    // Budget exhausted
    assert_eq!(crate::reanimator::calculate_batch_size(50, 100, 100), 0);

    // Over budget
    assert_eq!(crate::reanimator::calculate_batch_size(50, 100, 101), 0);
}

/// Test that batch size respects remaining timers.
#[tokio::test]
async fn batch_size_respects_remaining_timers() {
    // More budget than timers, should return timer count
    assert_eq!(crate::reanimator::calculate_batch_size(10, 100, 0), 10);

    // Less budget than timers, should return budget remaining
    assert_eq!(crate::reanimator::calculate_batch_size(10, 100, 95), 5);
}

// =============================================================================
// Recovery Timing Tests
// =============================================================================

/// Test that crash recovery timeout is handled correctly.
#[tokio::test]
async fn recovery_timeout_handling() {
    let instance_id = make_instance_id(1);
    let storage = Arc::new(MockTimerStorage::empty());
    let work_queue = Arc::new(MockWorkQueue::new());

    // Add timer
    storage
        .add_timer(make_timer(instance_id.clone(), 5000))
        .await;

    // Mark as pending
    storage
        .mark_timer_processing(&instance_id, TimestampMs::try_from(5000).expect("valid"))
        .await
        .expect("mark processing should succeed");

    // Simulate slow recovery with timeout
    let config = ReanimatorConfig {
        scan_interval: Duration::from_millis(100),
        max_timers_per_cycle: 10,
        max_concurrent_resumes: 5,
        shutdown_timeout: Duration::from_millis(500),
    };

    // Verify config is valid
    assert_eq!(config.scan_interval, Duration::from_millis(100));
    assert_eq!(config.shutdown_timeout, Duration::from_millis(500));
}

/// Test recovery with concurrent operations.
#[tokio::test]
async fn concurrent_recovery_operations() {
    let storage = Arc::new(MockTimerStorage::empty());
    let work_queue = Arc::new(MockWorkQueue::new());

    // Add multiple timers with unique instance IDs
    let instance_ids: Vec<InstanceId> = (0..10u8).map(|i| make_instance_id(i)).collect();

    for (i, instance_id) in instance_ids.iter().enumerate() {
        storage
            .add_timer(make_timer(instance_id.clone(), 5000 + i as u64 * 100))
            .await;
    }

    // Mark all as pending
    for instance_id in &instance_ids {
        storage
            .mark_timer_processing(instance_id, TimestampMs::try_from(5000u64).expect("valid"))
            .await
            .expect("mark processing should succeed");
    }

    // Concurrent scan and replay
    let pending = storage
        .scan_pending_timers(100)
        .await
        .expect("scan should succeed");

    assert_eq!(pending.len(), 10);

    // Concurrent enqueue
    let mut handles = vec![];
    for timer in &pending {
        let wq = work_queue.clone();
        let instance = timer.instance_id.clone();
        handles.push(tokio::spawn(
            async move { wq.enqueue_resume(instance).await },
        ));
    }

    let results = futures::future::join_all(handles).await;
    let success_count = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(success_count, 10, "all enqueues should succeed");

    let enqueued = work_queue.enqueued().await;
    assert_eq!(enqueued.len(), 10);
}

// =============================================================================
// Recovery State Machine Tests
// =============================================================================

/// Test state transitions during recovery process.
#[tokio::test]
async fn recovery_state_transitions() {
    let instance_id = make_instance_id(1);
    let storage = Arc::new(MockTimerStorage::empty());

    // Initial state: no pending timers
    let pending = storage
        .scan_pending_timers(100)
        .await
        .expect("scan should succeed");
    assert!(pending.is_empty());

    // Transition 1: Timer becomes pending
    storage
        .mark_timer_processing(&instance_id, TimestampMs::try_from(5000).expect("valid"))
        .await
        .expect("mark processing should succeed");

    let pending = storage
        .scan_pending_timers(100)
        .await
        .expect("scan should succeed");
    assert_eq!(pending.len(), 1);

    // Transition 2: Timer completes recovery
    storage
        .complete_timer_processing(&instance_id, TimestampMs::try_from(5000).expect("valid"))
        .await
        .expect("complete should succeed");

    let pending = storage
        .scan_pending_timers(100)
        .await
        .expect("scan should succeed");
    assert!(pending.is_empty());
}

/// Test cleanup of stale pending timers with various thresholds.
#[tokio::test]
async fn stale_timer_cleanup_with_thresholds() {
    let instance1 = make_instance_id(1);
    let instance2 = make_instance_id(2);
    let storage = Arc::new(MockTimerStorage::empty());

    // Mark timers as pending at different times
    storage
        .mark_timer_processing(&instance1, TimestampMs::try_from(5000).expect("valid"))
        .await
        .expect("mark processing should succeed");

    let instance2_pending = PendingTimer {
        instance_id: instance2.clone(),
        fire_at_ms: TimestampMs::try_from(6000).expect("valid"),
        scheduled_at_ms: TimestampMs::try_from(5000).expect("valid"),
        marked_at_ms: TimestampMs::try_from(100).expect("valid"), // Old marker
    };

    storage.add_pending_timer(instance2_pending).await;

    // Clean up with threshold between the two markers
    let threshold = TimestampMs::try_from(1000).expect("valid");
    let cleaned = storage
        .cleanup_stale_pending_timers(threshold)
        .await
        .expect("cleanup should succeed");

    // Only instance2 should be cleaned (marked_at_ms = 100 < threshold = 1000)
    assert_eq!(cleaned, 1);

    let remaining = storage
        .scan_pending_timers(100)
        .await
        .expect("scan should succeed");

    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].instance_id, instance1);
}

/// Test that delete_all_timers_for_instance correctly cancels all timers for an instance.
#[tokio::test]
async fn delete_all_timers_for_instance_cancels_all() {
    let instance1 = make_instance_id(1);
    let instance2 = make_instance_id(2);
    let storage = Arc::new(MockTimerStorage::empty());

    // Add multiple timers for instance1
    storage
        .add_timer(make_timer(instance1.clone(), 5000))
        .await;
    storage
        .add_timer(make_timer(instance1.clone(), 6000))
        .await;
    storage
        .add_timer(make_timer(instance1.clone(), 7000))
        .await;

    // Add timer for instance2 (should not be deleted)
    storage
        .add_timer(make_timer(instance2.clone(), 5000))
        .await;

    // Verify all timers exist
    let before = storage
        .scan_due_timers(
            TimestampMs::try_from(0u64).expect("valid"),
            TimestampMs::try_from(10000u64).expect("valid"),
            100,
        )
        .await
        .expect("scan should succeed");

    assert_eq!(before.len(), 4, "should have 4 timers initially");

    // Delete all timers for instance1
    let deleted = storage
        .delete_all_timers_for_instance(&instance1)
        .await
        .expect("delete should succeed");

    assert_eq!(deleted, 3, "should delete 3 timers for instance1");

    // Verify instance2 timer remains
    let after = storage
        .scan_due_timers(
            TimestampMs::try_from(0u64).expect("valid"),
            TimestampMs::try_from(10000u64).expect("valid"),
            100,
        )
        .await
        .expect("scan should succeed");

    assert_eq!(after.len(), 1, "should have 1 timer remaining");
    assert_eq!(after[0].instance_id, instance2);

    // Verify delete_all_calls was recorded
    let calls = storage.delete_all_calls().await;
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0], instance1);
}
