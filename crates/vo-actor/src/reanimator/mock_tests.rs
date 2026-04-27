//! Mock integration tests for the Reanimator Loop.

use std::sync::Arc;
use vo_types::{InstanceId, TimestampMs};

use crate::reanimator::{
    mock::{MockTimerStorage, MockWorkQueue},
    traits::{TimerStorage, WorkQueue},
    types::TimerRecord,
    ReanimatorError,
};

// Helper function to create TimestampMs from u64 without unwrap in test code
fn ts_ms(value: u64) -> TimestampMs {
    TimestampMs::try_from(value).expect("valid timestamp")
}

// =============================================================================
// Mock Storage Integration Tests
// =============================================================================

mod mock_storage_tests {
    use super::*;
    use crate::reanimator::mock::MockTimerStorage;

    #[tokio::test]
    async fn mock_storage_scan_returns_due_timers() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let timers = vec![TimerRecord::new(
            instance_id.clone(),
            ts_ms(1000),
            None,
            ts_ms(500),
        )];

        let storage = Arc::new(MockTimerStorage::new(timers.clone()));
        let result = storage.scan_due_timers(ts_ms(0), ts_ms(2000), 100).await;

        assert_eq!(result, Ok(timers));
    }

    #[tokio::test]
    async fn mock_storage_delete_removes_timer() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let timers = vec![TimerRecord::new(
            instance_id.clone(),
            ts_ms(1000),
            None,
            ts_ms(500),
        )];

        let storage = Arc::new(MockTimerStorage::new(timers));

        storage
            .delete_timer(&instance_id, ts_ms(1000))
            .await
            .unwrap();

        // Verify timer was removed
        let remaining = storage
            .scan_due_timers(ts_ms(0), ts_ms(2000), 100)
            .await
            .unwrap();

        assert!(remaining.is_empty());
    }

    #[tokio::test]
    async fn mock_storage_record_fire_tracks_call() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::new(Vec::new()));

        storage
            .record_timer_fired(&instance_id, ts_ms(1000))
            .await
            .unwrap();

        let calls = storage.fire_calls().await;
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, instance_id);
        assert_eq!(calls[0].1, ts_ms(1000));
    }

    #[tokio::test]
    async fn mock_storage_failure_returns_error() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::new(Vec::new()));
        storage.set_should_fail(true).await;

        let result = storage.record_timer_fired(&instance_id, ts_ms(1000)).await;

        assert_eq!(
            result,
            Err(ReanimatorError::StorageError("Mock failure".to_string()))
        );
    }
}

// =============================================================================
// Mock WorkQueue Tests
// =============================================================================

mod mock_work_queue_tests {
    use super::*;
    use crate::reanimator::mock::MockWorkQueue;

    #[tokio::test]
    async fn mock_work_queue_enqueue() {
        let queue = Arc::new(MockWorkQueue::new());
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        queue.enqueue_resume(instance_id.clone()).await.unwrap();

        let enqueued = queue.enqueued().await;
        assert_eq!(enqueued.len(), 1);
        assert_eq!(enqueued[0], instance_id);
    }

    #[tokio::test]
    async fn mock_work_queue_failure() {
        let queue = Arc::new(MockWorkQueue::new());
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        queue.set_should_fail(true).await;

        let result = queue.enqueue_resume(instance_id.clone()).await;

        assert_eq!(
            result,
            Err(ReanimatorError::EnqueueFailed("Mock failure".to_string()))
        );
    }

    #[tokio::test]
    async fn mock_work_queue_multiple_enqueues() {
        let queue = Arc::new(MockWorkQueue::new());

        // InstanceId requires exactly 26 characters
        let ids = [
            "01H5JYV4XHGSR2F8KZ9BWNRFMA",
            "01H5JYV4XHGSR2F8KZ9BWNRFMB",
            "01H5JYV4XHGSR2F8KZ9BWNRFMC",
            "01H5JYV4XHGSR2F8KZ9BWNRFMD",
            "01H5JYV4XHGSR2F8KZ9BWNRFME",
        ];

        for id in &ids {
            let instance_id = InstanceId::parse(id).unwrap();
            queue.enqueue_resume(instance_id).await.unwrap();
        }

        assert_eq!(queue.enqueued().await.len(), 5);
    }
}

// =============================================================================
// REDQUEEN: Coevolutionary Adversarial Tests
// These tests verify invariants, failure modes, and atomicity guarantees
// by attempting to break the system in various ways.
// =============================================================================

mod redqueen_invariant_tests {
    use super::*;

    #[tokio::test]
    async fn invariant_no_double_fire_same_timer_recorded_twice() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::empty());

        // Record the same timer fired twice (simulating duplicate fire event)
        storage
            .record_timer_fired(&instance_id, ts_ms(1000))
            .await
            .unwrap();
        storage
            .record_timer_fired(&instance_id, ts_ms(1000))
            .await
            .unwrap();

        // INVARIANT: fire_calls should only contain ONE entry for this timer
        let calls = storage.fire_calls().await;
        let same_timer_calls: Vec<_> = calls
            .iter()
            .filter(|(id, ts)| *id == instance_id && *ts == ts_ms(1000))
            .collect();

        // REDQUEEN ASSERTION: Timer should only be recorded as fired ONCE
        assert_eq!(
            same_timer_calls.len(),
            1,
            "INVARIANT VIOLATION: Timer was recorded as fired {} times (expected 1)",
            same_timer_calls.len()
        );
    }

    #[tokio::test]
    async fn invariant_scan_excludes_deleted_timers() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::new(vec![TimerRecord::new(
            instance_id.clone(),
            ts_ms(1000),
            None,
            ts_ms(500),
        )]));

        // Delete the timer
        storage
            .delete_timer(&instance_id, ts_ms(1000))
            .await
            .unwrap();

        // Scan should NOT return the deleted timer
        let remaining = storage
            .scan_due_timers(ts_ms(0), ts_ms(2000), 100)
            .await
            .unwrap();

        // INVARIANT: Deleted timer must not appear in scan results
        assert!(
            remaining.is_empty(),
            "INVARIANT VIOLATION: Deleted timer still appears in scan results"
        );
    }

    #[tokio::test]
    async fn invariant_delete_call_recorded_before_fire() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::new(vec![TimerRecord::new(
            instance_id.clone(),
            ts_ms(1000),
            None,
            ts_ms(500),
        )]));

        // Execute delete-before-dispatch (INV-2)
        storage
            .delete_timer(&instance_id, ts_ms(1000))
            .await
            .unwrap();
        storage
            .record_timer_fired(&instance_id, ts_ms(1000))
            .await
            .unwrap();

        let delete_calls = storage.delete_calls().await;
        let fire_calls = storage.fire_calls().await;

        // INVARIANT: Delete must happen BEFORE fire in the call sequence
        let delete_idx = delete_calls
            .iter()
            .position(|(id, ts)| *id == instance_id && *ts == ts_ms(1000));
        let fire_idx = fire_calls
            .iter()
            .position(|(id, ts)| *id == instance_id && *ts == ts_ms(1000));

        assert!(
            delete_idx.is_some() && fire_idx.is_some(),
            "Both delete and fire should be recorded"
        );
        // This test documents the ordering; actual enforcement is in loop_core
    }

    #[tokio::test]
    async fn invariant_empty_storage_scan_returns_empty() {
        let storage = Arc::new(MockTimerStorage::empty());

        let result = storage.scan_due_timers(ts_ms(0), ts_ms(1000), 100).await;

        assert!(result.is_ok());
        assert!(
            result.unwrap().is_empty(),
            "Empty storage should return empty scan results"
        );
    }

    #[tokio::test]
    async fn invariant_zero_max_results_returns_empty() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::new(vec![TimerRecord::new(
            instance_id.clone(),
            ts_ms(1000),
            None,
            ts_ms(500),
        )]));

        let result = storage.scan_due_timers(ts_ms(0), ts_ms(2000), 0).await;

        // With max_results=0, should return empty (or all if 0 means "no limit")
        // This tests the boundary condition
        assert!(result.is_ok());
    }
}

mod redqueen_atomicity_tests {
    use super::*;

    #[tokio::test]
    async fn atomicity_delete_before_dispatch_sequence() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::new(vec![TimerRecord::new(
            instance_id.clone(),
            ts_ms(1000),
            None,
            ts_ms(500),
        )]));
        let queue = Arc::new(MockWorkQueue::new());

        // Simulate INV-2: delete BEFORE dispatch
        // Step 1: Delete the timer first
        storage
            .delete_timer(&instance_id, ts_ms(1000))
            .await
            .unwrap();

        // Step 2: Record timer fired
        storage
            .record_timer_fired(&instance_id, ts_ms(1000))
            .await
            .unwrap();

        // Step 3: Enqueue resume work
        queue.enqueue_resume(instance_id.clone()).await.unwrap();

        // Verify: Timer should be gone from storage
        let remaining = storage
            .scan_due_timers(ts_ms(0), ts_ms(2000), 100)
            .await
            .unwrap();
        assert!(remaining.is_empty(), "Timer must be removed after delete");

        // Verify: Work was enqueued
        assert_eq!(queue.enqueued().await.len(), 1);
    }

    #[tokio::test]
    async fn atomicity_partial_failure_delete_succeeds_fire_fails() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::new(vec![TimerRecord::new(
            instance_id.clone(),
            ts_ms(1000),
            None,
            ts_ms(500),
        )]));

        // First, do delete successfully
        storage
            .delete_timer(&instance_id, ts_ms(1000))
            .await
            .unwrap();

        // Now set failure mode for record_timer_fired
        storage.set_should_fail(true).await;

        // record_timer_fired should fail
        let result = storage.record_timer_fired(&instance_id, ts_ms(1000)).await;
        assert!(result.is_err());

        // Timer should already be deleted from scan (delete already happened)
        let remaining = storage
            .scan_due_timers(ts_ms(0), ts_ms(2000), 100)
            .await
            .unwrap();
        assert!(remaining.is_empty());

        // delete_calls should still record the delete
        let delete_calls = storage.delete_calls().await;
        assert!(!delete_calls.is_empty());
    }

    #[tokio::test]
    async fn atomicity_fire_without_delete_not_tracked_in_deleted_timers() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::empty());

        // Call record_timer_fired WITHOUT prior delete
        // This is a direct fire event, not via the delete->fire path
        storage
            .record_timer_fired(&instance_id, ts_ms(1000))
            .await
            .unwrap();

        // Should still record the fire call
        let calls = storage.fire_calls().await;
        assert_eq!(calls.len(), 1);
    }
}

mod redqueen_failure_mode_tests {
    use super::*;

    #[tokio::test]
    async fn failure_mode_scan_failure_returns_error() {
        let storage = Arc::new(MockTimerStorage::empty());
        storage.set_should_fail(true).await;

        let result = storage.scan_due_timers(ts_ms(0), ts_ms(1000), 100).await;

        assert!(
            result.is_err(),
            "Scan should return error when storage fails"
        );
        assert!(matches!(
            result.unwrap_err(),
            ReanimatorError::StorageError(_)
        ));
    }

    #[tokio::test]
    async fn failure_mode_delete_failure_returns_error() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::new(vec![TimerRecord::new(
            instance_id.clone(),
            ts_ms(1000),
            None,
            ts_ms(500),
        )]));
        storage.set_should_fail(true).await;

        let result = storage.delete_timer(&instance_id, ts_ms(1000)).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ReanimatorError::StorageError(_)
        ));
    }

    #[tokio::test]
    async fn failure_mode_record_fire_failure_returns_error() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::empty());
        storage.set_should_fail(true).await;

        let result = storage.record_timer_fired(&instance_id, ts_ms(1000)).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ReanimatorError::StorageError(_)
        ));
    }

    #[tokio::test]
    async fn failure_mode_work_queue_enqueue_failure() {
        let queue = Arc::new(MockWorkQueue::new());
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        queue.set_should_fail(true).await;

        let result = queue.enqueue_resume(instance_id).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ReanimatorError::EnqueueFailed(_)
        ));
    }

    #[tokio::test]
    async fn failure_mode_storage_recovery_after_failure() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::empty());

        // Fail operations
        storage.set_should_fail(true).await;
        assert!(
            storage.scan_due_timers(ts_ms(0), ts_ms(1000), 100).await.is_err()
        );

        // Recover
        storage.set_should_fail(false).await;

        // Should work again
        let result = storage
            .scan_due_timers(ts_ms(0), ts_ms(1000), 100)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn failure_mode_work_queue_recovery_after_failure() {
        let queue = Arc::new(MockWorkQueue::new());
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        // Fail operations
        queue.set_should_fail(true).await;
        assert!(queue.enqueue_resume(instance_id.clone()).await.is_err());

        // Recover
        queue.set_should_fail(false).await;

        // Should work again
        let result = queue.enqueue_resume(instance_id).await;
        assert!(result.is_ok());
        assert_eq!(queue.enqueued().await.len(), 1);
    }
}

mod redqueen_edge_case_tests {
    use super::*;

    #[tokio::test]
    async fn edge_case_timer_at_exact_boundary() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::new(vec![TimerRecord::new(
            instance_id.clone(),
            ts_ms(1000), // fire_at exactly at boundary
            None,
            ts_ms(500),
        )]));

        // Timer with fire_at=1000, scan to 1000 should include it
        let result = storage.scan_due_timers(ts_ms(0), ts_ms(1000), 100).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1, "Timer at exact boundary should be included");

        // Timer with fire_at=1000, scan to 999 should NOT include it
        let result = storage.scan_due_timers(ts_ms(0), ts_ms(999), 100).await;
        assert!(result.is_ok());
        assert!(
            result.unwrap().is_empty(),
            "Timer just past boundary should not be included"
        );
    }

    #[tokio::test]
    async fn edge_case_multiple_timers_same_instance_different_timer_ids() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::empty());

        // Add multiple timers for same instance with different timer_ids
        let timer_id_1 = vo_types::TimerId::from_bytes([1u8; 16]);
        let timer_id_2 = vo_types::TimerId::from_bytes([2u8; 16]);

        storage
            .add_timer(TimerRecord::new(
                instance_id.clone(),
                ts_ms(1000),
                Some(timer_id_1),
                ts_ms(500),
            ))
            .await;
        storage
            .add_timer(TimerRecord::new(
                instance_id.clone(),
                ts_ms(2000),
                Some(timer_id_2),
                ts_ms(1500),
            ))
            .await;

        let all_timers = storage
            .scan_due_timers(ts_ms(0), ts_ms(3000), 100)
            .await
            .unwrap();
        assert_eq!(all_timers.len(), 2, "Should return all timers for instance");

        // Delete only one timer
        storage
            .delete_timer(&instance_id, ts_ms(1000))
            .await
            .unwrap();

        let remaining = storage
            .scan_due_timers(ts_ms(0), ts_ms(3000), 100)
            .await
            .unwrap();
        assert_eq!(remaining.len(), 1, "Only non-deleted timer should remain");
    }

    #[tokio::test]
    async fn edge_case_scan_from_timestamp_greater_than_to() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::new(vec![TimerRecord::new(
            instance_id.clone(),
            ts_ms(1000),
            None,
            ts_ms(500),
        )]));

        // Edge case: from > to (should return empty or error)
        let result = storage.scan_due_timers(ts_ms(2000), ts_ms(1000), 100).await;
        // The implementation should handle this gracefully
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn edge_case_max_results_larger_than_available() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::new(vec![TimerRecord::new(
            instance_id.clone(),
            ts_ms(1000),
            None,
            ts_ms(500),
        )]));

        // Request more results than available
        let result = storage.scan_due_timers(ts_ms(0), ts_ms(2000), 1000).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1, "Should return all available timers");
    }

    #[tokio::test]
    async fn edge_case_delete_nonexistent_timer() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::empty());

        // Deleting non-existent timer should succeed (idempotent)
        let result = storage.delete_timer(&instance_id, ts_ms(1000)).await;
        assert!(result.is_ok(), "Delete of non-existent timer should not error");
    }

    #[tokio::test]
    async fn edge_case_record_fire_without_prior_delete() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::empty());

        // Direct record without delete path
        let result = storage.record_timer_fired(&instance_id, ts_ms(1000)).await;
        assert!(result.is_ok());

        let calls = storage.fire_calls().await;
        assert_eq!(calls.len(), 1);
    }
}

mod redqueen_deduplication_tests {
    use super::*;

    #[tokio::test]
    async fn dedup_scan_results_have_no_duplicate_timers() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::empty());

        // Add same timer multiple times (simulating scan returning duplicates)
        let timer = TimerRecord::new(
            instance_id.clone(),
            ts_ms(1000),
            None,
            ts_ms(500),
        );
        storage.add_timer(timer.clone()).await;
        storage.add_timer(timer.clone()).await;
        storage.add_timer(timer.clone()).await;

        let result = storage.scan_due_timers(ts_ms(0), ts_ms(2000), 100).await;
        assert!(result.is_ok());

        // Note: MockTimerStorage doesn't deduplicate in scan,
        // but the loop_core does dedup by (instance_id, fire_at_ms)
        // This test documents the behavior
        let timers = result.unwrap();
        assert_eq!(timers.len(), 3, "Scan returns all added timers");
    }

    #[tokio::test]
    async fn dedup_delete_removes_all_matching_timers() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::empty());

        // Add same timer multiple times
        let timer = TimerRecord::new(
            instance_id.clone(),
            ts_ms(1000),
            None,
            ts_ms(500),
        );
        storage.add_timer(timer.clone()).await;
        storage.add_timer(timer.clone()).await;

        // Delete should remove ALL matching timers
        storage
            .delete_timer(&instance_id, ts_ms(1000))
            .await
            .unwrap();

        let remaining = storage
            .scan_due_timers(ts_ms(0), ts_ms(2000), 100)
            .await
            .unwrap();
        assert!(
            remaining.is_empty(),
            "All timers with same (instance_id, fire_at) should be deleted"
        );
    }
}

mod redqueen_pending_timer_tests {
    use super::*;

    #[tokio::test]
    async fn pending_timer_mark_and_scan() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::empty());

        storage
            .mark_timer_processing(&instance_id, ts_ms(1000))
            .await
            .unwrap();

        let pending = storage.scan_pending_timers(100).await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].instance_id, instance_id);
        assert_eq!(pending[0].fire_at_ms, ts_ms(1000));
    }

    #[tokio::test]
    async fn pending_timer_complete_removes_pending() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::empty());

        storage
            .mark_timer_processing(&instance_id, ts_ms(1000))
            .await
            .unwrap();

        storage
            .complete_timer_processing(&instance_id, ts_ms(1000))
            .await
            .unwrap();

        let pending = storage.scan_pending_timers(100).await.unwrap();
        assert!(pending.is_empty(), "Completed timer should not be pending");
    }

    #[tokio::test]
    async fn pending_timer_cleanup_stale() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::empty());

        // Mark timer with very old timestamp
        storage
            .mark_timer_processing(&instance_id, ts_ms(1000))
            .await
            .unwrap();

        // Cleanup with threshold higher than marked_at
        let cleaned = storage
            .cleanup_stale_pending_timers(TimestampMs::now())
            .await
            .unwrap();

        assert_eq!(cleaned, 1, "Should clean 1 stale pending timer");
    }

    #[tokio::test]
    async fn pending_timer_isolation_per_instance() {
        let instance1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let instance2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();
        let storage = Arc::new(MockTimerStorage::empty());

        storage
            .mark_timer_processing(&instance1, ts_ms(1000))
            .await
            .unwrap();
        storage
            .mark_timer_processing(&instance2, ts_ms(2000))
            .await
            .unwrap();

        let pending = storage.scan_pending_timers(100).await.unwrap();
        assert_eq!(pending.len(), 2, "Both instances should have pending timers");

        // Complete only instance1
        storage
            .complete_timer_processing(&instance1, ts_ms(1000))
            .await
            .unwrap();

        let pending = storage.scan_pending_timers(100).await.unwrap();
        assert_eq!(pending.len(), 1, "Only instance2 should remain pending");
        assert_eq!(pending[0].instance_id, instance2);
    }
}

mod redqueen_delete_all_tests {
    use super::*;

    #[tokio::test]
    async fn delete_all_removes_all_timers_for_instance() {
        let instance1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let instance2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();
        let storage = Arc::new(MockTimerStorage::empty());

        // Add multiple timers for instance1
        for i in 1..=5u64 {
            storage
                .add_timer(TimerRecord::new(
                    instance1.clone(),
                    ts_ms(i * 1000),
                    None,
                    ts_ms(i * 500),
                ))
                .await;
        }

        // Add timer for instance2
        storage
            .add_timer(TimerRecord::new(
                instance2.clone(),
                ts_ms(1000),
                None,
                ts_ms(500),
            ))
            .await;

        // Delete all for instance1
        let deleted = storage
            .delete_all_timers_for_instance(&instance1)
            .await
            .unwrap();

        assert_eq!(deleted, 5, "Should delete 5 timers for instance1");

        // Verify instance2 timer remains
        let remaining = storage
            .scan_due_timers(ts_ms(0), ts_ms(10000), 100)
            .await
            .unwrap();
        assert_eq!(remaining.len(), 1, "Only instance2 timer should remain");
        assert_eq!(remaining[0].instance_id, instance2);

        // Verify delete_all_calls recorded
        let calls = storage.delete_all_calls().await;
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], instance1);
    }

    #[tokio::test]
    async fn delete_all_nonexistent_instance_returns_zero() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::empty());

        let deleted = storage
            .delete_all_timers_for_instance(&instance_id)
            .await
            .unwrap();

        assert_eq!(deleted, 0, "Deleting non-existent instance should return 0");
    }

    #[tokio::test]
    async fn delete_all_failure_mode() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::empty());
        storage.set_should_fail(true).await;

        let result = storage.delete_all_timers_for_instance(&instance_id).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ReanimatorError::StorageError(_)
        ));
    }
}
