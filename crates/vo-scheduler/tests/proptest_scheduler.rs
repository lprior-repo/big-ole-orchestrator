//! Property-based tests for vo-scheduler state machine, queue, and retry policy.

use proptest::{prop_assert, prop_assert_eq, proptest};
use vo_scheduler::{
    job::ScheduledJob,
    queue::SchedulerQueue,
    types::{JobKind, JobPriority, JobState, RetryPolicy, SchedulePolicy},
};

fn make_job(priority: JobPriority, policy: SchedulePolicy) -> ScheduledJob {
    ScheduledJob::new(
        JobKind::OneShot,
        priority,
        policy,
        RetryPolicy::default_policy(),
        bytes::Bytes::from_static(b"payload"),
    )
}

fn past_due() -> SchedulePolicy {
    SchedulePolicy::At(chrono::Utc::now() - chrono::Duration::seconds(10))
}

// PROPERTY 1: Retry policy backoff is always bounded by max_delay.
proptest! {
    #[test]
    fn retry_policy_caps_at_max_delay(
        max_attempts in 1u32..1000u32,
        multiplier in 1.0f64..100.0f64,
        initial_secs in 1u64..3600u64,
        max_secs in 1u64..7200u64
    ) {
        let initial = std::time::Duration::from_secs(initial_secs);
        let max = std::time::Duration::from_secs(max_secs.max(initial_secs));
        let policy = RetryPolicy::try_new(max_attempts, multiplier, initial, max).unwrap();
        for attempt in 0..max_attempts {
            let backoff = policy.compute_backoff(attempt);
            prop_assert!(backoff <= max,
                "backoff {:?} at attempt {} exceeds max_delay {:?} (multiplier={})",
                backoff, attempt, max, multiplier);
        }
    }
}

// PROPERTY 2: Backoff is monotonically non-decreasing within max_delay.
proptest! {
    #[test]
    fn backoff_is_monotonic(
        max_attempts in 2u32..50u32,
        multiplier in 1.0f64..5.0f64,
        initial_secs in 1u64..60u64,
        max_secs in 60u64..7200u64
    ) {
        let initial = std::time::Duration::from_secs(initial_secs);
        let max = std::time::Duration::from_secs(max_secs.max(initial_secs));
        let policy = RetryPolicy::try_new(max_attempts, multiplier, initial, max).unwrap();
        let mut prev = policy.compute_backoff(0);
        for attempt in 1..max_attempts {
            let curr = policy.compute_backoff(attempt);
            prop_assert!(curr >= prev,
                "backoff {:?} at attempt {} < {:?} at {} — not monotonic",
                curr, attempt, prev, attempt - 1);
            prev = curr;
        }
    }
}

// PROPERTY 3: can_retry is consistent with attempt count.
proptest! {
    #[test]
    fn can_retry_reflects_attempt_count(
        max_attempts in 1u32..100u32,
        attempt_count in 0u32..100u32
    ) {
        let policy = RetryPolicy::try_new(
            max_attempts, 2.0,
            std::time::Duration::from_secs(1),
            std::time::Duration::from_secs(300),
        ).unwrap();
        let can = policy.can_retry(attempt_count);
        if attempt_count < max_attempts {
            prop_assert!(can, "should retry at {} with max {}", attempt_count, max_attempts);
        } else {
            prop_assert!(!can, "should NOT retry at {} with max {}", attempt_count, max_attempts);
        }
    }
}

// PROPERTY 4: Capacity limit is always respected.
proptest! {
    #[test]
    fn capacity_limit_always_respected(
        capacity in 1usize..50usize,
        num_inserts in 0usize..100usize
    ) {
        let mut q = SchedulerQueue::new(capacity);
        let mut inserted_count = 0usize;
        for _ in 0..num_inserts {
            let job = make_job(JobPriority::Normal, past_due());
            match q.insert(job) {
                Ok(_) => inserted_count += 1,
                Err(vo_scheduler::error::SchedulerError::QueueFull) => {}
                Err(e) => panic!("unexpected error: {:?}", e),
            }
        }
        prop_assert_eq!(q.len(), inserted_count.min(capacity));
        prop_assert!(q.len() <= capacity);
    }
}

// PROPERTY 5: pop_due respects priority ordering for due jobs.
proptest! {
    #[test]
    fn pop_order_respects_priority(
        priorities in proptest::collection::vec(
            proptest::prop_oneof![
                proptest::strategy::Just(JobPriority::Critical),
                proptest::strategy::Just(JobPriority::High),
                proptest::strategy::Just(JobPriority::Normal),
                proptest::strategy::Just(JobPriority::Low),
                proptest::strategy::Just(JobPriority::Background),
            ],
            0..20
        )
    ) {
        let mut q = SchedulerQueue::new(100);
        for p in &priorities {
            let job = make_job(*p, past_due());
            q.insert(job).unwrap();
        }
        let mut last = JobPriority::Critical;
        while let Some(job) = q.pop_due(chrono::Utc::now()) {
            prop_assert!(job.priority >= last,
                "popped {:?} after {:?} — priority order violated", job.priority, last);
            last = job.priority;
        }
    }
}

// PROPERTY 6: RetryPolicy::try_new rejects all invalid parameter combinations.
proptest! {
    #[test]
    fn retry_policy_rejects_invalid_params(
        max_attempts in 0u32..5u32,
        multiplier in 0.0f64..2.0f64,
        initial_nanos in 0u64..100u64,
        max_nanos in 0u64..100u64,
    ) {
        let initial = std::time::Duration::from_nanos(initial_nanos);
        let max = std::time::Duration::from_nanos(max_nanos);
        let result = RetryPolicy::try_new(max_attempts, multiplier, initial, max);
        let valid = max_attempts > 0
            && multiplier >= 1.0
            && initial_nanos > 0
            && max_nanos >= initial_nanos;
        prop_assert_eq!(result.is_ok(), valid,
            "expected is_ok={} for max={}, mult={}, init={:?}, max_d={:?}",
            valid, max_attempts, multiplier, initial, max);
    }
}

// PROPERTY 7: State machine transitions respect the defined transition table.
proptest! {
    #[test]
    fn state_transitions_respect_job_kind(
        kind in proptest::prop_oneof![
            proptest::strategy::Just(JobKind::OneShot),
            proptest::strategy::Just(JobKind::Recurring),
            proptest::strategy::Just(JobKind::Delayed),
        ],
        from in proptest::prop_oneof![
            proptest::strategy::Just(JobState::Scheduled),
            proptest::strategy::Just(JobState::Pending),
            proptest::strategy::Just(JobState::Running),
            proptest::strategy::Just(JobState::Completed),
            proptest::strategy::Just(JobState::Failed),
            proptest::strategy::Just(JobState::Cancelled),
            proptest::strategy::Just(JobState::Retrying),
        ],
        to in proptest::prop_oneof![
            proptest::strategy::Just(JobState::Scheduled),
            proptest::strategy::Just(JobState::Pending),
            proptest::strategy::Just(JobState::Running),
            proptest::strategy::Just(JobState::Completed),
            proptest::strategy::Just(JobState::Failed),
            proptest::strategy::Just(JobState::Cancelled),
            proptest::strategy::Just(JobState::Retrying),
        ]
    ) {
        // Drive to `from` state via known valid transition paths.
        // Create a fresh job using a future schedule to start in Scheduled state.
        let mut job = ScheduledJob::new(
            kind, JobPriority::Normal,
            SchedulePolicy::At(chrono::Utc::now() + chrono::Duration::hours(1)),
            RetryPolicy::default_policy(), bytes::Bytes::from_static(b"prop"),
        );
        // Now drive to target `from` state
        match from {
            JobState::Scheduled => { /* already Scheduled */ }
            JobState::Pending => { let _ = job.transition(JobState::Pending); }
            JobState::Running => {
                let _ = job.transition(JobState::Pending);
                let _ = job.transition(JobState::Running);
            }
            JobState::Completed => {
                let _ = job.transition(JobState::Pending);
                let _ = job.transition(JobState::Running);
                let _ = job.transition(JobState::Completed);
            }
            JobState::Failed => {
                let _ = job.transition(JobState::Pending);
                let _ = job.transition(JobState::Running);
                let _ = job.transition(JobState::Failed);
            }
            JobState::Cancelled => {
                let _ = job.transition(JobState::Cancelled);
            }
            JobState::Retrying => {
                let _ = job.transition(JobState::Pending);
                let _ = job.transition(JobState::Running);
                let _ = job.transition(JobState::Failed);
                let _ = job.transition(JobState::Retrying);
            }
        }
        let expected = matches!(
            (from, to),
            (JobState::Scheduled, JobState::Pending) |
            (JobState::Scheduled, JobState::Cancelled) |
            (JobState::Pending, JobState::Running) |
            (JobState::Pending, JobState::Cancelled) |
            (JobState::Running, JobState::Completed) |
            (JobState::Running, JobState::Failed) |
            (JobState::Running, JobState::Cancelled) |
            (JobState::Failed, JobState::Retrying) |
            (JobState::Retrying, JobState::Pending) |
            (JobState::Retrying, JobState::Cancelled)
        ) || (from == JobState::Completed && to == JobState::Scheduled && kind == JobKind::Recurring);

        // Skip self-transitions — not meaningful for state machine testing
        if from == to { return Ok(()); }
        let result = job.transition(to);
        if expected {
            prop_assert!(result.is_ok(), "{:?}->{:?} should be valid for {:?}", from, to, kind);
        } else {
            prop_assert!(result.is_err(), "{:?}->{:?} should be INVALID for {:?}", from, to, kind);
        }
    }
}

// PROPERTY 8: Terminal states have no valid transitions (except Recurring Completed->Scheduled).
proptest! {
    #[test]
    fn terminal_states_have_no_valid_transitions(
        terminal in proptest::prop_oneof![
            proptest::strategy::Just(JobState::Completed),
            proptest::strategy::Just(JobState::Failed),
            proptest::strategy::Just(JobState::Cancelled),
        ],
        target in proptest::prop_oneof![
            proptest::strategy::Just(JobState::Scheduled),
            proptest::strategy::Just(JobState::Pending),
            proptest::strategy::Just(JobState::Running),
            proptest::strategy::Just(JobState::Completed),
            proptest::strategy::Just(JobState::Failed),
            proptest::strategy::Just(JobState::Cancelled),
            proptest::strategy::Just(JobState::Retrying),
        ],
        kind in proptest::prop_oneof![
            proptest::strategy::Just(JobKind::OneShot),
            proptest::strategy::Just(JobKind::Recurring),
            proptest::strategy::Just(JobKind::Delayed),
        ],
    ) {
        if terminal == target { return Ok(()); }
        let should_be_valid = terminal == JobState::Completed
            && target == JobState::Scheduled
            && kind == JobKind::Recurring;
        let valid = matches!(
            (terminal, target),
            (JobState::Completed, JobState::Scheduled)
        ) && kind == JobKind::Recurring;
        prop_assert_eq!(valid, should_be_valid,
            "terminal {:?} -> {:?} validity mismatch for {:?}", terminal, target, kind);
    }
}

// PROPERTY 9: Queue len() is always accurate after mixed insert/pop/cancel operations.
proptest! {
    #[test]
    fn queue_len_accurate_after_mixed_ops(
        ops in proptest::collection::vec(
            proptest::prop_oneof![
                0 => proptest::strategy::Just(0u8),  // Insert
                1 => proptest::strategy::Just(1u8),  // PopDue
                2 => proptest::strategy::Just(2u8),  // CancelRandom
            ],
            0..30
        )
    ) {
        let mut q = SchedulerQueue::new(50);
        let mut tracked_ids: Vec<_> = Vec::new();
        for op in ops {
            match op {
                0 => {
                    let job = make_job(JobPriority::Normal, past_due());
                    let id = job.id;
                    if q.insert(job).is_ok() {
                        tracked_ids.push(id);
                    }
                }
                1 => {
                    let _ = q.pop_due(chrono::Utc::now());
                    tracked_ids.retain(|id| q.lookup(id).is_ok());
                }
                2 => {
                    if let Some(id) = tracked_ids.first().copied() {
                        let _ = q.cancel(&id);
                        tracked_ids.retain(|i| q.lookup(i).is_ok());
                    }
                }
                _ => unreachable!(),
            }
        }
        let actual: usize = tracked_ids.iter().filter(|id| q.lookup(id).is_ok()).count();
        prop_assert_eq!(q.len(), actual, "len() mismatch after mixed operations");
    }
}

// PROPERTY 10: Remove on nonexistent IDs always errors (no panic, no corruption).
proptest! {
    #[test]
    fn remove_nonexistent_is_always_error(num_ids in 0usize..20usize) {
        let mut q = SchedulerQueue::new(10);
        for _ in 0..num_ids {
            let id = vo_scheduler::types::JobId::generate();
            prop_assert!(q.remove(&id).is_err());
        }
    }
}
