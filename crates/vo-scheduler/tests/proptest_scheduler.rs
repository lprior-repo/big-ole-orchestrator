//! Proptest generators for vo-scheduler types.
//!
//! These generators enable property-based testing of the scheduler queue,
//! job lifecycle, retry policies, and error classification.

use proptest::prelude::*;
use std::time::Duration;

use vo_scheduler::error::{ExecutionError, RetryExhaustedError, SchedulerError};
use vo_scheduler::job::ScheduledJob;
use vo_scheduler::queue::SchedulerQueue;
use vo_scheduler::types::{
    JobId, JobKind, JobPriority, JobState, RetryPolicy, SchedulePolicy,
};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    // ── ScheduledJob generation and serialization ──

    #[test]
    fn job_roundtrip_serialization(
        kind in any::<JobKind>(),
        priority in any::<JobPriority>(),
        policy in any::<SchedulePolicy>(),
        payload in any::<Vec<u8>>(),
    ) {
        let retry = RetryPolicy::default();
        let job = ScheduledJob::new(kind, priority, policy.clone(), retry, payload.clone().into())
            .unwrap();
        let serialized = serde_json::to_string(&job).unwrap();
        let deserialized: ScheduledJob = serde_json::from_str(&serialized).unwrap();
        assert_eq!(job.id, deserialized.id);
        assert_eq!(job.kind, deserialized.kind);
        assert_eq!(job.priority, deserialized.priority);
        assert_eq!(job.state, deserialized.state);
        assert_eq!(job.payload, deserialized.payload);
    }

    #[test]
    fn job_new_immediate_starts_pending(job_id in any::<Ulid>()) {
        let policy = SchedulePolicy::Immediate;
        let job = ScheduledJob::new(
            JobKind::OneShot,
            JobPriority::Normal,
            policy,
            RetryPolicy::default(),
            bytes::Bytes::new(),
        )
        .unwrap();
        assert_eq!(job.state, JobState::Pending);
    }

    #[test]
    fn job_new_future_starts_scheduled() {
        let future = chrono::Utc::now() + chrono::Duration::hours(24);
        let job = ScheduledJob::new(
            JobKind::OneShot,
            JobPriority::Normal,
            SchedulePolicy::At(future),
            RetryPolicy::default(),
            bytes::Bytes::new(),
        )
        .unwrap();
        assert_eq!(job.state, JobState::Scheduled);
    }

    // ── JobState transitions ──

    #[test]
    fn valid_transition_chain_pending_running_completed() {
        let mut job = ScheduledJob::new(
            JobKind::OneShot,
            JobPriority::Normal,
            SchedulePolicy::Immediate,
            RetryPolicy::default(),
            bytes::Bytes::new(),
        )
        .unwrap();
        assert_eq!(job.state, JobState::Pending);
        job.transition(JobState::Running).unwrap();
        assert_eq!(job.state, JobState::Running);
        job.transition(JobState::Completed).unwrap();
        assert_eq!(job.state, JobState::Completed);
        assert!(job.state.is_terminal());
    }

    #[test]
    fn valid_transition_chain_failed_retrying_pending() {
        let mut job = ScheduledJob::new(
            JobKind::OneShot,
            JobPriority::Normal,
            SchedulePolicy::Immediate,
            RetryPolicy::default(),
            bytes::Bytes::new(),
        )
        .unwrap();
        job.transition(JobState::Running).unwrap();
        job.transition(JobState::Failed).unwrap();
        job.transition(JobState::Retrying).unwrap();
        job.transition(JobState::Pending).unwrap();
        assert_eq!(job.state, JobState::Pending);
        assert!(!job.state.is_terminal());
    }

    #[test]
    fn valid_transition_chain_scheduled_pending() {
        let future = chrono::Utc::now() + chrono::Duration::hours(1);
        let mut job = ScheduledJob::new(
            JobKind::OneShot,
            JobPriority::Normal,
            SchedulePolicy::At(future),
            RetryPolicy::default(),
            bytes::Bytes::new(),
        )
        .unwrap();
        assert_eq!(job.state, JobState::Scheduled);
        job.transition(JobState::Pending).unwrap();
        assert_eq!(job.state, JobState::Pending);
    }

    #[test]
    fn valid_transition_scheduled_or_pending_to_cancelled() {
        let future = chrono::Utc::now() + chrono::Duration::hours(1);
        let mut job = ScheduledJob::new(
            JobKind::OneShot,
            JobPriority::Normal,
            SchedulePolicy::At(future),
            RetryPolicy::default(),
            bytes::Bytes::new(),
        )
        .unwrap();
        job.transition(JobState::Cancelled).unwrap();
        assert!(job.state.is_terminal());

        let mut job2 = ScheduledJob::new(
            JobKind::OneShot,
            JobPriority::Normal,
            SchedulePolicy::Immediate,
            RetryPolicy::default(),
            bytes::Bytes::new(),
        )
        .unwrap();
        job2.transition(JobState::Cancelled).unwrap();
        assert!(job2.state.is_terminal());
    }

    #[test]
    fn invalid_transition_recurring_completed_to_scheduled() {
        let mut job = ScheduledJob::new(
            JobKind::Recurring,
            JobPriority::Normal,
            SchedulePolicy::Immediate,
            RetryPolicy::default(),
            bytes::Bytes::new(),
        )
        .unwrap();
        job.transition(JobState::Running).unwrap();
        job.transition(JobState::Completed).unwrap();
        job.transition(JobState::Scheduled).unwrap();
        assert!(!job.state.is_terminal());
    }

    #[test]
    fn invalid_transition_oneshot_completed_to_scheduled_fails() {
        let mut job = ScheduledJob::new(
            JobKind::OneShot,
            JobPriority::Normal,
            SchedulePolicy::Immediate,
            RetryPolicy::default(),
            bytes::Bytes::new(),
        )
        .unwrap();
        job.transition(JobState::Running).unwrap();
        job.transition(JobState::Completed).unwrap();
        let result = job.transition(JobState::Scheduled);
        assert!(result.is_err());
    }

    // ── RetryPolicy properties ──

    #[test]
    fn retry_policy_can_retry_before_max_attempts(policy_max in 1u32..100u32) {
        let policy = RetryPolicy::try_new(
            policy_max,
            2.0,
            Duration::from_millis(100),
            Duration::from_secs(60),
        )
        .unwrap();
        for i in 0..policy_max {
            assert!(
                policy.can_retry(i),
                "should be able to retry at attempt {i} < {policy_max}"
            );
        }
        assert!(!policy.can_retry(policy_max));
    }

    #[test]
    fn backoff_never_exceeds_max_delay(
        initial_millis in 1u64..1000u64,
        max_millis in 1000u64..60000u64,
    ) {
        let policy = RetryPolicy::try_new(
            10,
            2.0,
            Duration::from_millis(initial_millis),
            Duration::from_millis(max_millis),
        )
        .unwrap();
        for attempt in 0u32..50 {
            let backoff = policy.compute_backoff(attempt);
            assert!(
                backoff <= policy.max_delay,
                "backoff {:?} exceeds max {:?}",
                backoff,
                policy.max_delay
            );
        }
    }

    #[test]
    fn backoff_increases_with_attempt(initial_millis in 1u64..100u64) {
        let max = Duration::from_secs(300);
        let policy = RetryPolicy::try_new(
            10,
            2.0,
            Duration::from_millis(initial_millis),
            max,
        )
        .unwrap();
        let b0 = policy.compute_backoff(0);
        let b1 = policy.compute_backoff(1);
        assert!(b1 >= b0, "backoff should not decrease");
    }

    // ── SchedulerQueue properties ──

    #[test]
    fn queue_len_matches_inserts_and_removals(capacity in 10u32..100u32) {
        let cap = capacity as usize;
        let mut q = SchedulerQueue::new(cap);
        let mut ids = Vec::new();
        for _ in 0..cap {
            let job = ScheduledJob::new(
                JobKind::OneShot,
                JobPriority::Normal,
                SchedulePolicy::Immediate,
                RetryPolicy::default(),
                bytes::Bytes::new(),
            )
            .unwrap();
            ids.push(job.id);
            q.insert(job).unwrap();
        }
        assert_eq!(q.len(), cap);

        // Remove half
        for id in ids.iter().take(cap / 2) {
            q.remove(id).unwrap();
        }
        assert_eq!(q.len(), cap / 2);
    }

    #[test]
    fn queue_capacity_enforced() {
        let q = SchedulerQueue::new(5);
        for _ in 0..5 {
            let job = ScheduledJob::new(
                JobKind::OneShot,
                JobPriority::Normal,
                SchedulePolicy::Immediate,
                RetryPolicy::default(),
                bytes::Bytes::new(),
            )
            .unwrap();
            let _ = q.insert(job);
        }
        let result = q.insert(
            ScheduledJob::new(
                JobKind::OneShot,
                JobPriority::High,
                SchedulePolicy::Immediate,
                RetryPolicy::default(),
                bytes::Bytes::new(),
            )
            .unwrap(),
        );
        assert!(matches!(result, Err(SchedulerError::QueueFull)));
    }

    #[test]
    fn queue_peek_consistent_with_pop() {
        let mut q = SchedulerQueue::new(20);
        for priority in [
            JobPriority::Critical,
            JobPriority::High,
            JobPriority::Normal,
            JobPriority::Low,
            JobPriority::Background,
        ] {
            let job = ScheduledJob::new(
                JobKind::OneShot,
                priority,
                SchedulePolicy::Immediate,
                RetryPolicy::default(),
                bytes::Bytes::new(),
            )
            .unwrap();
            let _ = q.insert(job);
        }
        let peeked = q.peek(chrono::Utc::now()).map(|j| j.id);
        let popped = q.pop_due(chrono::Utc::now()).map(|j| j.id);
        assert_eq!(peeked, popped, "peek and pop must return same job");
    }

    #[test]
    fn queue_is_empty_after_all_popped() {
        let mut q = SchedulerQueue::new(10);
        for _ in 0..5 {
            let job = ScheduledJob::new(
                JobKind::OneShot,
                JobPriority::Normal,
                SchedulePolicy::Immediate,
                RetryPolicy::default(),
                bytes::Bytes::new(),
            )
            .unwrap();
            let _ = q.insert(job);
        }
        while q.pop_due(chrono::Utc::now()).is_some() {}
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
    }

    // ── Error classification ──

    #[test]
    fn scheduler_error_classification() {
        assert!(SchedulerError::QueueFull.is_transient());
        assert!(!SchedulerError::QueueFull.is_permanent());

        assert!(SchedulerError::SerializationError("test".into()).is_transient());
        assert!(!SchedulerError::SerializationError("test".into()).is_permanent());

        assert!(!SchedulerError::JobNotFound.is_transient());
        assert!(SchedulerError::JobNotFound.is_permanent());

        assert!(!SchedulerError::InvalidSchedule.is_transient());
        assert!(SchedulerError::InvalidSchedule.is_permanent());

        assert!(!SchedulerError::InvalidTransition.is_transient());
        assert!(SchedulerError::InvalidTransition.is_permanent());
    }

    #[test]
    fn execution_error_retryable_classification() {
        assert!(ExecutionError::ResourceExhausted.is_retryable());
        assert!(ExecutionError::ResourceExhausted.is_transient());

        assert!(!ExecutionError::Panicked.is_retryable());
        assert!(!ExecutionError::Panicked.is_transient());

        assert!(!ExecutionError::TimedOut.is_retryable());
        assert!(!ExecutionError::TimedOut.is_transient());

        assert!(!ExecutionError::Cancelled.is_retryable());
        assert!(!ExecutionError::Cancelled.is_transient());
    }

    // ── SchedulePolicy validation ──

    #[test]
    fn valid_cron_expressions_accepted(
        minute in "([0-9]|\\*/[1-9][0-9]*)",
        hour in "([0-9]|\\*/[1-9][0-9]*)",
        dom in "([1-9]|\\*/[1-9][0-9]*)",
        month in "([1-9]|\\*/[1-9])",
        dow in "([0-6]|\\*/[1-7])",
    ) {
        let cron = format!("{minute} {hour} {dom} {month} {dow}");
        let result = SchedulePolicy::validate_cron(&cron);
        assert!(
            result.is_ok(),
            "cron '{cron}' should be valid (got: {result:?})"
        );
    }

    #[test]
    fn invalid_cron_rejected(short in "([0-9]+ [0-9]+ [0-9]+)", long in "([0-9]+ [0-9]+ [0-9]+ [0-9]+ [0-9]+ [0-9]+)") {
        assert!(SchedulePolicy::validate_cron(&short).is_err());
        assert!(SchedulePolicy::validate_cron(&long).is_err());
    }

    #[test]
    fn invalid_cron_out_of_range_minute() {
        assert!(SchedulePolicy::validate_cron("60 * * * *").is_err());
    }

    #[test]
    fn invalid_cron_out_of_range_hour() {
        assert!(SchedulePolicy::validate_cron("0 24 * * *").is_err());
    }

    #[test]
    fn invalid_cron_out_of_range_month() {
        assert!(SchedulePolicy::validate_cron("0 0 * 13 *").is_err());
    }

    #[test]
    fn invalid_cron_out_of_range_day_of_week() {
        assert!(SchedulePolicy::validate_cron("0 0 * * 7").is_err());
    }

    // ── RetryPolicy try_new constraints ──

    #[test]
    fn retry_policy_zero_max_attempts_rejected() {
        let result = RetryPolicy::try_new(0, 2.0, Duration::from_secs(1), Duration::from_secs(60));
        assert!(result.is_err());
    }

    #[test]
    fn retry_policy_backoff_below_one_rejected() {
        let result = RetryPolicy::try_new(3, 0.99, Duration::from_secs(1), Duration::from_secs(60));
        assert!(result.is_err());
    }

    #[test]
    fn retry_policy_zero_initial_delay_rejected() {
        let result = RetryPolicy::try_new(3, 2.0, Duration::ZERO, Duration::from_secs(60));
        assert!(result.is_err());
    }

    #[test]
    fn retry_policy_max_below_initial_rejected() {
        let result = RetryPolicy::try_new(3, 2.0, Duration::from_secs(10), Duration::from_secs(5));
        assert!(result.is_err());
    }

    #[test]
    fn retry_policy_valid_boundary_values() {
        let policy = RetryPolicy::try_new(1, 1.0, Duration::from_millis(1), Duration::from_secs(300));
        assert!(policy.is_ok());
    }
}

// ── Custom proptest strategy for JobPriority ──

impl Strategy for JobPriority {
    type Item = Self;
    type Strategy = proptest::collection::IndexVecStrategy<usize, Self>;

    fn strategy(&self) -> Self::Strategy {
        proptest::collection::index_vec(any::<u8>().prop_map(|v| JobPriority::from_u8(v)))
    }
}

// ── Helper strategies for types without built-in strategies ──

/// Proptest strategy for JobKind.
pub fn any_job_kind() -> impl Strategy<Value = JobKind> {
    prop_oneof![
        Just(JobKind::OneShot),
        Just(JobKind::Recurring),
        Just(JobKind::Delayed),
    ]
}

/// Proptest strategy for JobPriority.
pub fn any_job_priority() -> impl Strategy<Value = JobPriority> {
    prop_oneof![
        Just(JobPriority::Critical),
        Just(JobPriority::High),
        Just(JobPriority::Normal),
        Just(JobPriority::Low),
        Just(JobPriority::Background),
    ]
}

/// Proptest strategy for JobState.
pub fn any_job_state() -> impl Strategy<Value = JobState> {
    prop_oneof![
        Just(JobState::Scheduled),
        Just(JobState::Pending),
        Just(JobState::Running),
        Just(JobState::Completed),
        Just(JobState::Failed),
        Just(JobState::Cancelled),
        Just(JobState::Retrying),
    ]
}

/// Proptest strategy for SchedulePolicy.
pub fn any_schedule_policy() -> impl Strategy<Value = SchedulePolicy> {
    prop_oneof![
        any::<chrono::DateTime<chrono::Utc>>().prop_map(SchedulePolicy::At),
        (1u64..3600000).prop_map(|ms| SchedulePolicy::After(Duration::from_millis(ms))),
        any::<String>()
            .prop_filter("valid cron", |s| {
                s.split_whitespace().count() == 5
            })
            .prop_map(SchedulePolicy::Cron),
        Just(SchedulePolicy::Immediate),
    ]
}

/// Proptest strategy for RetryPolicy.
pub fn any_retry_policy() -> impl Strategy<Value = RetryPolicy> {
    (
        1u32..10u32,
        1.0f64..3.0f64,
        1u64..5000u64,
        1000u64..300000u64,
    )
        .prop_map(|(max, mult, init, max_d)| {
            // Ensure max_delay >= initial_delay by adjusting
            let init = Duration::from_millis(init);
            let max_delay = Duration::from_millis(max_d.max(init.as_millis() as u64));
            RetryPolicy::try_new(max, mult, init, max_delay).unwrap()
        })
}

/// Proptest strategy for ScheduledJob.
pub fn any_scheduled_job() -> impl Strategy<Value = ScheduledJob> {
    (
        any_job_kind(),
        any_job_priority(),
        any_schedule_policy(),
        any_retry_policy(),
        any::<Vec<u8>>(),
    )
        .prop_map(|(kind, priority, policy, retry, payload)| {
            ScheduledJob::new(kind, priority, policy, retry, payload.into()).unwrap()
        })
}
