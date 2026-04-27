//! Integration tests for Reanimator core modules covering:
//! - High reuse rate scenarios (single instance getting many resumes)
//! - Low reuse rate scenarios (many instances each getting few resumes)
//! - Connection churn (rapid instance creation/deletion cycles)
//! - Budget exhaustion under contention
//! - Timer record validation

use std::sync::Arc;
use vo_types::{InstanceId, TimestampMs};

use crate::reanimator::{
    mock::{MockTimerStorage, MockWorkQueue},
    traits::{PendingTimer, TimerStorage, WorkQueue},
    types::{FairnessBudget, ReanimatorConfig, TimerRecord},
    ReanimatorError,
};

fn ts_ms(value: u64) -> TimestampMs {
    TimestampMs::try_from(value).expect("valid timestamp")
}

fn make_instance_id(seed: u8) -> InstanceId {
    InstanceId::from_bytes([seed; 16])
}

fn make_timer(instance_id: InstanceId, fire_at_ms: u64) -> TimerRecord {
    TimerRecord::new(
        instance_id,
        ts_ms(fire_at_ms),
        Some(vo_types::TimerId::from_bytes([1; 16])),
        ts_ms(fire_at_ms.saturating_sub(1000)),
    )
}

// =============================================================================
// High Reuse Rate Tests
// Tests where a single instance receives many resume operations
// =============================================================================

mod high_reuse_rate_tests {
    use super::*;

    #[tokio::test]
    async fn single_instance_high_resume_count() {
        let instance_id = make_instance_id(1);
        let storage = Arc::new(MockTimerStorage::empty());
        let work_queue = Arc::new(MockWorkQueue::new());

        storage
            .add_timer(make_timer(instance_id.clone(), 5000))
            .await;

        for _ in 0..10 {
            work_queue
                .enqueue_resume(instance_id.clone())
                .await
                .expect("enqueue should succeed");
        }

        let enqueued = work_queue.enqueued().await;
        assert_eq!(enqueued.len(), 10);
        assert_eq!(enqueued[0], instance_id);
    }

    #[tokio::test]
    async fn fairness_budget_enforced_at_high_reuse() {
        let instance_id = make_instance_id(1);
        let mut budget = FairnessBudget::with_limits(3, 50);

        let mut resume_count = 0;
        while budget.can_resume(&instance_id) {
            assert!(budget.record_resume(instance_id.clone()));
            resume_count += 1;
        }

        assert_eq!(resume_count, 3, "should allow exactly 3 resumes at limit 3");
        assert!(!budget.can_resume(&instance_id));

        resume_count = 0;
        while budget.record_resume(instance_id.clone()) {
            resume_count += 1;
        }
        assert_eq!(resume_count, 0, "should not allow any more resumes");
    }

    #[tokio::test]
    async fn budget_exhaustion_blocks_subsequent_resumes() {
        let instance_id = make_instance_id(1);
        let mut budget = FairnessBudget::with_limits(2, 10);

        assert!(budget.record_resume(instance_id.clone()));
        assert!(budget.record_resume(instance_id.clone()));
        assert!(!budget.can_resume(&instance_id));

        let result = crate::reanimator::check_resume_budget(&instance_id, &budget);
        assert!(result.is_err());
        assert!(matches!(result, Err(ReanimatorError::BudgetExceeded(_))));
    }

    #[tokio::test]
    async fn high_resume_rate_with_storage_contention() {
        let instance_id = make_instance_id(1);
        let storage = Arc::new(MockTimerStorage::empty());
        let work_queue = Arc::new(MockWorkQueue::new());

        storage
            .add_timer(make_timer(instance_id.clone(), 5000))
            .await;

        storage
            .mark_timer_processing(&instance_id, ts_ms(5000))
            .await
            .expect("mark should succeed");

        for _ in 0..5 {
            work_queue
                .enqueue_resume(instance_id.clone())
                .await
                .expect("enqueue should succeed");
        }

        let enqueued = work_queue.enqueued().await;
        assert_eq!(enqueued.len(), 5);

        storage
            .complete_timer_processing(&instance_id, ts_ms(5000))
            .await
            .expect("complete should succeed");

        let pending = storage
            .scan_pending_timers(100)
            .await
            .expect("scan should succeed");
        assert!(pending.is_empty());
    }

    #[tokio::test]
    async fn max_per_instance_limit_respected_under_load() {
        let instance_id = make_instance_id(1);
        let mut budget = FairnessBudget::with_limits(1, 1);

        assert!(budget.record_resume(instance_id.clone()));
        assert!(!budget.can_resume(&instance_id));

        let result = budget.record_resume(instance_id.clone());
        assert!(!result, "second resume should be rejected");
    }
}

// =============================================================================
// Low Reuse Rate Tests
// Tests where many instances each receive few resumes
// =============================================================================

mod low_reuse_rate_tests {
    use super::*;

    #[tokio::test]
    async fn many_instances_few_resumes_each() {
        let instance_ids: Vec<InstanceId> = (0..20u8).map(make_instance_id).collect();
        let storage = Arc::new(MockTimerStorage::empty());
        let work_queue = Arc::new(MockWorkQueue::new());

        for instance_id in &instance_ids {
            storage
                .add_timer(make_timer(instance_id.clone(), 5000))
                .await;
        }

        for instance_id in &instance_ids {
            work_queue
                .enqueue_resume(instance_id.clone())
                .await
                .expect("enqueue should succeed");
        }

        let enqueued = work_queue.enqueued().await;
        assert_eq!(enqueued.len(), 20);

        for (i, instance_id) in instance_ids.iter().enumerate() {
            assert_eq!(enqueued[i], *instance_id);
        }
    }

    #[tokio::test]
    async fn fairness_budget_distributes_across_instances() {
        let instance1 = make_instance_id(1);
        let instance2 = make_instance_id(2);
        let instance3 = make_instance_id(3);

        let mut budget = FairnessBudget::with_limits(2, 10);

        assert!(budget.record_resume(instance1.clone()));
        assert!(budget.record_resume(instance2.clone()));
        assert!(budget.record_resume(instance3.clone()));

        assert!(budget.can_resume(&instance1));
        assert!(budget.can_resume(&instance2));
        assert!(budget.can_resume(&instance3));

        assert!(budget.record_resume(instance1.clone()));
        assert!(budget.record_resume(instance2.clone()));
        assert!(budget.record_resume(instance3.clone()));

        assert!(!budget.can_resume(&instance1));
        assert!(!budget.can_resume(&instance2));
        assert!(!budget.can_resume(&instance3));

        budget.reset();

        assert!(budget.can_resume(&instance1));
        assert!(budget.can_resume(&instance2));
        assert!(budget.can_resume(&instance3));
    }

    #[tokio::test]
    async fn each_instance_gets_own_budget_allocation() {
        let instance1 = make_instance_id(1);
        let instance2 = make_instance_id(2);

        let mut budget = FairnessBudget::with_limits(1, 100);

        assert!(budget.record_resume(instance1.clone()));
        assert!(!budget.can_resume(&instance1));

        assert!(budget.can_resume(&instance2));
        assert!(budget.record_resume(instance2.clone()));
        assert!(!budget.can_resume(&instance2));

        assert!(!budget.can_resume(&instance1));
    }

    #[tokio::test]
    async fn low_reuse_with_different_fire_times() {
        let instance_ids: Vec<InstanceId> = (0..5u8).map(make_instance_id).collect();
        let storage = Arc::new(MockTimerStorage::empty());
        let work_queue = Arc::new(MockWorkQueue::new());

        for (i, instance_id) in instance_ids.iter().enumerate() {
            let fire_at = 5000 + (i as u64 * 1000);
            storage
                .add_timer(make_timer(instance_id.clone(), fire_at))
                .await;
        }

        let timers = storage
            .scan_due_timers(ts_ms(0), ts_ms(10000), 100)
            .await
            .expect("scan should succeed");

        assert_eq!(timers.len(), 5);

        for timer in &timers {
            work_queue
                .enqueue_resume(timer.instance_id.clone())
                .await
                .expect("enqueue should succeed");
        }

        let enqueued = work_queue.enqueued().await;
        assert_eq!(enqueued.len(), 5);
    }

    #[tokio::test]
    async fn mixed_high_and_low_reuse_scenario() {
        let hot_instance = make_instance_id(1);
        let cold_instances: Vec<InstanceId> = (2..12u8).map(make_instance_id).collect();

        let mut budget = FairnessBudget::with_limits(5, 50);

        for _ in 0..3 {
            assert!(budget.record_resume(hot_instance.clone()));
        }

        for cold in &cold_instances {
            assert!(budget.record_resume(cold.clone()));
        }

        assert!(budget.can_resume(&hot_instance));
        assert!(budget.can_resume(&cold_instances[0]));

        for _ in 0..2 {
            assert!(budget.record_resume(hot_instance.clone()));
        }

        assert!(!budget.can_resume(&hot_instance));
        assert!(budget.can_resume(&cold_instances[0]));
    }
}

// =============================================================================
// Connection Churn Tests
// Tests with rapid instance creation/deletion cycles
// =============================================================================

mod connection_churn_tests {
    use super::*;

    #[tokio::test]
    async fn rapid_instance_creation_and_deletion() {
        let storage = Arc::new(MockTimerStorage::empty());
        let work_queue = Arc::new(MockWorkQueue::new());

        for i in 0..50u8 {
            let instance_id = make_instance_id(i);
            storage
                .add_timer(make_timer(instance_id.clone(), 5000))
                .await;

            work_queue
                .enqueue_resume(instance_id.clone())
                .await
                .expect("enqueue should succeed");

            storage
                .delete_all_timers_for_instance(&instance_id)
                .await
                .expect("delete all should succeed");
        }

        let enqueued = work_queue.enqueued().await;
        assert_eq!(enqueued.len(), 50);

        for i in 0..50u8 {
            let instance_id = make_instance_id(i);
            let remaining = storage
                .scan_due_timers(ts_ms(0), ts_ms(10000), 100)
                .await
                .expect("scan should succeed");

            let has_timers = remaining.iter().any(|t| t.instance_id == instance_id);
            assert!(!has_timers, "instance {} should have no timers", i);
        }
    }

    #[tokio::test]
    async fn instance_churn_with_pending_timer_handling() {
        let storage = Arc::new(MockTimerStorage::empty());
        let work_queue = Arc::new(MockWorkQueue::new());

        for i in 0..10u8 {
            let instance_id = make_instance_id(i);

            storage
                .add_timer(make_timer(instance_id.clone(), 5000))
                .await;

            storage
                .mark_timer_processing(&instance_id, ts_ms(5000))
                .await
                .expect("mark should succeed");

            let pending = storage
                .scan_pending_timers(100)
                .await
                .expect("scan should succeed");

            assert!(!pending.is_empty());

            if i % 2 == 0 {
                work_queue
                    .enqueue_resume(instance_id.clone())
                    .await
                    .expect("enqueue should succeed");

                storage
                    .complete_timer_processing(&instance_id, ts_ms(5000))
                    .await
                    .expect("complete should succeed");
            }
        }

        let enqueued = work_queue.enqueued().await;
        assert_eq!(enqueued.len(), 5);
    }

    #[tokio::test]
    async fn churn_with_storage_failure_recovery() {
        let storage = Arc::new(MockTimerStorage::empty());

        for i in 0..5u8 {
            let instance_id = make_instance_id(i);
            storage
                .add_timer(make_timer(instance_id.clone(), 5000 + i * 100))
                .await;
        }

        storage.set_should_fail(true).await;

        let result = storage.scan_due_timers(ts_ms(0), ts_ms(10000), 100).await;
        assert!(result.is_err());

        storage.set_should_fail(false).await;

        let result = storage.scan_due_timers(ts_ms(0), ts_ms(10000), 100).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 5);
    }

    #[tokio::test]
    async fn high_churn_delete_all_timers_for_instance() {
        let storage = Arc::new(MockTimerStorage::empty());

        for i in 0..20u8 {
            let instance_id = make_instance_id(i);
            for fire_at in [5000u64, 6000, 7000] {
                storage
                    .add_timer(make_timer(instance_id.clone(), fire_at))
                    .await;
            }
        }

        for i in 0..20u8 {
            let instance_id = make_instance_id(i);
            let deleted = storage
                .delete_all_timers_for_instance(&instance_id)
                .await
                .expect("delete should succeed");

            assert_eq!(deleted, 3, "should delete 3 timers for instance {}", i);
        }

        let remaining = storage
            .scan_due_timers(ts_ms(0), ts_ms(10000), 1000)
            .await
            .expect("scan should succeed");

        assert!(remaining.is_empty(), "all timers should be deleted");
    }
}

// =============================================================================
// Budget Exhaustion Tests
// =============================================================================

mod budget_exhaustion_tests {
    use super::*;

    #[tokio::test]
    async fn budget_exhaustion_prevents_resumes() {
        let instance_id = make_instance_id(1);
        let mut budget = FairnessBudget::with_limits(2, 10);

        assert!(budget.record_resume(instance_id.clone()));
        assert!(budget.record_resume(instance_id.clone()));
        assert!(!budget.record_resume(instance_id.clone()));
        assert!(!budget.can_resume(&instance_id));
    }

    #[tokio::test]
    async fn zero_max_per_instance_blocks_all() {
        let instance_id = make_instance_id(1);
        let budget = FairnessBudget::with_limits(0, 0);

        assert!(!budget.can_resume(&instance_id));

        let result = budget.record_resume(instance_id.clone());
        assert!(!result);
    }

    #[tokio::test]
    async fn budget_reset_allows_new_resumes() {
        let instance_id = make_instance_id(1);
        let mut budget = FairnessBudget::with_limits(1, 10);

        assert!(budget.record_resume(instance_id.clone()));
        assert!(!budget.can_resume(&instance_id));

        budget.reset();

        assert!(budget.can_resume(&instance_id));
        assert!(budget.record_resume(instance_id.clone()));
    }

    #[tokio::test]
    async fn budget_exhaustion_error_message_contains_instance() {
        let instance_id = make_instance_id(1);
        let mut budget = FairnessBudget::with_limits(1, 10);

        budget.record_resume(instance_id.clone()).unwrap();

        let result = crate::reanimator::check_resume_budget(&instance_id, &budget);
        assert!(result.is_err());

        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("has exceeded resume budget"));
    }
}

// =============================================================================
// Fairness Under Contention Tests
// =============================================================================

mod fairness_contention_tests {
    use super::*;

    #[tokio::test]
    async fn fairness_prevents_single_instance_starvation() {
        let greedy = make_instance_id(1);
        let victims: Vec<InstanceId> = (2..11u8).map(make_instance_id).collect();

        let mut budget = FairnessBudget::with_limits(1, 10);

        budget.record_resume(greedy.clone()).unwrap();

        for victim in &victims {
            assert!(
                budget.can_resume(victim),
                "victim should still be resumable"
            );
            budget.record_resume(victim.clone()).unwrap();
        }

        assert!(
            !budget.can_resume(&greedy),
            "greedy instance should be blocked"
        );
    }

    #[tokio::test]
    async fn contention_stress_test_many_instances() {
        let instances: Vec<InstanceId> = (0..100u8).map(make_instance_id).collect();
        let mut budget = FairnessBudget::with_limits(1, 100);

        for instance_id in &instances {
            assert!(
                budget.can_resume(instance_id),
                "instance should be resumable"
            );
            budget.record_resume(instance_id.clone()).unwrap();
        }

        for instance_id in &instances {
            assert!(
                !budget.can_resume(instance_id),
                "all instances should be exhausted"
            );
        }
    }

    #[tokio::test]
    async fn fairness_with_cyclical_access_pattern() {
        let instances: Vec<InstanceId> = (0..5u8).map(make_instance_id).collect();
        let mut budget = FairnessBudget::with_limits(2, 10);

        for _ in 0..3 {
            for instance_id in &instances {
                budget.record_resume(instance_id.clone()).unwrap();
            }
        }

        for instance_id in &instances {
            assert!(
                !budget.can_resume(instance_id),
                "instance should be exhausted after 3 cycles of 2 each"
            );
        }

        budget.reset();

        for instance_id in &instances {
            assert!(budget.can_resume(instance_id));
        }
    }

    #[tokio::test]
    async fn mixed_workload_high_and_low_priority_instances() {
        let high_priority = make_instance_id(1);
        let low_priority: Vec<InstanceId> = (2..6u8).map(make_instance_id).collect();

        let mut budget = FairnessBudget::with_limits(3, 15);

        for _ in 0..3 {
            budget.record_resume(high_priority.clone()).unwrap();
        }

        for lp in &low_priority {
            budget.record_resume(lp.clone()).unwrap();
        }

        assert!(!budget.can_resume(&high_priority));
        assert!(budget.can_resume(&low_priority[0]));

        budget.reset();

        assert!(budget.can_resume(&high_priority));
        for lp in &low_priority {
            assert!(budget.can_resume(lp));
        }
    }
}

// =============================================================================
// Timer Record Validation Tests
// =============================================================================

mod timer_validation_tests {
    use super::*;

    #[tokio::test]
    async fn validate_timer_fire_at_zero_is_invalid() {
        let timer = TimerRecord::new(
            make_instance_id(1),
            ts_ms(0),
            None,
            ts_ms(1000),
        );

        let result = crate::reanimator::validate_timer_record(&timer);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn validate_timer_scheduled_at_zero_is_invalid() {
        let timer = TimerRecord::new(
            make_instance_id(1),
            ts_ms(1000),
            None,
            ts_ms(0),
        );

        let result = crate::reanimator::validate_timer_record(&timer);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn validate_timer_fire_before_scheduled_is_invalid() {
        let timer = TimerRecord::new(
            make_instance_id(1),
            ts_ms(500),
            None,
            ts_ms(1000),
        );

        let result = crate::reanimator::validate_timer_record(&timer);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn validate_timer_zero_instance_id_is_invalid() {
        let timer = TimerRecord::new(
            InstanceId::from_bytes([0u8; 16]),
            ts_ms(1000),
            None,
            ts_ms(500),
        );

        let result = crate::reanimator::validate_timer_record(&timer);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn validate_timer_valid_record_passes() {
        let timer = TimerRecord::new(
            make_instance_id(1),
            ts_ms(1000),
            Some(vo_types::TimerId::from_bytes([1; 16])),
            ts_ms(500),
        );

        let result = crate::reanimator::validate_timer_record(&timer);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn validate_timer_null_timer_id_is_valid() {
        let timer = TimerRecord::new(
            make_instance_id(1),
            ts_ms(1000),
            None,
            ts_ms(500),
        );

        let result = crate::reanimator::validate_timer_record(&timer);
        assert!(result.is_ok());
    }
}

// =============================================================================
// Crash Recovery Integration Tests
// =============================================================================

mod crash_recovery_integration_tests {
    use super::*;

    #[tokio::test]
    async fn full_crash_recovery_cycle() {
        let instance_id = make_instance_id(1);
        let storage = Arc::new(MockTimerStorage::empty());
        let work_queue = Arc::new(MockWorkQueue::new());

        storage
            .add_timer(make_timer(instance_id.clone(), 5000))
            .await;

        storage
            .mark_timer_processing(&instance_id, ts_ms(5000))
            .await
            .expect("mark should succeed");

        let pending = storage
            .scan_pending_timers(100)
            .await
            .expect("scan should succeed");

        assert_eq!(pending.len(), 1);

        work_queue
            .enqueue_resume(instance_id.clone())
            .await
            .expect("enqueue should succeed");

        storage
            .complete_timer_processing(&instance_id, ts_ms(5000))
            .await
            .expect("complete should succeed");

        let remaining = storage
            .scan_pending_timers(100)
            .await
            .expect("scan should succeed");

        assert!(remaining.is_empty());

        let enqueued = work_queue.enqueued().await;
        assert_eq!(enqueued.len(), 1);
        assert_eq!(enqueued[0], instance_id);
    }

    #[tokio::test]
    async fn recovery_with_budget_enforcement() {
        let instance1 = make_instance_id(1);
        let instance2 = make_instance_id(2);
        let storage = Arc::new(MockTimerStorage::empty());
        let work_queue = Arc::new(MockWorkQueue::new());

        for instance_id in [&instance1, &instance2] {
            storage
                .mark_timer_processing(instance_id, ts_ms(5000))
                .await
                .expect("mark should succeed");
        }

        let pending = storage
            .scan_pending_timers(100)
            .await
            .expect("scan should succeed");

        assert_eq!(pending.len(), 2);

        let mut budget = FairnessBudget::with_limits(1, 10);

        for p in &pending {
            if budget.can_resume(&p.instance_id) {
                budget.record_resume(p.instance_id.clone()).unwrap();
                work_queue
                    .enqueue_resume(p.instance_id.clone())
                    .await
                    .expect("enqueue should succeed");
            }
        }

        let enqueued = work_queue.enqueued().await;
        assert_eq!(enqueued.len(), 1);

        assert!(!budget.can_resume(&instance1));
    }

    #[tokio::test]
    async fn stale_timer_cleanup_during_recovery() {
        let storage = Arc::new(MockTimerStorage::empty());

        let stale = PendingTimer {
            instance_id: make_instance_id(1),
            fire_at_ms: ts_ms(5000),
            scheduled_at_ms: ts_ms(4000),
            marked_at_ms: ts_ms(100),
        };

        storage.add_pending_timer(stale).await;

        let cleaned = storage
            .cleanup_stale_pending_timers(ts_ms(1000))
            .await
            .expect("cleanup should succeed");

        assert_eq!(cleaned, 1);

        let remaining = storage
            .scan_pending_timers(100)
            .await
            .expect("scan should succeed");

        assert!(remaining.is_empty());
    }
}

// =============================================================================
// Mixed Workload Stress Tests
// =============================================================================

mod stress_tests {
    use super::*;

    #[tokio::test]
    async fn mixed_reuse_rates_stress() {
        let hot_instance = make_instance_id(1);
        let warm_instances: Vec<InstanceId> = (2..6u8).map(make_instance_id).collect();
        let cold_instances: Vec<InstanceId> = (6..21u8).map(make_instance_id).collect();

        let mut budget = FairnessBudget::with_limits(10, 100);

        for _ in 0..5 {
            budget.record_resume(hot_instance.clone()).unwrap();
        }

        for warm in &warm_instances {
            for _ in 0..3 {
                budget.record_resume(warm.clone()).unwrap();
            }
        }

        for cold in &cold_instances {
            budget.record_resume(cold.clone()).unwrap();
        }

        assert!(!budget.can_resume(&hot_instance));

        for warm in &warm_instances {
            assert!(!budget.can_resume(warm));
        }

        for cold in &cold_instances {
            assert!(!budget.can_resume(cold));
        }
    }

    #[tokio::test]
    async fn churn_with_validation_failures() {
        let storage = Arc::new(MockTimerStorage::empty());
        let work_queue = Arc::new(MockWorkQueue::new());

        for i in 0..10u8 {
            let instance_id = make_instance_id(i);
            storage
                .add_timer(make_timer(instance_id.clone(), 5000))
                .await;

            let valid = crate::reanimator::validate_timer_record(
                &make_timer(instance_id.clone(), 5000),
            )
            .is_ok();

            if valid {
                work_queue
                    .enqueue_resume(instance_id.clone())
                    .await
                    .expect("enqueue should succeed");
            }
        }

        let enqueued = work_queue.enqueued().await;
        assert_eq!(enqueued.len(), 10);
    }

    #[tokio::test]
    async fn config_limits_enforced_integration() {
        let config = ReanimatorConfig {
            scan_interval: std::time::Duration::from_millis(100),
            max_timers_per_cycle: 50,
            max_concurrent_resumes: 10,
            shutdown_timeout: std::time::Duration::from_secs(5),
        };

        let mut budget = FairnessBudget::with_limits(
            config.max_timers_per_cycle,
            config.max_timers_per_cycle * config.max_concurrent_resumes,
        );

        let instance_ids: Vec<InstanceId> = (0..30u8).map(make_instance_id).collect();

        for instance_id in &instance_ids {
            budget.record_resume(instance_id.clone()).unwrap();
        }

        for instance_id in &instance_ids {
            assert!(!budget.can_resume(instance_id));
        }

        assert_eq!(budget.max_per_instance, 50);
        assert_eq!(budget.max_per_workflow, 500);
    }
}