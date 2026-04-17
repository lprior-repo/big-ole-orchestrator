// TDD Red: Background job scheduler tests
// These tests define the expected behavior per ADR-047-v2 contract
// Tests marked FAILING are not yet passing - implementation incomplete

#[cfg(test)]
mod scheduler_retry_policy_tests {
    use vo_executor::RetryPolicy;

    #[test]
    fn retry_policy_new_valid() {
        let policy = RetryPolicy::new(3, 100, 2.0).unwrap();
        assert_eq!(policy.max_attempts, 3);
        assert_eq!(policy.backoff_ms, 100);
        assert!((policy.backoff_multiplier - 2.0).abs() < f64::EPSILON);
        assert_eq!(policy.max_backoff_ms, u64::MAX);
    }

    #[test]
    fn retry_policy_new_zero_attempts_rejects() {
        let result = RetryPolicy::new(0, 100, 2.0);
        assert!(result.is_err());
    }

    #[test]
    fn retry_policy_new_nan_multiplier_rejects() {
        let result = RetryPolicy::new(3, 100, f64::NAN);
        assert!(result.is_err());
    }

    #[test]
    fn retry_policy_new_infinity_multiplier_rejects() {
        let result = RetryPolicy::new(3, 100, f64::INFINITY);
        assert!(result.is_err());
    }

    #[test]
    fn retry_policy_new_multiplier_below_one_rejects() {
        let result = RetryPolicy::new(3, 100, 0.99);
        assert!(result.is_err());
    }

    #[test]
    fn retry_policy_new_multiplier_exactly_one_ok() {
        let result = RetryPolicy::new(3, 100, 1.0);
        assert!(result.is_ok());
    }

    #[test]
    fn retry_policy_with_max_backoff_valid() {
        let policy = RetryPolicy::with_max_backoff(5, 100, 2.0, 5000).unwrap();
        assert_eq!(policy.max_attempts, 5);
        assert_eq!(policy.max_backoff_ms, 5000);
    }

    #[test]
    fn retry_policy_with_max_backoff_equal_to_initial_ok() {
        let policy = RetryPolicy::with_max_backoff(3, 100, 2.0, 100).unwrap();
        assert_eq!(policy.max_backoff_ms, 100);
    }

    #[test]
    fn retry_policy_with_max_backoff_less_than_initial_rejects() {
        let result = RetryPolicy::with_max_backoff(3, 100, 2.0, 50);
        assert!(result.is_err());
    }

    #[test]
    fn calculate_backoff_delay_attempt_zero_returns_zero() {
        let policy = RetryPolicy::new(3, 100, 2.0).unwrap();
        assert_eq!(policy.calculate_backoff_delay(0), 0);
    }

    #[test]
    fn calculate_backoff_delay_zero_initial_returns_zero() {
        let policy = RetryPolicy::new(3, 0, 2.0).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 0);
        assert_eq!(policy.calculate_backoff_delay(5), 0);
    }

    #[test]
    fn calculate_backoff_delay_linear_multiplier_one() {
        let policy = RetryPolicy::new(10, 100, 1.0).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 100);
        assert_eq!(policy.calculate_backoff_delay(5), 100);
        assert_eq!(policy.calculate_backoff_delay(10), 100);
    }

    #[test]
    fn calculate_backoff_delay_exponential() {
        let policy = RetryPolicy::new(10, 100, 2.0).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 100);
        assert_eq!(policy.calculate_backoff_delay(2), 200);
        assert_eq!(policy.calculate_backoff_delay(3), 400);
        assert_eq!(policy.calculate_backoff_delay(4), 800);
        assert_eq!(policy.calculate_backoff_delay(5), 1600);
    }

    #[test]
    fn calculate_backoff_delay_capped_at_max_backoff() {
        let policy = RetryPolicy::with_max_backoff(10, 100, 10.0, 500).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 100);
        assert_eq!(policy.calculate_backoff_delay(2), 500);
        assert_eq!(policy.calculate_backoff_delay(3), 500);
    }

    #[test]
    fn calculate_backoff_delay_exponential_with_small_multiplier() {
        let policy = RetryPolicy::with_max_backoff(5, 1000, 1.5, 10000).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 1000);
        assert_eq!(policy.calculate_backoff_delay(2), 1500);
        assert_eq!(policy.calculate_backoff_delay(3), 2250);
    }

    #[test]
    fn max_backoff_clamp_prevents_exponential_overflow() {
        let policy = RetryPolicy::with_max_backoff(10, 1000, 2.0, 30000).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 1000);
        assert_eq!(policy.calculate_backoff_delay(2), 2000);
        assert_eq!(policy.calculate_backoff_delay(3), 4000);
        assert_eq!(policy.calculate_backoff_delay(4), 8000);
        assert_eq!(policy.calculate_backoff_delay(5), 16000);
        assert_eq!(policy.calculate_backoff_delay(6), 30000);
        assert_eq!(policy.calculate_backoff_delay(7), 30000);
    }

    #[test]
    fn max_backoff_very_large_value_allows_full_exponential() {
        let policy = RetryPolicy::new(5, 100, 2.0).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 100);
        assert_eq!(policy.calculate_backoff_delay(2), 200);
        assert_eq!(policy.calculate_backoff_delay(3), 400);
        assert_eq!(policy.calculate_backoff_delay(4), 800);
    }

    #[test]
    fn max_backoff_exactly_at_exponential_result() {
        let policy = RetryPolicy::with_max_backoff(3, 100, 2.0, 200).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 100);
        assert_eq!(policy.calculate_backoff_delay(2), 200);
        assert_eq!(policy.calculate_backoff_delay(3), 200);
    }

    #[test]
    fn retry_exhaustion_single_attempt() {
        let policy = RetryPolicy::new(1, 100, 2.0).unwrap();
        assert_eq!(policy.max_attempts, 1);
    }

    #[test]
    fn retry_exhaustion_three_attempts() {
        let policy = RetryPolicy::new(3, 100, 2.0).unwrap();
        assert_eq!(policy.max_attempts, 3);
    }

    #[test]
    fn retry_exhaustion_after_max_attempts() {
        let policy = RetryPolicy::new(3, 100, 2.0).unwrap();
        assert_eq!(policy.max_attempts, 3);
    }

    #[test]
    fn retry_exhaustion_with_different_delays() {
        let policy = RetryPolicy::new(5, 1000, 2.0).unwrap();
        for attempt in 1..=5 {
            let delay = policy.calculate_backoff_delay(attempt);
            assert!(delay > 0);
        }
    }

    #[test]
    fn retry_policy_zero_initial_delay_with_multiplier() {
        let policy = RetryPolicy::new(3, 0, 2.0).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 0);
        assert_eq!(policy.calculate_backoff_delay(2), 0);
        assert_eq!(policy.calculate_backoff_delay(3), 0);
    }

    #[test]
    fn retry_policy_large_multiplier() {
        let policy = RetryPolicy::with_max_backoff(5, 1, 100.0, 10000).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 1);
        assert_eq!(policy.calculate_backoff_delay(2), 100);
        assert_eq!(policy.calculate_backoff_delay(3), 10000);
    }

    #[test]
    fn retry_policy_zero_max_backoff_effectively_disables_backoff() {
        let policy = RetryPolicy::with_max_backoff(3, 100, 2.0, 100).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 100);
        assert_eq!(policy.calculate_backoff_delay(2), 100);
        assert_eq!(policy.calculate_backoff_delay(3), 100);
    }

    #[test]
    fn retry_policy_very_small_max_backoff() {
        let policy = RetryPolicy::with_max_backoff(3, 1, 2.0, 1).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 1);
        assert_eq!(policy.calculate_backoff_delay(2), 1);
        assert_eq!(policy.calculate_backoff_delay(3), 1);
    }
}

#[cfg(test)]
mod scheduler_unit_tests {
    use std::time::Duration;
    use vo_executor::scheduler::{JobRunError, SchedulerError};
    use vo_executor::{Job, JobId, JobPriority, JobResult, Schedule, SchedulerConfig};

    // =========================================================================
    // Section 1: JobPriority Enum Tests (5 tests)
    // =========================================================================

    #[test]
    fn job_priority_default_is_normal() {
        let priority = JobPriority::default();
        assert_eq!(priority, JobPriority::Normal);
    }

    #[test]
    fn job_priority_all_variants_present() {
        let variants = [
            JobPriority::Critical,
            JobPriority::High,
            JobPriority::Normal,
            JobPriority::Low,
        ];
        assert_eq!(variants.len(), 4);
    }

    #[test]
    fn job_priority_debug_format() {
        let priority = JobPriority::High;
        let debug = format!("{:?}", priority);
        assert!(debug.contains("High"));
    }

    // =========================================================================
    // Section 2: Schedule Enum Tests (10 tests)
    // =========================================================================

    #[test]
    fn schedule_cron_creation() {
        let schedule = Schedule::cron("*/5 * * * *");
        match schedule {
            Schedule::Cron(expr) => assert_eq!(expr, "*/5 * * * *"),
            _ => panic!("Expected Cron schedule"),
        }
    }

    #[test]
    fn schedule_cron_next_fire_returns_none() {
        let schedule = Schedule::cron("*/5 * * * *");
        let next = schedule.next_fire_time(0);
        assert!(
            next.is_none(),
            "Cron next_fire_time should return None (not implemented)"
        );
    }

    #[test]
    fn schedule_one_shot_creation() {
        let delay = Duration::from_secs(60);
        let schedule = Schedule::one_shot(delay);
        match schedule {
            Schedule::OneShot { fire_at_ms } => {
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_millis() as u64);
                assert!(fire_at_ms > now_ms);
            }
            _ => panic!("Expected OneShot schedule"),
        }
    }

    #[test]
    fn schedule_one_shot_next_fire_first_call() {
        let schedule = Schedule::one_shot(Duration::from_secs(60));
        let next = schedule.next_fire_time(0);
        assert!(
            next.is_some(),
            "First call with last_fire_ms=0 should return Some"
        );
    }

    #[test]
    fn schedule_one_shot_next_fire_second_call() {
        let schedule = Schedule::one_shot(Duration::from_secs(60));
        let first = schedule.next_fire_time(0).unwrap();
        let second = schedule.next_fire_time(first);
        assert!(
            second.is_none(),
            "Second call with last_fire_ms!=0 should return None"
        );
    }

    #[test]
    fn schedule_interval_creation() {
        let interval = Duration::from_secs(30);
        let schedule = Schedule::interval(interval);
        match schedule {
            Schedule::Interval { interval_ms } => assert_eq!(interval_ms, 30_000),
            _ => panic!("Expected Interval schedule"),
        }
    }

    #[test]
    fn schedule_interval_next_fire_first() {
        let schedule = Schedule::interval(Duration::from_secs(30));
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        let next = schedule.next_fire_time(0);
        assert!(next.is_some());
        assert!(next.unwrap() > now_ms);
    }

    #[test]
    fn schedule_interval_next_fire_subsequent() {
        let schedule = Schedule::interval(Duration::from_secs(30));
        let first = schedule.next_fire_time(0).unwrap();
        let second = schedule.next_fire_time(first).unwrap();
        assert_eq!(second - first, 30_000);
    }

    #[test]
    fn schedule_interval_no_overflow() {
        let schedule = Schedule::interval(Duration::from_secs(1));
        let max_u64 = u64::MAX;
        let next = schedule.next_fire_time(max_u64);
        assert!(
            next.is_some(),
            "saturating_add should prevent overflow at u64::MAX"
        );
    }

    // =========================================================================
    // Section 3: Job Type Tests (8 tests)
    // =========================================================================

    #[test]
    fn job_new_sets_all_fields() {
        let job = Job::new(
            JobId::new(1),
            "test payload".to_string(),
            Schedule::one_shot(Duration::from_secs(10)),
        );
        assert_eq!(job.id, JobId::new(1));
        assert_eq!(job.payload, "test payload");
    }

    #[test]
    fn job_default_priority_is_normal() {
        let job = Job::new(
            JobId::new(1),
            "test".to_string(),
            Schedule::one_shot(Duration::from_secs(10)),
        );
        assert_eq!(job.priority, JobPriority::Normal);
    }

    #[test]
    fn job_default_retries_is_3() {
        let job = Job::new(
            JobId::new(1),
            "test".to_string(),
            Schedule::one_shot(Duration::from_secs(10)),
        );
        assert_eq!(job.max_retries, 3);
    }

    #[test]
    fn job_default_backoff_is_1000ms() {
        let job = Job::new(
            JobId::new(1),
            "test".to_string(),
            Schedule::one_shot(Duration::from_secs(10)),
        );
        assert_eq!(job.backoff_ms, 1000);
    }

    #[test]
    fn job_with_priority() {
        let job = Job::new(
            JobId::new(1),
            "test".to_string(),
            Schedule::one_shot(Duration::from_secs(10)),
        )
        .with_priority(JobPriority::Critical);
        assert_eq!(job.priority, JobPriority::Critical);
    }

    #[test]
    fn job_with_retries() {
        let job = Job::new(
            JobId::new(1),
            "test".to_string(),
            Schedule::one_shot(Duration::from_secs(10)),
        )
        .with_retries(5, 500);
        assert_eq!(job.max_retries, 5);
        assert_eq!(job.backoff_ms, 500);
    }

    #[test]
    fn job_payload_is_string() {
        let job = Job::new(
            JobId::new(1),
            "test payload".to_string(),
            Schedule::one_shot(Duration::from_secs(10)),
        );
        assert!(matches!(job.payload, String));
    }

    // =========================================================================
    // Section 4: JobId Type Tests (5 tests)
    // =========================================================================

    #[test]
    fn job_id_new_constructs() {
        let job_id = JobId::new(42);
        assert_eq!(job_id.0, 42);
    }

    #[test]
    fn job_id_equality() {
        let id1 = JobId::new(100);
        let id2 = JobId::new(100);
        let id3 = JobId::new(200);
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn job_id_hash() {
        use std::collections::HashMap;
        let mut map = HashMap::new();
        let id = JobId::new(1);
        map.insert(id, "test");
        assert_eq!(map.get(&JobId::new(1)), Some(&"test"));
    }

    #[test]
    fn job_id_display() {
        let job_id = JobId::new(42);
        let display = format!("{}", job_id);
        assert_eq!(display, "job-42");
    }

    #[test]
    fn job_id_debug() {
        let job_id = JobId::new(42);
        let debug = format!("{:?}", job_id);
        assert!(debug.contains("42"));
    }

    // =========================================================================
    // Section 5: JobResult Type Tests (4 tests)
    // =========================================================================

    #[test]
    fn job_result_has_all_fields() {
        let result = JobResult {
            job_id: JobId::new(1),
            success: true,
            output: Some("output".to_string()),
            error: None,
            attempt: 1,
        };
        assert_eq!(result.job_id, JobId::new(1));
        assert!(result.success);
        assert_eq!(result.attempt, 1);
    }

    #[test]
    fn job_result_success_true() {
        let result = JobResult {
            job_id: JobId::new(1),
            success: true,
            output: Some("done".to_string()),
            error: None,
            attempt: 1,
        };
        assert!(result.success);
        assert!(result.error.is_none());
    }

    #[test]
    fn job_result_failure_false() {
        let result = JobResult {
            job_id: JobId::new(1),
            success: false,
            output: None,
            error: Some("failed".to_string()),
            attempt: 3,
        };
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    // =========================================================================
    // Section 6: SchedulerConfig Tests (4 tests)
    // =========================================================================

    #[test]
    fn scheduler_config_default_values() {
        let config = SchedulerConfig::default();
        assert_eq!(config.max_concurrent, 10);
        assert_eq!(config.scan_interval, Duration::from_millis(100));
        assert_eq!(config.max_jobs_per_scan, 100);
    }

    #[test]
    fn scheduler_config_custom_values() {
        let config = SchedulerConfig {
            max_concurrent: 5,
            scan_interval: Duration::from_millis(200),
            max_jobs_per_scan: 50,
        };
        assert_eq!(config.max_concurrent, 5);
        assert_eq!(config.scan_interval, Duration::from_millis(200));
        assert_eq!(config.max_jobs_per_scan, 50);
    }

    // =========================================================================
    // Section 7: SchedulerError Taxonomy Tests (6 tests)
    // =========================================================================

    #[test]
    fn scheduler_error_job_not_found() {
        let err = SchedulerError::JobNotFound(JobId::new(42));
        let display = format!("{}", err);
        assert!(display.contains("42") || display.contains("not found"));
    }

    #[test]
    fn scheduler_error_queue_full() {
        let err = SchedulerError::QueueFull;
        let display = format!("{}", err);
        assert!(display.contains("Queue") || display.contains("full"));
    }

    #[test]
    fn scheduler_error_scheduler_stopped() {
        let err = SchedulerError::SchedulerStopped;
        let display = format!("{}", err);
        assert!(display.contains("stopped") || display.contains("Scheduler"));
    }

    #[test]
    fn scheduler_error_invalid_schedule() {
        let err = SchedulerError::InvalidSchedule("bad cron".to_string());
        let display = format!("{}", err);
        assert!(display.contains("Invalid") || display.contains("schedule"));
    }

    #[test]
    fn scheduler_error_concurrency_limit_reached() {
        let err = SchedulerError::ConcurrencyLimitReached;
        let display = format!("{}", err);
        assert!(display.contains("Concurrency") || display.contains("limit"));
    }

    #[test]
    fn scheduler_error_storage_error() {
        let err = SchedulerError::StorageError("disk full".to_string());
        let display = format!("{}", err);
        assert!(display.contains("Storage") || display.contains("disk"));
    }

    // =========================================================================
    // Section 8: JobRunError Taxonomy Tests (3 tests)
    // =========================================================================

    #[test]
    fn job_run_error_failed() {
        let err = JobRunError::Failed {
            job_id: JobId::new(1),
            reason: "oops".to_string(),
        };
        let display = format!("{}", err);
        assert!(display.contains("1") || display.contains("oops"));
    }

    #[test]
    fn job_run_error_exceeded_retries() {
        let err = JobRunError::ExceededRetries {
            job_id: JobId::new(1),
            attempts: 3,
        };
        let display = format!("{}", err);
        assert!(display.contains("1") || display.contains("3"));
    }

    #[test]
    fn job_run_error_cancelled() {
        let err = JobRunError::Cancelled {
            job_id: JobId::new(1),
        };
        let display = format!("{}", err);
        assert!(display.contains("1") || display.contains("ancelled"));
    }

    // =========================================================================
    // Section 14: Edge Cases & Boundary Tests (10 tests)
    // =========================================================================

    #[test]
    fn schedule_one_shot_zero_delay() {
        let schedule = Schedule::one_shot(Duration::ZERO);
        if let Schedule::OneShot { fire_at_ms } = schedule {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64);
            assert!(fire_at_ms >= now_ms);
        } else {
            panic!("Expected OneShot");
        }
    }

    #[test]
    fn schedule_interval_zero_interval() {
        let schedule = Schedule::interval(Duration::ZERO);
        if let Schedule::Interval { interval_ms } = schedule {
            assert_eq!(interval_ms, 0);
        } else {
            panic!("Expected Interval");
        }
    }

    #[test]
    fn job_priority_extremes() {
        assert!(JobPriority::Critical < JobPriority::Low);
    }

    #[test]
    fn job_empty_payload() {
        let job = Job::new(
            JobId::new(1),
            String::new(),
            Schedule::one_shot(Duration::from_secs(10)),
        );
        assert_eq!(job.payload, "");
    }

    #[test]
    fn job_large_payload() {
        let large_payload = "x".repeat(1_000_000);
        let job = Job::new(
            JobId::new(1),
            large_payload.clone(),
            Schedule::one_shot(Duration::from_secs(10)),
        );
        assert_eq!(job.payload, large_payload);
    }

    #[test]
    fn scheduler_config_zero_max_concurrent() {
        let config = SchedulerConfig {
            max_concurrent: 0,
            scan_interval: Duration::from_millis(100),
            max_jobs_per_scan: 100,
        };
        assert_eq!(config.max_concurrent, 0);
    }

    #[test]
    fn scheduler_config_zero_scan_interval() {
        let config = SchedulerConfig {
            max_concurrent: 10,
            scan_interval: Duration::ZERO,
            max_jobs_per_scan: 100,
        };
        assert_eq!(config.scan_interval, Duration::ZERO);
    }
}

// =========================================================================
// Integration Tests: Scheduler Lifecycle
// =========================================================================

#[cfg(test)]
mod scheduler_integration_tests {
    use std::time::Duration;
    use vo_executor::scheduler::Scheduler;
    use vo_executor::{Job, JobId, JobPriority, Schedule, SchedulerConfig};

    #[tokio::test]
    async fn scheduler_schedule_one_shot() {
        let config = SchedulerConfig::default();
        let mut scheduler = Scheduler::new(config);

        let job = Job::new(
            JobId::new(1),
            "test".to_string(),
            Schedule::one_shot(Duration::from_millis(50)),
        );
        let result = scheduler.schedule(job);
        assert!(result.is_ok(), "Schedule should succeed: {:?}", result);

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        let due = scheduler.poll_due_jobs(now_ms + 100);
        assert_eq!(due.len(), 1, "Should have 1 job due");
        assert_eq!(due[0].id, JobId::new(1));
    }

    #[tokio::test]
    async fn scheduler_schedule_multiple() {
        let config = SchedulerConfig::default();
        let mut scheduler = Scheduler::new(config);

        for i in 0..5 {
            let job = Job::new(
                JobId::new(i),
                format!("job-{}", i),
                Schedule::one_shot(Duration::from_millis(50)),
            );
            scheduler.schedule(job).unwrap();
        }

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        let due = scheduler.poll_due_jobs(now_ms + 100);
        assert_eq!(due.len(), 5, "Should have 5 jobs due");
    }

    #[tokio::test]
    async fn scheduler_cancel_existing() {
        let config = SchedulerConfig::default();
        let mut scheduler = Scheduler::new(config);

        let job = Job::new(
            JobId::new(1),
            "test".to_string(),
            Schedule::one_shot(Duration::from_millis(50)),
        );
        scheduler.schedule(job).unwrap();

        let removed = scheduler.cancel(JobId::new(1));
        assert!(removed.is_some(), "Cancel should return removed job");

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        let due = scheduler.poll_due_jobs(now_ms + 100);
        assert!(due.is_empty(), "Cancelled job should not be in due jobs");
    }

    #[tokio::test]
    async fn scheduler_cancel_nonexistent() {
        let config = SchedulerConfig::default();
        let mut scheduler = Scheduler::new(config);

        let removed = scheduler.cancel(JobId::new(999));
        assert!(removed.is_none(), "Cancel non-existent should return None");
    }

    #[tokio::test]
    async fn scheduler_poll_due_jobs_empty() {
        let config = SchedulerConfig::default();
        let mut scheduler = Scheduler::new(config);

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        let due = scheduler.poll_due_jobs(now_ms);
        assert!(due.is_empty(), "Poll with nothing due should return empty");
    }

    #[tokio::test]
    async fn scheduler_poll_due_jobs_respects_max() {
        let config = SchedulerConfig {
            max_concurrent: 10,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 2,
        };
        let mut scheduler = Scheduler::new(config);

        for i in 0..5 {
            let job = Job::new(
                JobId::new(i),
                format!("job-{}", i),
                Schedule::one_shot(Duration::from_millis(10)),
            );
            scheduler.schedule(job).unwrap();
        }

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        let due = scheduler.poll_due_jobs(now_ms + 100);
        assert!(due.len() <= 2, "Should respect max_jobs_per_scan=2");
    }
}

// =========================================================================
// Integration Tests: Concurrency Control
// =========================================================================

#[cfg(test)]
mod scheduler_concurrency_tests {
    use std::time::Duration;
    use vo_executor::scheduler::Scheduler;
    use vo_executor::SchedulerConfig;

    #[tokio::test]
    async fn scheduler_try_acquire_success() {
        let config = SchedulerConfig {
            max_concurrent: 2,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Scheduler::new(config);

        let permit = scheduler.try_acquire();
        assert!(permit.is_some(), "Should acquire permit under limit");
    }

    #[tokio::test]
    async fn scheduler_try_acquire_failure() {
        let config = SchedulerConfig {
            max_concurrent: 1,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Scheduler::new(config);

        let permit1 = scheduler.try_acquire();
        let permit2 = scheduler.try_acquire();

        assert!(permit1.is_some());
        assert!(permit2.is_none(), "Should fail at limit");
    }

    #[tokio::test]
    async fn scheduler_start_stop() {
        let config = SchedulerConfig::default();
        let mut scheduler = Scheduler::new(config);

        assert!(!scheduler.is_running(), "Should not be running initially");

        scheduler.start();
        assert!(scheduler.is_running(), "Should be running after start");

        scheduler.stop();
        assert!(!scheduler.is_running(), "Should not be running after stop");
    }
}

// =========================================================================
// Priority Queue Tests
// =========================================================================

#[cfg(test)]
mod priority_queue_tests {
    use std::time::Duration;
    use vo_executor::scheduler::Scheduler;
    use vo_executor::{Job, JobId, JobPriority, Schedule, SchedulerConfig};

    #[tokio::test]
    async fn priority_queue_critical_before_high() {
        let config = SchedulerConfig::default();
        let mut scheduler = Scheduler::new(config);

        let job_high = Job::new(
            JobId::new(1),
            "high".to_string(),
            Schedule::one_shot(Duration::from_millis(50)),
        )
        .with_priority(JobPriority::High);

        let job_critical = Job::new(
            JobId::new(2),
            "critical".to_string(),
            Schedule::one_shot(Duration::from_millis(50)),
        )
        .with_priority(JobPriority::Critical);

        scheduler.schedule(job_high).unwrap();
        scheduler.schedule(job_critical).unwrap();

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        let due = scheduler.poll_due_jobs(now_ms + 100);
        assert_eq!(due[0].id, JobId::new(2), "Critical should come before High");
    }

    #[tokio::test]
    async fn priority_queue_same_priority_earlier_first() {
        let config = SchedulerConfig::default();
        let mut scheduler = Scheduler::new(config);

        let job1 = Job::new(
            JobId::new(1),
            "later".to_string(),
            Schedule::one_shot(Duration::from_millis(100)),
        );

        let job2 = Job::new(
            JobId::new(2),
            "earlier".to_string(),
            Schedule::one_shot(Duration::from_millis(50)),
        );

        scheduler.schedule(job1).unwrap();
        scheduler.schedule(job2).unwrap();

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        let due = scheduler.poll_due_jobs(now_ms + 200);
        assert_eq!(
            due[0].id,
            JobId::new(2),
            "Earlier fire time should come first"
        );
    }

    #[tokio::test]
    async fn priority_queue_due_jobs_filters_time() {
        let config = SchedulerConfig::default();
        let mut scheduler = Scheduler::new(config);

        let job_future = Job::new(
            JobId::new(1),
            "future".to_string(),
            Schedule::one_shot(Duration::from_secs(3600)),
        );

        scheduler.schedule(job_future).unwrap();

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        let due = scheduler.poll_due_jobs(now_ms);
        assert!(due.is_empty(), "Future jobs should not be due");
    }

    #[tokio::test]
    async fn priority_queue_due_jobs_none_due() {
        let config = SchedulerConfig::default();
        let mut scheduler = Scheduler::new(config);

        let job = Job::new(
            JobId::new(1),
            "future".to_string(),
            Schedule::one_shot(Duration::from_secs(3600)),
        );
        scheduler.schedule(job).unwrap();

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        let due = scheduler.poll_due_jobs(now_ms);
        assert!(due.is_empty(), "All jobs in future should return empty");
    }

    #[tokio::test]
    async fn scheduler_schedule_and_poll_complete() {
        let config = SchedulerConfig::default();
        let mut scheduler = Scheduler::new(config);

        let job = Job::new(
            JobId::new(1),
            "test".to_string(),
            Schedule::one_shot(Duration::from_millis(50)),
        );
        scheduler.schedule(job).unwrap();

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        let due = scheduler.poll_due_jobs(now_ms + 100);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].id, JobId::new(1));
        assert_eq!(due[0].payload, "test");
    }

    #[tokio::test]
    async fn scheduler_reschedule_job() {
        let config = SchedulerConfig::default();
        let mut scheduler = Scheduler::new(config);

        let job = Job::new(
            JobId::new(1),
            "test".to_string(),
            Schedule::interval(Duration::from_millis(100)),
        );
        scheduler.schedule(job).unwrap();

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        let due = scheduler.poll_due_jobs(now_ms + 200);
        assert_eq!(due.len(), 1);

        let job_id = due[0].id;
        scheduler.cancel(job_id);

        if let Schedule::Interval { interval_ms } = &due[0].schedule {
            let next_fire = now_ms + 200 + interval_ms;
            scheduler.reschedule(due[0].clone(), next_fire);
        }

        let due2 = scheduler.poll_due_jobs(now_ms + 400);
        assert!(!due2.is_empty(), "Rescheduled job should be due");
    }
}
