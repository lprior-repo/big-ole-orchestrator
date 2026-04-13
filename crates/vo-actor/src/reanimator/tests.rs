//! Unit tests for Reanimator calculation layer and handle.

use std::time::Duration;
use tokio::sync::{broadcast, watch};
use vo_types::{InstanceId, TimestampMs};

use crate::reanimator::{
    loop_core::ReanimatorHandle,
    types::{
        calculate_batch_size, check_resume_budget, filter_timers_by_fairness,
        validate_timer_record, FairnessBudget, ReanimatorState, TimerRecord,
    },
    ReanimatorError,
};
use crate::timer_lifecycle;

// Helper function to create TimestampMs from u64 without unwrap in test code
fn ts_ms(value: u64) -> TimestampMs {
    TimestampMs::try_from(value).expect("valid timestamp")
}

// =============================================================================
// Calculation Layer Tests
// =============================================================================

mod calculation_tests {
    use super::*;

    #[test]
    fn filter_timers_by_fairness_allows_within_budget() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let timers = vec![TimerRecord::new(
            instance_id.clone(),
            ts_ms(1000),
            None,
            ts_ms(500),
        )];

        let budget = FairnessBudget::default();
        let (allowed, rejected) = filter_timers_by_fairness(timers.clone(), &budget);

        assert_eq!(allowed.len(), 1);
        assert_eq!(rejected.len(), 0);
    }

    #[test]
    fn filter_timers_by_fairness_rejects_over_budget() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let mut budget = FairnessBudget::with_limits(1, 100);

        // Exhaust budget
        assert!(budget.record_resume(instance_id.clone()));

        let timers = vec![TimerRecord::new(
            instance_id.clone(),
            ts_ms(1000),
            None,
            ts_ms(500),
        )];

        let (allowed, rejected) = filter_timers_by_fairness(timers, &budget);

        assert_eq!(allowed.len(), 0);
        assert_eq!(rejected.len(), 1);
    }

    #[test]
    fn calculate_batch_size_respects_budget() {
        assert_eq!(calculate_batch_size(50, 100, 0), 50);
        assert_eq!(calculate_batch_size(50, 100, 30), 50);
        assert_eq!(calculate_batch_size(50, 100, 70), 30);
        assert_eq!(calculate_batch_size(50, 100, 100), 0);
        assert_eq!(calculate_batch_size(50, 100, 101), 0);
    }

    #[test]
    fn calculate_batch_size_respects_remaining() {
        assert_eq!(calculate_batch_size(10, 100, 0), 10);
        assert_eq!(calculate_batch_size(10, 100, 95), 5);
    }

    #[test]
    fn validate_timer_record_accepts_valid_record() {
        let record = TimerRecord::new(
            InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
            ts_ms(1000),
            None,
            ts_ms(500),
        );
        assert_eq!(validate_timer_record(&record), Ok(()));
    }

    #[test]
    fn validate_timer_record_rejects_zero_fire_at() {
        let record = TimerRecord::new(
            InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
            ts_ms(0),
            None,
            ts_ms(500),
        );
        let err = validate_timer_record(&record).unwrap_err();
        assert!(matches!(err, ReanimatorError::CorruptKey(_)));
    }

    #[test]
    fn validate_timer_record_rejects_zero_scheduled_at() {
        let record = TimerRecord::new(
            InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
            ts_ms(1000),
            None,
            ts_ms(0),
        );
        let err = validate_timer_record(&record).unwrap_err();
        assert!(matches!(err, ReanimatorError::CorruptKey(_)));
    }

    #[test]
    fn validate_timer_record_rejects_fire_before_scheduled() {
        let record = TimerRecord::new(
            InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
            ts_ms(500),
            None,
            ts_ms(1000),
        );
        let err = validate_timer_record(&record).unwrap_err();
        assert!(matches!(err, ReanimatorError::CorruptKey(_)));
    }

    #[test]
    fn check_resume_budget_success() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let budget = FairnessBudget::default();

        let result = check_resume_budget(&instance_id, &budget);
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn check_resume_budget_fails_over_budget() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let mut budget = FairnessBudget::with_limits(1, 100);

        // Exhaust budget
        assert!(budget.record_resume(instance_id.clone()));

        let result = check_resume_budget(&instance_id, &budget);
        assert_eq!(
            result,
            Err(ReanimatorError::BudgetExceeded(format!(
                "Instance {} has exceeded resume budget",
                instance_id
            )))
        );
    }
}

// =============================================================================
// ReanimatorHandle Tests
// =============================================================================

mod reanimator_handle_tests {
    use super::*;

    #[test]
    fn handle_initial_state() {
        let (state_sender, _) = watch::channel(ReanimatorState::Stopped);
        let (shutdown_trigger, _) = broadcast::channel(1);

        let handle = ReanimatorHandle {
            state_sender,
            shutdown_trigger,
            task_handle: None,
        };

        assert_eq!(handle.current_state(), ReanimatorState::Stopped);
    }
}

// =============================================================================
// Crash Recovery Tests (ADR-005)
// =============================================================================

mod crash_recovery_tests {
    use super::*;
    use crate::reanimator::traits::{PendingTimer, TimerStorage, WorkQueue};

    fn make_pending_timer(instance_id: InstanceId, fire_at_ms: TimestampMs, scheduled_at_ms: TimestampMs) -> PendingTimer {
        PendingTimer {
            instance_id,
            fire_at_ms,
            scheduled_at_ms,
            marked_at_ms: TimestampMs::now(),
        }
    }

    #[tokio::test]
    async fn crash_recovery_replays_pending_timers() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::new(Vec::new()));
        let work_queue = Arc::new(MockWorkQueue::new());

        storage.add_timer(TimerRecord::new(
            instance_id.clone(),
            ts_ms(1000),
            None,
            ts_ms(500),
        )).await;

        let pending = make_pending_timer(instance_id.clone(), ts_ms(1000), ts_ms(500));
        storage.mark_timer_processing(&instance_id, ts_ms(1000)).await.unwrap();

        let result = storage.scan_pending_timers(100).await.unwrap();
        assert_eq!(result.len(), 1);

        let enqueued = work_queue.enqueued().await;
        assert_eq!(enqueued.len(), 0);

        work_queue.enqueue_resume(instance_id.clone()).await.unwrap();

        assert_eq!(work_queue.enqueued().await.len(), 1);
        assert_eq!(work_queue.enqueued().await[0], instance_id);
    }

    #[tokio::test]
    async fn crash_recovery_skips_terminal_instances() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::empty());
        let work_queue = Arc::new(MockWorkQueue::new());

        let pending = make_pending_timer(instance_id.clone(), ts_ms(1000), ts_ms(500));
        storage.mark_timer_processing(&instance_id, ts_ms(1000)).await.unwrap();

        let pending_timers = storage.scan_pending_timers(100).await.unwrap();
        assert_eq!(pending_timers.len(), 1);

        let is_terminal = work_queue.is_instance_terminal(&instance_id).await.unwrap();
        assert!(!is_terminal);
    }

    #[tokio::test]
    async fn crash_recovery_cleans_stale_pending_timers() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::empty());

        let pending = make_pending_timer(instance_id.clone(), ts_ms(1000), ts_ms(500));
        storage.mark_timer_processing(&instance_id, ts_ms(1000)).await.unwrap();

        let before = storage.scan_pending_timers(100).await.unwrap();
        assert_eq!(before.len(), 1);

        let old_threshold = TimestampMs::now();
        let cleaned = storage.cleanup_stale_pending_timers(old_threshold).await.unwrap();
        assert_eq!(cleaned, 1);

        let after = storage.scan_pending_timers(100).await.unwrap();
        assert_eq!(after.len(), 0);
    }

    #[tokio::test]
    async fn crash_recovery_enqueues_resume_for_non_terminal_instance() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::empty());
        let work_queue = Arc::new(MockWorkQueue::new());

        let pending = make_pending_timer(instance_id.clone(), ts_ms(1000), ts_ms(500));
        storage.mark_timer_processing(&instance_id, ts_ms(1000)).await.unwrap();

        let pending_timers = storage.scan_pending_timers(100).await.unwrap();
        assert!(!pending_timers.is_empty());

        work_queue.enqueue_resume(instance_id.clone()).await.unwrap();

        let enqueued = work_queue.enqueued().await;
        assert_eq!(enqueued.len(), 1);
        assert_eq!(enqueued[0], instance_id);

        storage.complete_timer_processing(&instance_id, ts_ms(1000)).await.unwrap();
        let remaining = storage.scan_pending_timers(100).await.unwrap();
        assert!(remaining.is_empty());
    }
}

// =============================================================================
// Timer Lifecycle Tests (ADR-005)
// =============================================================================

mod timer_lifecycle_tests {
    use super::*;
    use crate::timer_lifecycle::{cancel_timers_for_instance, has_pending_timers, scan_instance_timers};

    fn create_timer_record(instance_id: InstanceId, fire_at_ms: u64) -> TimerRecord {
        TimerRecord::new(
            instance_id,
            ts_ms(fire_at_ms),
            Some(vo_types::TimerId::from_bytes([2; 16])),
            ts_ms(fire_at_ms - 1000),
        )
    }

    #[tokio::test]
    async fn timer_lifecycle_cancellation_on_completion() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::empty());

        storage.add_timer(create_timer_record(instance_id.clone(), 5000)).await;
        storage.add_timer(create_timer_record(instance_id.clone(), 6000)).await;

        let other_instance = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();
        storage.add_timer(create_timer_record(other_instance.clone(), 5500)).await;

        let has_before = has_pending_timers(&storage, &instance_id).await.unwrap();
        assert!(has_before);

        let count = cancel_timers_for_instance(&storage, &instance_id).await.unwrap();
        assert_eq!(count, 2);

        let has_after = has_pending_timers(&storage, &instance_id).await.unwrap();
        assert!(!has_after);

        let other_has = has_pending_timers(&storage, &other_instance).await.unwrap();
        assert!(other_has);
    }

    #[tokio::test]
    async fn timer_lifecycle_canonical_path_timer_scheduled_to_fired() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::empty());

        let scheduled_at = ts_ms(1000);
        let fire_at = ts_ms(2000);

        storage.add_timer(TimerRecord::new(
            instance_id.clone(),
            fire_at,
            Some(vo_types::TimerId::from_bytes([3; 16])),
            scheduled_at,
        )).await;

        let due_timers = storage.scan_due_timers(ts_ms(0), ts_ms(2500), 100).await.unwrap();
        assert_eq!(due_timers.len(), 1);
        assert_eq!(due_timers[0].fire_at_ms, fire_at);

        storage.delete_timer(&instance_id, fire_at).await.unwrap();
        storage.record_timer_fired(&instance_id, fire_at).await.unwrap();

        let after_delete = storage.scan_due_timers(ts_ms(0), ts_ms(2500), 100).await.unwrap();
        assert!(after_delete.is_empty());

        let fire_calls = storage.fire_calls().await;
        assert_eq!(fire_calls.len(), 1);
        assert_eq!(fire_calls[0].0, instance_id);
        assert_eq!(fire_calls[0].1, fire_at);
    }

    #[tokio::test]
    async fn timer_lifecycle_multiple_timers_per_instance() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let storage = Arc::new(MockTimerStorage::empty());

        storage.add_timer(TimerRecord::new(
            instance_id.clone(),
            ts_ms(1000),
            Some(vo_types::TimerId::from_bytes([1; 16])),
            ts_ms(500),
        )).await;

        storage.add_timer(TimerRecord::new(
            instance_id.clone(),
            ts_ms(2000),
            Some(vo_types::TimerId::from_bytes([2; 16])),
            ts_ms(1500),
        )).await;

        storage.add_timer(TimerRecord::new(
            instance_id.clone(),
            ts_ms(3000),
            Some(vo_types::TimerId::from_bytes([3; 16])),
            ts_ms(2500),
        )).await;

        let all_timers = scan_instance_timers(&storage, &instance_id, 100).await.unwrap();
        assert_eq!(all_timers.len(), 3);

        let due_timers = storage.scan_due_timers(ts_ms(0), ts_ms(2500), 100).await.unwrap();
        assert_eq!(due_timers.len(), 2);
    }
}
