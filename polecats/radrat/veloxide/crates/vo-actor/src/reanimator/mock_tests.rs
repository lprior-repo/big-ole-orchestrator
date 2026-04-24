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
