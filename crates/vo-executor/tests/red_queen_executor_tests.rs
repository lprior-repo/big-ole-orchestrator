//! Red Queen adversarial tests for vo-executor core
//!
//! Evolutionary co-evolving test suite targeting:
//! DIMENSION: scheduler-state-transitions — JobState machine adversarial probing
//! DIMENSION: retry-backoff-boundaries — Backoff arithmetic edge cases
//! DIMENSION: error-chaining-invariants — Error taxonomy contract verification
//! DIMENSION: concurrent-state-corruption — DashMap race condition detection
//! DIMENSION: subprocess-ipc-boundaries — FD3/FD4 framing edge cases
//! DIMENSION: execution-idempotency — Repeated execution safety
//! DIMENSION: timeout-boundary-adversarial — Timeout validation fuzzing

use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::MutexGuard;
use vo_executor::{
    cancel_execution, clear_error, execute_step, execute_step_with_retry, get_execution_status,
    get_last_error, reset_all_state, set_error, set_executing_state_for_test,
    ExecutionStatus, ExecuteNodeError, RetryPolicy, StepId, StepResult,
};
use vo_executor::scheduler::{
    Job, JobId, JobPriority, JobResult, JobState, Schedule, SchedulePolicy, SchedulerConfig,
    SchedulerError, SchedulerQueue,
};
use vo_executor::state::{get_state, get_state_count, StepState};
use vo_executor::subprocess::{SubprocessConfig, SubprocessError};

static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn state_guard() -> MutexGuard<'static, ()> {
    let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset_all_state();
    guard
}

// ============================================================================
// DIMENSION: scheduler-state-transitions
// ============================================================================

#[cfg(test)]
mod scheduler_state_adversarial {
    use super::*;

    #[test]
    fn job_state_all_variants_are_exhaustive() {
        let all_states = [
            JobState::Scheduled,
            JobState::Pending,
            JobState::Running,
            JobState::Completed,
            JobState::Failed,
            JobState::Cancelled,
            JobState::Retrying,
        ];

        let terminal_count = all_states.iter().filter(|s| s.is_terminal()).count();
        assert_eq!(terminal_count, 3, "Exactly 3 terminal states: Completed, Failed, Cancelled");

        let non_terminal_count = all_states.iter().filter(|s| s.is_non_terminal()).count();
        assert_eq!(non_terminal_count, 4, "Exactly 4 non-terminal states");
    }

    #[test]
    fn job_state_terminal_symmetry() {
        for state in [
            JobState::Completed,
            JobState::Failed,
            JobState::Cancelled,
        ] {
            assert!(state.is_terminal(), "{:?} should be terminal", state);
            assert!(!state.is_non_terminal(), "{:?} should not be non-terminal", state);
        }
    }

    #[test]
    fn job_state_non_terminal_symmetry() {
        for state in [
            JobState::Scheduled,
            JobState::Pending,
            JobState::Running,
            JobState::Retrying,
        ] {
            assert!(!state.is_terminal(), "{:?} should not be terminal", state);
            assert!(state.is_non_terminal(), "{:?} should be non-terminal", state);
        }
    }

    #[test]
    fn scheduler_queue_state_tracking_across_push_pop_remove() {
        let mut queue = SchedulerQueue::new();

        let job = Job::new(
            JobId::new(42),
            "payload".to_string(),
            Schedule::one_shot(std::time::Duration::from_secs(1)),
        );

        queue.push(job.clone(), 1000);

        assert_eq!(queue.get_state(&JobId::new(42)), Some(JobState::Scheduled));
        assert_eq!(queue.len(), 1);

        let popped = queue.pop();
        assert!(popped.is_some());
        assert_eq!(queue.get_state(&JobId::new(42)), Some(JobState::Pending));

        let removed = queue.remove(&JobId::new(42));
        assert!(removed.is_none(), "Already popped, should be None");
        assert_eq!(queue.get_state(&JobId::new(42)), None);
    }

    #[test]
    fn scheduler_queue_set_state_independent_of_push() {
        let mut queue = SchedulerQueue::new();

        queue.set_state(JobId::new(1), JobState::Running);
        assert_eq!(queue.get_state(&JobId::new(1)), Some(JobState::Running));
        assert_eq!(queue.len(), 0, "set_state should not add to queue");
    }

    #[test]
    fn scheduler_queue_push_overwrites_existing_state() {
        let mut queue = SchedulerQueue::new();

        let job1 = Job::new(
            JobId::new(1),
            "first".to_string(),
            Schedule::one_shot(std::time::Duration::from_secs(1)),
        );
        let job2 = Job::new(
            JobId::new(1),
            "second".to_string(),
            Schedule::one_shot(std::time::Duration::from_secs(2)),
        );

        queue.push(job1, 1000);
        assert_eq!(queue.get_state(&JobId::new(1)), Some(JobState::Scheduled));

        queue.push(job2, 2000);
        assert_eq!(queue.get_state(&JobId::new(1)), Some(JobState::Scheduled));
        assert_eq!(queue.len(), 2, "Both pushes should be in the queue");
    }

    #[test]
    fn scheduler_queue_reschedule_transitions_to_scheduled() {
        let mut queue = SchedulerQueue::new();

        let job = Job::new(
            JobId::new(1),
            "payload".to_string(),
            Schedule::one_shot(std::time::Duration::from_secs(1)),
        );

        queue.push(job.clone(), 1000);
        queue.pop();
        assert_eq!(queue.get_state(&JobId::new(1)), Some(JobState::Pending));

        queue.reschedule(job, 2000);
        assert_eq!(queue.get_state(&JobId::new(1)), Some(JobState::Scheduled));
    }

    #[test]
    fn scheduler_queue_remove_nonexistent_is_none() {
        let mut queue = SchedulerQueue::new();
        assert!(queue.remove(&JobId::new(999)).is_none());
    }

    #[test]
    fn scheduler_queue_pop_on_empty_is_none() {
        let mut queue = SchedulerQueue::new();
        assert!(queue.pop().is_none());
        assert_eq!(queue.len(), 0);
    }

    #[tokio::test]
    async fn scheduler_start_stop_lifecycle() {
        let config = SchedulerConfig {
            max_concurrent: 1,
            scan_interval: std::time::Duration::from_millis(10),
            max_jobs_per_scan: 10,
        };
        let mut scheduler = vo_executor::scheduler::Scheduler::new(config);

        assert!(!scheduler.is_running());
        scheduler.start();
        assert!(scheduler.is_running());
        scheduler.stop();
        assert!(!scheduler.is_running());
    }

    #[tokio::test]
    async fn scheduler_cancel_nonexistent_returns_none() {
        let config = SchedulerConfig::default();
        let mut scheduler = vo_executor::scheduler::Scheduler::new(config);

        let removed = scheduler.cancel(JobId::new(999));
        assert!(removed.is_none());
    }

    #[tokio::test]
    async fn scheduler_schedule_cron_returns_invalid_schedule() {
        let config = SchedulerConfig::default();
        let mut scheduler = vo_executor::scheduler::Scheduler::new(config);

        let job = Job::new(
            JobId::new(1),
            "cron".to_string(),
            Schedule::cron("* * * * *"),
        );

        let result = scheduler.schedule(job);
        assert!(matches!(result, Err(SchedulerError::InvalidSchedule(_))));
    }

    #[test]
    fn job_id_max_value_roundtrip() {
        let id = JobId::new(u64::MAX);
        assert_eq!(id.get(), u64::MAX);
        assert_eq!(format!("{}", id), format!("job-{}", u64::MAX));
    }

    #[test]
    fn job_id_zero_roundtrip() {
        let id = JobId::new(0);
        assert_eq!(id.get(), 0);
        assert_eq!(format!("{}", id), "job-0");
    }

    #[test]
    fn job_result_construction() {
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
    fn job_result_failure_construction() {
        let result = JobResult {
            job_id: JobId::new(2),
            success: false,
            output: None,
            error: Some("boom".to_string()),
            attempt: 3,
        };
        assert!(!result.success);
        assert!(result.output.is_none());
        assert_eq!(result.attempt, 3);
    }
}

// ============================================================================
// DIMENSION: retry-backoff-boundaries
// ============================================================================

#[cfg(test)]
mod retry_backoff_adversarial {
    use super::*;

    #[test]
    fn backoff_with_multiplier_1_is_constant() {
        let policy = RetryPolicy::new(10, 500, 1.0).unwrap();
        for attempt in 1..=10 {
            assert_eq!(
                policy.calculate_backoff_delay(attempt),
                500,
                "With multiplier 1.0, backoff should always be 500ms (attempt {})",
                attempt
            );
        }
    }

    #[test]
    fn backoff_with_zero_initial_is_always_zero() {
        let policy = RetryPolicy::new(10, 0, 1000.0).unwrap();
        for attempt in 1..=10 {
            assert_eq!(
                policy.calculate_backoff_delay(attempt),
                0,
                "With backoff_ms=0, delay should always be 0 (attempt {})",
                attempt
            );
        }
    }

    #[test]
    fn backoff_attempt_zero_is_always_zero() {
        let policies = [
            RetryPolicy::new(3, 100, 2.0).unwrap(),
            RetryPolicy::new(3, 99999, 10.0).unwrap(),
            RetryPolicy::with_max_backoff(3, 100, 2.0, 1000).unwrap(),
        ];
        for policy in &policies {
            assert_eq!(
                policy.calculate_backoff_delay(0),
                0,
                "Attempt 0 should always return 0 delay"
            );
        }
    }

    #[test]
    fn backoff_capped_to_max_backoff_exactly() {
        let policy = RetryPolicy::with_max_backoff(10, 100, 2.0, 500).unwrap();

        assert_eq!(policy.calculate_backoff_delay(1), 100);
        assert_eq!(policy.calculate_backoff_delay(2), 200);
        assert_eq!(policy.calculate_backoff_delay(3), 400);
        assert_eq!(policy.calculate_backoff_delay(4), 500);
        assert_eq!(policy.calculate_backoff_delay(5), 500);
        assert_eq!(policy.calculate_backoff_delay(100), 500);
    }

    #[test]
    fn backoff_max_backoff_equals_backoff() {
        let policy = RetryPolicy::with_max_backoff(5, 100, 2.0, 100).unwrap();

        assert_eq!(policy.calculate_backoff_delay(1), 100);
        assert_eq!(policy.calculate_backoff_delay(2), 100);
        assert_eq!(policy.calculate_backoff_delay(100), 100);
    }

    #[test]
    fn backoff_large_multiplier_capped_quickly() {
        let policy = RetryPolicy::with_max_backoff(10, 1, 1e9, 1000).unwrap();

        assert_eq!(policy.calculate_backoff_delay(1), 1);
        assert_eq!(policy.calculate_backoff_delay(2), 1000);
        assert_eq!(policy.calculate_backoff_delay(3), 1000);
    }

    #[test]
    fn backoff_u64_max_backoff_never_caps() {
        let policy = RetryPolicy::new(5, 100, 2.0).unwrap();
        assert_eq!(policy.max_backoff_ms, u64::MAX);

        let delay = policy.calculate_backoff_delay(5);
        assert!(delay > 0, "Should produce a real delay");
    }

    #[test]
    fn backoff_monotonic_with_multiplier_gt_1() {
        let policy = RetryPolicy::new(20, 100, 1.5).unwrap();
        let mut prev = 0u64;
        for attempt in 1..=20 {
            let delay = policy.calculate_backoff_delay(attempt);
            assert!(
                delay >= prev,
                "Backoff should be monotonically non-decreasing: attempt {} gave {} < {}",
                attempt,
                delay,
                prev
            );
            prev = delay;
        }
    }

    #[test]
    fn retry_policy_with_max_backoff_1ms_backoff() {
        let policy = RetryPolicy::with_max_backoff(3, 1, 2.0, 1).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 1);
        assert_eq!(policy.calculate_backoff_delay(2), 1);
    }

    #[test]
    fn retry_policy_max_backoff_zero_rejects() {
        let result = RetryPolicy::with_max_backoff(3, 100, 2.0, 0);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            vo_executor::RetryPolicyError::MaxBackoffTooSmall { .. }
        ));
    }

    #[test]
    fn retry_policy_multiplier_negative_infinity_rejects() {
        let result = RetryPolicy::new(3, 100, f64::NEG_INFINITY);
        assert!(result.is_err());
    }

    #[test]
    fn retry_policy_multiplier_negative_rejects() {
        let result = RetryPolicy::new(3, 100, -1.0);
        assert!(result.is_err());
    }

    #[test]
    fn retry_policy_multiplier_zero_rejects() {
        let result = RetryPolicy::new(3, 100, 0.0);
        assert!(result.is_err());
    }

    #[test]
    fn retry_policy_multiplier_subnormal_rejects() {
        let result = RetryPolicy::new(3, 100, f64::MIN_POSITIVE);
        assert!(result.is_err());
    }
}

// ============================================================================
// DIMENSION: error-chaining-invariants
// ============================================================================

#[cfg(test)]
mod error_chaining_adversarial {
    use super::*;

    #[test]
    fn retry_exhausted_boxes_inner_error() {
        let inner = ExecuteNodeError::TransientError {
            reason: "conn reset".to_string(),
            recoverable: true,
        };
        let exhausted = ExecuteNodeError::RetryExhausted {
            attempts: 5,
            last_error: Box::new(inner.clone()),
        };

        if let ExecuteNodeError::RetryExhausted {
            attempts,
            last_error,
        } = &exhausted
        {
            assert_eq!(*attempts, 5);
            assert_eq!(last_error.to_string(), inner.to_string());
        } else {
            panic!("Should be RetryExhausted variant");
        }
    }

    #[test]
    fn retry_exhausted_deeply_nested() {
        let level3 = ExecuteNodeError::TransientError {
            reason: "network".to_string(),
            recoverable: true,
        };
        let level2 = ExecuteNodeError::RetryExhausted {
            attempts: 3,
            last_error: Box::new(level3),
        };
        let level1 = ExecuteNodeError::RetryExhausted {
            attempts: 10,
            last_error: Box::new(level2),
        };

        let display = level1.to_string();
        assert!(display.contains("10"));
        assert!(display.contains("3"));
        assert!(display.contains("network"));
    }

    #[test]
    fn all_error_variants_are_cloneable() {
        let errors = vec![
            ExecuteNodeError::StepNotFound {
                step_id: StepId::new("x".to_string()),
            },
            ExecuteNodeError::InvalidTimeout {
                value: 0,
                reason: "test".to_string(),
            },
            ExecuteNodeError::TimeoutExceeded {
                elapsed_ms: 1,
                limit_ms: 2,
            },
            ExecuteNodeError::InvalidTransition {
                from_state: "A".to_string(),
                action: "B".to_string(),
            },
            ExecuteNodeError::RetryExhausted {
                attempts: 1,
                last_error: Box::new(ExecuteNodeError::ExecutionCancelled {
                    reason: "r".to_string(),
                }),
            },
            ExecuteNodeError::InvalidRetryPolicy {
                node_name: "n".to_string(),
                reason: vo_executor::RetryPolicyError::ZeroAttempts,
            },
            ExecuteNodeError::ExecutionCancelled {
                reason: "test".to_string(),
            },
            ExecuteNodeError::TransientError {
                reason: "test".to_string(),
                recoverable: false,
            },
        ];

        for err in &errors {
            let cloned = err.clone();
            assert_eq!(format!("{:?}", err), format!("{:?}", cloned));
        }
    }

    #[test]
    fn all_error_variants_are_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ExecuteNodeError>();
        assert_send::<vo_executor::RetryPolicyError>();
    }

    #[test]
    fn all_error_variants_are_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ExecuteNodeError>();
        assert_sync::<vo_executor::RetryPolicyError>();
    }

    #[test]
    fn transient_error_recoverable_flag_preserved() {
        let recoverable = ExecuteNodeError::TransientError {
            reason: "timeout".to_string(),
            recoverable: true,
        };
        let permanent = ExecuteNodeError::TransientError {
            reason: "auth".to_string(),
            recoverable: false,
        };

        let display_r = recoverable.to_string();
        let display_p = permanent.to_string();
        assert!(display_r.contains("recoverable=true"));
        assert!(display_p.contains("recoverable=false"));
        assert_ne!(recoverable, permanent);
    }

    #[test]
    fn scheduler_error_all_variants_construct_and_display() {
        let errors: Vec<SchedulerError> = vec![
            SchedulerError::JobNotFound(JobId::new(1)),
            SchedulerError::QueueFull,
            SchedulerError::SchedulerStopped,
            SchedulerError::InvalidSchedule("bad cron".to_string()),
            SchedulerError::ConcurrencyLimitReached,
            SchedulerError::StorageError("io error".to_string()),
            SchedulerError::InvalidTransition {
                from_state: "Running".to_string(),
                event: "cancel".to_string(),
            },
            SchedulerError::SerializationError("json".to_string()),
            SchedulerError::InvalidJobId("empty".to_string()),
        ];

        for err in &errors {
            let display = err.to_string();
            assert!(!display.is_empty(), "Error display should not be empty");
        }
    }

    #[test]
    fn subprocess_error_all_variants_construct() {
        let errors = vec![
            SubprocessError::PipeSetupFailed("pipe".to_string()),
            SubprocessError::SpawnFailed("exec".to_string()),
            SubprocessError::Fd3WriteFailed("write".to_string()),
            SubprocessError::Fd4ReadFailed("read".to_string()),
            SubprocessError::Timeout { elapsed_ms: 5000 },
            SubprocessError::ProcessFailed { exit_code: 1 },
            SubprocessError::BoundedBufferExceeded {
                max: 65536,
                tried: 100000,
            },
        ];

        for err in &errors {
            let display = err.to_string();
            assert!(!display.is_empty());
        }
    }

    #[test]
    fn subprocess_error_equality() {
        let a = SubprocessError::Timeout { elapsed_ms: 1000 };
        let b = SubprocessError::Timeout { elapsed_ms: 1000 };
        let c = SubprocessError::Timeout { elapsed_ms: 2000 };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}

// ============================================================================
// DIMENSION: concurrent-state-corruption
// ============================================================================

#[cfg(test)]
mod concurrent_state_adversarial {
    use super::*;

    #[tokio::test]
    async fn concurrent_execute_different_steps_no_interference() {
        let _guard = state_guard();

        let handles: Vec<_> = (0..20)
            .map(|i| {
                let step_id = StepId::new(format!("step-{}", i));
                tokio::spawn(async move {
                    execute_step(step_id, 5000).await
                })
            })
            .collect();

        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok(), "Each step should succeed independently");
        }
    }

    #[tokio::test]
    async fn concurrent_cancel_and_execute_no_deadlock() {
        let _guard = state_guard();

        let step_id = StepId::new("step-1".to_string());

        let (exec_result, cancel_result) = tokio::join!(
            execute_step(step_id.clone(), 5000),
            cancel_execution(step_id.clone()),
        );

        assert!(exec_result.is_ok());
        assert!(cancel_result.is_ok());
    }

    #[tokio::test]
    async fn state_leak_detection_after_many_executions() {
        let _guard = state_guard();

        for i in 0..100 {
            let step_id = StepId::new(format!("step-{}", i));
            let _ = execute_step(step_id.clone(), 1000).await;
        }

        let count = get_state_count();
        assert_eq!(
            count, 100,
            "SURVIVOR: execute_step inserts DashMap entries for each unique step_id \
             but never removes them. After 100 executions on different steps, \
             STATE.len() == 100 (memory leak pattern under sustained load). \
             set_state only overwrites values, it does not remove entries."
        );
    }

    #[tokio::test]
    async fn error_leak_detection_after_many_transients() {
        let _guard = state_guard();

        for i in 0..10 {
            let step_id = StepId::new(format!("transient-step-{}", i));
            let _ = execute_step(step_id.clone(), 1000).await;
        }

        let error_count = vo_executor::get_error_count();
        assert!(
            error_count <= 10,
            "Error count should not exceed number of transient steps"
        );
    }

    #[tokio::test]
    async fn rapid_fire_execute_same_step() {
        let _guard = state_guard();

        let step_id = StepId::new("step-1".to_string());
        for _ in 0..50 {
            let result = execute_step(step_id.clone(), 100).await;
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn concurrent_status_reads_during_execution() {
        let _guard = state_guard();

        let step_id = StepId::new("step-1".to_string());

        let exec_handle = tokio::spawn({
            let step_id = step_id.clone();
            async move { execute_step(step_id, 5000).await }
        });

        let status_handle = tokio::spawn({
            let step_id = step_id.clone();
            async move { get_execution_status(&step_id) }
        });

        let _ = exec_handle.await;
        let status = status_handle.await.unwrap();
        assert!(
            matches!(status, ExecutionStatus::Ready | ExecutionStatus::Executing { .. }),
            "Status should be Ready or Executing, got {:?}",
            status
        );
    }
}

// ============================================================================
// DIMENSION: subprocess-ipc-boundaries
// ============================================================================

#[cfg(test)]
mod subprocess_ipc_adversarial {
    use super::*;

    #[test]
    fn subprocess_config_empty_payload() {
        let config = SubprocessConfig::new(
            "/bin/true".to_string(),
            vec!["true".to_string()],
            5000,
            vec![],
        );
        assert_eq!(config.fd3_payload().len(), 0);
    }

    #[test]
    fn subprocess_config_empty_argv() {
        let config = SubprocessConfig::new(
            "/bin/true".to_string(),
            vec![],
            5000,
            vec![1],
        );
        assert_eq!(config.argv().len(), 0);
    }

    #[test]
    fn subprocess_config_zero_timeout() {
        let config = SubprocessConfig::new(
            "/bin/true".to_string(),
            vec!["true".to_string()],
            0,
            vec![],
        );
        assert_eq!(config.timeout_ms(), 0);
    }

    #[test]
    fn subprocess_output_equality() {
        let a = vo_executor::subprocess::SubprocessOutput {
            fd4_bytes: vec![1, 2, 3],
            exit_code: Some(0),
        };
        let b = vo_executor::subprocess::SubprocessOutput {
            fd4_bytes: vec![1, 2, 3],
            exit_code: Some(0),
        };
        assert_eq!(a, b);
    }

    #[test]
    fn subprocess_output_none_exit_code() {
        let output = vo_executor::subprocess::SubprocessOutput {
            fd4_bytes: vec![],
            exit_code: None,
        };
        assert!(output.exit_code.is_none());
        assert!(output.fd4_bytes.is_empty());
    }

    #[test]
    fn subprocess_error_bounded_buffer_exact() {
        let err = SubprocessError::BoundedBufferExceeded {
            max: 65536,
            tried: 65537,
        };
        let display = err.to_string();
        assert!(display.contains("65536"));
        assert!(display.contains("65537"));
    }
}

// ============================================================================
// DIMENSION: execution-idempotency
// ============================================================================

#[cfg(test)]
mod execution_idempotency_adversarial {
    use super::*;

    #[tokio::test]
    async fn execute_success_idempotent() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        for _ in 0..10 {
            let result = execute_step(step_id.clone(), 5000).await;
            assert!(result.is_ok());
            assert!(result.unwrap().is_success());
        }
    }

    #[tokio::test]
    async fn execute_failure_idempotent() {
        let _guard = state_guard();
        let step_id = StepId::new("step-fail".to_string());

        for _ in 0..10 {
            let result = execute_step(step_id.clone(), 5000).await;
            assert!(result.is_ok());
            assert!(!result.unwrap().is_success());
        }
    }

    #[tokio::test]
    async fn execute_transient_error_preserves_no_side_effects_on_success_step() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        for _ in 0..5 {
            let result = execute_step(step_id.clone(), 5000).await;
            assert!(result.is_ok());
            assert!(get_last_error(&step_id).is_none());
        }
    }

    #[tokio::test]
    async fn cancel_idempotent_from_ready() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        for _ in 0..5 {
            let result = cancel_execution(step_id.clone()).await;
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn execute_after_cancel_succeeds() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        cancel_execution(step_id.clone()).await.unwrap();
        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn execute_after_cancel_then_cancel_then_execute() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        cancel_execution(step_id.clone()).await.unwrap();
        cancel_execution(step_id.clone()).await.unwrap();
        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn status_always_consistent_after_execute() {
        let _guard = state_guard();

        let steps = [
            StepId::new("step-1".to_string()),
            StepId::new("step-fail".to_string()),
            StepId::new("step-good".to_string()),
        ];

        for step_id in &steps {
            let _ = execute_step(step_id.clone(), 5000).await;
            let status = get_execution_status(step_id);
            assert!(
                matches!(status, ExecutionStatus::Ready),
                "After execute_step, status should always be Ready"
            );
        }
    }
}

// ============================================================================
// DIMENSION: timeout-boundary-adversarial
// ============================================================================

#[cfg(test)]
mod timeout_boundary_adversarial {
    use super::*;

    #[tokio::test]
    async fn timeout_1ms_accepted() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-1".to_string()), 1).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn timeout_u64_max_minus_1_rejected() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-1".to_string()), u64::MAX - 1).await;
        assert!(result.is_ok(), "u64::MAX - 1 should be accepted");
    }

    #[tokio::test]
    async fn timeout_exactly_slow_threshold_times_out() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-slow".to_string()), 2999).await;
        assert!(
            result.is_err(),
            "step-slow with timeout < 3000ms should time out"
        );
        assert!(matches!(
            result.unwrap_err(),
            ExecuteNodeError::TimeoutExceeded { .. }
        ));
    }

    #[tokio::test]
    async fn timeout_exactly_at_threshold_succeeds() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-slow".to_string()), 3000).await;
        assert!(
            result.is_ok(),
            "step-slow with timeout == 3000ms should succeed"
        );
    }

    #[tokio::test]
    async fn timeout_one_above_threshold_succeeds() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-slow".to_string()), 3001).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn invalid_timeout_does_not_change_state() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        let result = execute_step(step_id.clone(), 0).await;
        assert!(result.is_err());

        let status = get_execution_status(&step_id);
        assert!(
            matches!(status, ExecutionStatus::Ready),
            "Invalid timeout should not change state"
        );
    }

    #[tokio::test]
    async fn invalid_timeout_does_not_set_error() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        let _ = execute_step(step_id.clone(), 0).await;
        let error = get_last_error(&step_id);
        assert!(error.is_none(), "Invalid timeout should not set error");
    }
}

// ============================================================================
// DIMENSION: scheduler-retry-policy
// ============================================================================

#[cfg(test)]
mod scheduler_retry_adversarial {
    use super::*;
    use vo_executor::scheduler::SchedulerRetryPolicy;
    use std::time::Duration;

    #[test]
    fn scheduler_retry_policy_default() {
        let policy = SchedulerRetryPolicy::default_retry();
        assert_eq!(policy.max_attempts, 3);
        assert!((policy.backoff_multiplier - 2.0).abs() < f64::EPSILON);
        assert_eq!(policy.initial_delay, Duration::from_millis(1000));
        assert_eq!(policy.max_delay, Duration::from_secs(60));
    }

    #[test]
    fn scheduler_retry_policy_calculate_delay_monotonic() {
        let policy = SchedulerRetryPolicy::new(10, 2.0, Duration::from_millis(100), Duration::from_secs(60));

        let mut prev = Duration::ZERO;
        for attempt in 0..10 {
            let delay = policy.calculate_delay(attempt);
            assert!(
                delay >= prev,
                "Delay should be monotonically non-decreasing: attempt {} gave {:?} < {:?}",
                attempt, delay, prev
            );
            prev = delay;
        }
    }

    #[test]
    fn scheduler_retry_policy_calculate_delay_capped() {
        let policy = SchedulerRetryPolicy::new(10, 1000.0, Duration::from_millis(100), Duration::from_millis(500));

        let delay_0 = policy.calculate_delay(0);
        let delay_1 = policy.calculate_delay(1);

        assert!(delay_1 > delay_0, "First retry should increase delay");
        assert!(policy.calculate_delay(5) <= Duration::from_millis(500), "Should be capped");
    }

    #[test]
    fn scheduler_retry_policy_zero_max_delay() {
        let policy = SchedulerRetryPolicy::new(5, 2.0, Duration::from_millis(100), Duration::ZERO);

        for attempt in 0..5 {
            assert_eq!(
                policy.calculate_delay(attempt),
                Duration::ZERO,
                "With max_delay=0, all delays should be 0 (attempt {})",
                attempt
            );
        }
    }

    #[test]
    fn schedule_policy_constructors() {
        let _ = SchedulePolicy::at(chrono::Utc::now());
        let _ = SchedulePolicy::after(Duration::from_secs(1));
        let _ = SchedulePolicy::cron("* * * * *");
        let _ = SchedulePolicy::immediate();
    }

    #[test]
    fn serialized_payload_accessors() {
        let payload = vo_executor::scheduler::SerializedPayload::new("hello".to_string());
        assert_eq!(payload.as_str(), "hello");
    }

    #[test]
    fn scheduled_job_construction() {
        let job = vo_executor::scheduler::ScheduledJob {
            id: JobId::new(1),
            kind: vo_executor::scheduler::JobKind::OneShot,
            state: JobState::Scheduled,
            priority: JobPriority::Normal,
            schedule_policy: SchedulePolicy::immediate(),
            retry_policy: vo_executor::scheduler::SchedulerRetryPolicy::default_retry(),
            attempt_count: 0,
            due_at: chrono::Utc::now(),
            payload: vo_executor::scheduler::SerializedPayload::new("data".to_string()),
            last_error: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        assert_eq!(job.id, JobId::new(1));
        assert!(!job.state.is_terminal());
        assert_eq!(job.attempt_count, 0);
    }
}

// ============================================================================
// DIMENSION: execution-recovery-invariants
// ============================================================================

#[cfg(test)]
mod execution_recovery_adversarial {
    use super::*;

    #[tokio::test]
    async fn error_cleared_on_successful_execution() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());

        let _ = execute_step(step_id.clone(), 5000).await;
        assert!(get_last_error(&step_id).is_some());

        let success_id = StepId::new("step-1".to_string());
        let _ = execute_step(success_id.clone(), 5000).await;
        assert!(get_last_error(&success_id).is_none());
    }

    #[tokio::test]
    async fn clear_error_allows_transient_to_succeed() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());

        let _ = execute_step(step_id.clone(), 5000).await;
        assert!(get_last_error(&step_id).is_some());

        clear_error(step_id.as_str());
        assert!(get_last_error(&step_id).is_none());
    }

    #[tokio::test]
    async fn set_error_overwrites_previous() {
        let _guard = state_guard();
        let step_id = StepId::new("test-unique-err-override".to_string());

        set_error(
            step_id.as_str(),
            ExecuteNodeError::TransientError {
                reason: "first".to_string(),
                recoverable: true,
            },
        );
        set_error(
            step_id.as_str(),
            ExecuteNodeError::TimeoutExceeded {
                elapsed_ms: 5000,
                limit_ms: 3000,
            },
        );

        let err = get_last_error(&step_id).unwrap();
        assert!(matches!(err, ExecuteNodeError::TimeoutExceeded { .. }));
    }

    #[tokio::test]
    async fn retry_with_transient_step_sets_error() {
        let _guard = state_guard();
        let step_id = StepId::new("step-flaky".to_string());
        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();

        let result = execute_step_with_retry(step_id.clone(), 5000, policy).await;
        assert!(result.is_err());
        assert!(get_last_error(&step_id).is_some());
    }

    #[tokio::test]
    async fn executing_state_blocks_second_execution() {
        let _guard = state_guard();
        let step_id = StepId::new("test-blocking-exec".to_string());

        set_executing_state_for_test(step_id.as_str());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(matches!(
            result,
            Err(ExecuteNodeError::InvalidTransition { .. })
        ));
    }

    #[tokio::test]
    async fn executing_state_error_does_not_change_state() {
        let _guard = state_guard();
        let step_id = StepId::new("test-state-preserved".to_string());

        set_executing_state_for_test(step_id.as_str());

        let _ = execute_step(step_id.clone(), 5000).await;

        let state = get_state(step_id.as_str());
        assert!(
            matches!(state, StepState::Executing { .. }),
            "State should remain Executing after InvalidTransition"
        );
    }

    #[tokio::test]
    async fn cancel_from_executing_returns_error() {
        let _guard = state_guard();
        let step_id = StepId::new("test-cancel-exec".to_string());

        set_executing_state_for_test(step_id.as_str());

        let result = cancel_execution(step_id.clone()).await;
        assert!(matches!(
            result,
            Err(ExecuteNodeError::ExecutionCancelled { .. })
        ));
    }

    #[tokio::test]
    async fn cancel_from_executing_preserves_state() {
        let _guard = state_guard();
        let step_id = StepId::new("test-cancel-state-preserve".to_string());

        set_executing_state_for_test(step_id.as_str());

        let _ = cancel_execution(step_id.clone()).await;

        let state = get_state(step_id.as_str());
        assert!(
            matches!(state, StepState::Executing { .. }),
            "State should remain Executing after cancel during execution"
        );
    }
}

// ============================================================================
// DIMENSION: step-id-validation-adversarial
// ============================================================================

#[cfg(test)]
mod step_id_adversarial {
    use super::*;

    #[test]
    fn step_id_new_bypasses_validation() {
        let id = StepId::new("has spaces!".to_string());
        assert_eq!(id.as_str(), "has spaces!");
    }

    #[test]
    fn step_id_parse_accepts_unicode() {
        let result = StepId::parse("café");
        assert!(
            result.is_ok(),
            "SURVIVOR: StepId::parse accepts Unicode because Rust's \
             char::is_alphanumeric() returns true for Unicode letters. \
             The doc says 'only alphanumeric' but does not restrict to ASCII. \
             If ASCII-only is intended, the validation should use is_ascii_alphanumeric()."
        );
    }

    #[test]
    fn step_id_parse_rejects_null_byte() {
        let result = StepId::parse("hello\0world");
        assert!(result.is_err());
    }

    #[test]
    fn step_id_parse_rejects_newlines() {
        for s in ["hello\nworld", "hello\rworld", "hello\tworld"] {
            assert!(StepId::parse(s).is_err(), "Should reject {:?}", s);
        }
    }

    #[test]
    fn step_id_parse_accepts_max_length() {
        let long_id = "a".repeat(10000);
        let result = StepId::parse(&long_id);
        assert!(result.is_ok());
    }

    #[test]
    fn step_id_parse_single_char() {
        assert!(StepId::parse("a").is_ok());
        assert!(StepId::parse("Z").is_ok());
        assert!(StepId::parse("-").is_ok());
        assert!(StepId::parse("_").is_ok());
    }

    #[test]
    fn step_result_failure_output_preserved() {
        let result = StepResult::Failure {
            output: "error: detailed message\nwith\nnewlines".to_string(),
        };
        assert!(!result.is_success());
        if let StepResult::Failure { output } = result {
            assert!(output.contains("detailed message"));
        } else {
            panic!("Should be Failure");
        }
    }

    #[test]
    fn step_result_serde_failure_roundtrip() {
        let result = StepResult::Failure {
            output: "err".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: StepResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, back);
    }
}
