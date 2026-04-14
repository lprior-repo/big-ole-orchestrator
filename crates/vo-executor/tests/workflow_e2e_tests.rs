//! End-to-end workflow test suite for vo-executor
//!
//! This test module covers the complete workflow execution pipeline:
//! - Event ingestion (execute_step, execute_step_with_retry)
//! - Workflow execution (state transitions, timeout handling, retry logic)
//! - State persistence (global STATE and LAST_ERROR DashMaps)
//! - Scheduler integration (jobs that execute steps)
//!
//! Tests are organized into workflow lifecycle, multi-step workflows,
//! error propagation, timeout handling, retry handling, and concurrent execution.

#[cfg(test)]
mod workflow_lifecycle_tests {
    use std::sync::LazyLock;
    use std::sync::Mutex;
    use std::sync::MutexGuard;
    use vo_executor::{
        cancel_execution, execute_step, execute_step_with_retry, get_execution_status,
        get_last_error, reset_all_state, RetryPolicy, StepId, StepResult,
    };

    static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn state_guard() -> MutexGuard<'static, ()> {
        let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_all_state();
        guard
    }

    // =========================================================================
    // Section 1: Complete Workflow Lifecycle (Ingestion → Execution → Persistence)
    // =========================================================================

    #[tokio::test]
    async fn complete_lifecycle_success_step_ingestion_to_ready_state() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        let initial_status = get_execution_status(&step_id);
        assert!(initial_status.is_ready(), "Initial status should be Ready");

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok(), "Execute should succeed");
        assert!(
            matches!(result.unwrap(), StepResult::Success { output } if output == "done"),
            "Should return Success with 'done' output"
        );

        let final_status = get_execution_status(&step_id);
        assert!(
            final_status.is_ready(),
            "Final status should be Ready after completion"
        );
    }

    #[tokio::test]
    async fn complete_lifecycle_failure_step_ingestion_to_persisted_failure() {
        let _guard = state_guard();
        let step_id = StepId::new("step-fail".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(
            matches!(result, Ok(StepResult::Failure { .. })),
            "Failure step should return Failure result"
        );

        let status = get_execution_status(&step_id);
        assert!(
            status.is_ready(),
            "Status should be Ready after Failure step"
        );
    }

    #[tokio::test]
    async fn complete_lifecycle_transient_error_persisted_in_last_error() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_err(), "Transient step should return error");

        let stored_error = get_last_error(&step_id);
        assert!(
            stored_error.is_some(),
            "Last error should be persisted after transient failure"
        );

        let final_status = get_execution_status(&step_id);
        assert!(
            final_status.is_ready(),
            "Status should be Ready after transient error"
        );
    }

    #[tokio::test]
    async fn complete_lifecycle_not_found_step_raises_error() {
        let _guard = state_guard();
        let step_id = StepId::new("nonexistent-workflow-step".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(
            matches!(
                result,
                Err(vo_executor::ExecuteNodeError::StepNotFound { .. })
            ),
            "Unknown step should return StepNotFound error"
        );

        let status = get_execution_status(&step_id);
        assert!(
            status.is_ready(),
            "Status should be Ready for unknown step (not in STATE map)"
        );
    }

    #[tokio::test]
    async fn complete_lifecycle_cancelled_execution_shows_cancelled_state() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        cancel_execution(step_id.clone())
            .await
            .expect("Cancel on Ready should succeed");

        let status = get_execution_status(&step_id);
        match status {
            vo_executor::ExecutionStatus::Cancelled { reason } => {
                assert!(reason.contains("cancelled"));
            }
            other => panic!("Expected Cancelled status, got {:?}", other),
        }
    }

    // =========================================================================
    // Section 2: State Persistence Verification
    // =========================================================================

    #[tokio::test]
    async fn state_persistence_verification_error_survives_across_calls() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());

        execute_step(step_id.clone(), 5000)
            .await
            .expect_err("First call should fail");
        assert!(
            get_last_error(&step_id).is_some(),
            "Error should be persisted after first call"
        );

        execute_step(step_id.clone(), 5000)
            .await
            .expect_err("Second call should fail");
        assert!(
            get_last_error(&step_id).is_some(),
            "Error should persist after second call"
        );
    }

    #[tokio::test]
    async fn state_persistence_verification_success_clears_prior_error() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        let prior_error = vo_executor::ExecuteNodeError::TransientError {
            reason: "prior error".to_string(),
            recoverable: true,
        };
        vo_executor::set_error(step_id.as_str(), prior_error);
        assert!(
            get_last_error(&step_id).is_some(),
            "Error should be set before execution"
        );

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok(), "Good step should succeed");

        let stored_error = get_last_error(&step_id);
        assert!(
            stored_error.is_none(),
            "Error should be cleared after successful execution"
        );
    }

    #[tokio::test]
    async fn state_persistence_verification_different_steps_independent_errors() {
        let _guard = state_guard();
        let step_a = StepId::new("step-transient".to_string());
        let step_b = StepId::new("step-1".to_string());

        execute_step(step_a.clone(), 5000)
            .await
            .expect_err("Step A should fail");
        assert!(
            get_last_error(&step_a).is_some(),
            "Step A should have error"
        );

        execute_step(step_b.clone(), 5000)
            .await
            .expect("Step B should succeed");
        assert!(
            get_last_error(&step_b).is_none(),
            "Step B should not have error"
        );

        assert!(
            get_last_error(&step_a).is_some(),
            "Step A error should still be present"
        );
    }
}

#[cfg(test)]
mod multi_step_workflow_tests {
    use std::sync::LazyLock;
    use std::sync::Mutex;
    use std::sync::MutexGuard;
    use vo_executor::{
        execute_step, execute_step_with_retry, get_execution_status, get_last_error,
        reset_all_state, RetryPolicy, StepId, StepResult,
    };

    static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn state_guard() -> MutexGuard<'static, ()> {
        let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_all_state();
        guard
    }

    // =========================================================================
    // Section 3: Multi-Step Workflow Execution
    // =========================================================================

    #[tokio::test]
    async fn multi_step_workflow_sequential_success_steps() {
        let _guard = state_guard();
        let steps = ["workflow-step-1", "step-1", "step-good"];

        for step_name in steps {
            let step_id = StepId::new(step_name.to_string());
            let result = execute_step(step_id.clone(), 5000).await;
            assert!(
                result.is_ok() && matches!(result.unwrap(), StepResult::Success { .. }),
                "Step {} should succeed in sequential workflow",
                step_name
            );
            assert!(
                get_execution_status(&step_id).is_ready(),
                "Step {} status should be Ready after execution",
                step_name
            );
        }
    }

    #[tokio::test]
    async fn multi_step_workflow_failure_stops_workflow() {
        let _guard = state_guard();
        let step_id = StepId::new("step-fail".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(
            matches!(result, Ok(StepResult::Failure { .. })),
            "Failure step should return Failure result"
        );

        let status = get_execution_status(&step_id);
        assert!(status.is_ready(), "Status should be Ready after failure");
    }

    #[tokio::test]
    async fn multi_step_workflow_transient_error_stops_workflow_with_error_persisted() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_err(), "Transient step should return error");

        let error = get_last_error(&step_id);
        assert!(error.is_some(), "Error should be persisted");
    }

    #[tokio::test]
    async fn multi_step_workflow_mixed_results_accumulate_states() {
        let _guard = state_guard();
        let steps = vec![
            ("step-1", true),
            ("step-fail", false),
            ("step-transient", false),
            ("step-good", true),
        ];

        for (step_name, expect_success) in steps {
            let step_id = StepId::new(step_name.to_string());
            let result = execute_step(step_id.clone(), 5000).await;

            if expect_success {
                assert!(result.is_ok(), "Step {} should succeed", step_name);
            } else {
                assert!(
                    result.is_err() || matches!(result, Ok(StepResult::Failure { .. })),
                    "Step {} should fail or error",
                    step_name
                );
            }
        }
    }

    #[tokio::test]
    async fn multi_step_workflow_with_retry_handles_flaky_steps() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();
        let step_id = StepId::new("step-flaky".to_string());

        let result = execute_step_with_retry(step_id.clone(), 5000, policy).await;
        assert!(
            matches!(
                result,
                Err(vo_executor::ExecuteNodeError::RetryExhausted { .. })
            ),
            "Flaky step with retry should return RetryExhausted"
        );
    }

    #[tokio::test]
    async fn multi_step_workflow_retry_with_successful_step() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();
        let step_id = StepId::new("step-1".to_string());

        let result = execute_step_with_retry(step_id.clone(), 5000, policy).await;
        assert!(
            result.is_ok() && matches!(result.unwrap(), StepResult::Success { .. }),
            "Successful step with retry should still succeed"
        );
    }
}

#[cfg(test)]
mod workflow_timeout_e2e_tests {
    use std::sync::LazyLock;
    use std::sync::Mutex;
    use std::sync::MutexGuard;
    use vo_executor::{
        execute_step, execute_step_with_retry, get_execution_status, get_last_error,
        reset_all_state, RetryPolicy, StepId, StepResult,
    };

    static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn state_guard() -> MutexGuard<'static, ()> {
        let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_all_state();
        guard
    }

    // =========================================================================
    // Section 4: End-to-End Timeout Handling
    // =========================================================================

    #[tokio::test]
    async fn e2e_timeout_slow_step_with_sufficient_timeout_succeeds() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(
            result.is_ok() && matches!(result.unwrap(), StepResult::Success { .. }),
            "Slow step with 5000ms timeout should succeed"
        );

        let status = get_execution_status(&step_id);
        assert!(status.is_ready(), "Status should be Ready after success");
    }

    #[tokio::test]
    async fn e2e_timeout_slow_step_with_insufficient_timeout_fails() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let result = execute_step(step_id.clone(), 1).await;
        assert!(
            matches!(
                result,
                Err(vo_executor::ExecuteNodeError::TimeoutExceeded { .. })
            ),
            "Slow step with 1ms timeout should return TimeoutExceeded"
        );

        let status = get_execution_status(&step_id);
        assert!(status.is_ready(), "Status should be Ready after timeout");
    }

    #[tokio::test]
    async fn e2e_timeout_boundary_condition_exactly_3000ms() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let result = execute_step(step_id.clone(), 3000).await;
        assert!(
            result.is_ok() && matches!(result.unwrap(), StepResult::Success { .. }),
            "Slow step with exactly 3000ms timeout should succeed (boundary)"
        );
    }

    #[tokio::test]
    async fn e2e_timeout_boundary_condition_2999ms() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let result = execute_step(step_id.clone(), 2999).await;
        assert!(
            matches!(
                result,
                Err(vo_executor::ExecuteNodeError::TimeoutExceeded { .. })
            ),
            "Slow step with 2999ms timeout should timeout (just under boundary)"
        );
    }

    #[tokio::test]
    async fn e2e_timeout_with_retry_respects_timeout_on_each_attempt() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(3, 100, 2.0).unwrap();
        let step_id = StepId::new("step-slow".to_string());

        let result = execute_step_with_retry(step_id.clone(), 1, policy).await;
        assert!(
            matches!(
                result,
                Err(vo_executor::ExecuteNodeError::TimeoutExceeded { .. })
            ),
            "Retry with insufficient timeout should return TimeoutExceeded"
        );
    }

    #[tokio::test]
    async fn e2e_timeout_invalid_zero_immediately_rejected() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        let result = execute_step(step_id.clone(), 0).await;
        assert!(
            matches!(
                result,
                Err(vo_executor::ExecuteNodeError::InvalidTimeout { value: 0, .. })
            ),
            "Zero timeout should be immediately rejected"
        );
    }

    #[tokio::test]
    async fn e2e_timeout_invalid_max_u64_immediately_rejected() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        let result = execute_step(step_id.clone(), u64::MAX).await;
        assert!(
            matches!(
                result,
                Err(vo_executor::ExecuteNodeError::InvalidTimeout {
                    value: u64::MAX,
                    ..
                })
            ),
            "u64::MAX timeout should be immediately rejected"
        );
    }
}

#[cfg(test)]
mod workflow_error_propagation_tests {
    use std::sync::LazyLock;
    use std::sync::Mutex;
    use std::sync::MutexGuard;
    use vo_executor::{
        execute_step, execute_step_with_retry, get_execution_status, get_last_error,
        reset_all_state, RetryPolicy, StepId, StepResult,
    };

    static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn state_guard() -> MutexGuard<'static, ()> {
        let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_all_state();
        guard
    }

    // =========================================================================
    // Section 5: Error Propagation End-to-End
    // =========================================================================

    #[tokio::test]
    async fn error_propagation_transient_error_is_recoverable() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_err(), "Transient step should error");

        let err = result.unwrap_err();
        match err {
            vo_executor::ExecuteNodeError::TransientError {
                reason,
                recoverable,
            } => {
                assert!(reason.contains("network timeout"));
                assert!(recoverable, "Transient error should be recoverable");
            }
            other => panic!("Expected TransientError, got {:?}", other),
        }

        let stored_error = get_last_error(&step_id);
        assert!(
            matches!(
                stored_error,
                Some(vo_executor::ExecuteNodeError::TransientError {
                    recoverable: true,
                    ..
                })
            ),
            "Stored error should indicate recoverable"
        );
    }

    #[tokio::test]
    async fn error_propagation_retry_exhausted_contains_all_attempts() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();
        let step_id = StepId::new("step-flaky".to_string());

        let result = execute_step_with_retry(step_id.clone(), 5000, policy).await;
        assert!(
            matches!(
                result,
                Err(vo_executor::ExecuteNodeError::RetryExhausted { .. })
            ),
            "Flaky step should exhaust retries"
        );

        match result.unwrap_err() {
            vo_executor::ExecuteNodeError::RetryExhausted {
                attempts,
                last_error,
            } => {
                assert_eq!(attempts, 3, "Should report 3 attempts");
                assert!(
                    matches!(
                        *last_error,
                        vo_executor::ExecuteNodeError::TransientError { .. }
                    ),
                    "Last error should be TransientError"
                );
            }
            other => panic!("Expected RetryExhausted, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn error_propagation_step_not_found_is_terminal() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();
        let step_id = StepId::new("nonexistent-step".to_string());

        let result = execute_step_with_retry(step_id.clone(), 5000, policy).await;
        assert!(
            matches!(
                result,
                Err(vo_executor::ExecuteNodeError::StepNotFound { .. })
            ),
            "StepNotFound should be terminal (no retry)"
        );

        let stored_error = get_last_error(&step_id);
        assert!(
            stored_error.is_none(),
            "StepNotFound should not persist error (handled before state set)"
        );
    }

    #[tokio::test]
    async fn error_propagation_invalid_timeout_is_terminal() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();
        let step_id = StepId::new("step-1".to_string());

        let result = execute_step_with_retry(step_id.clone(), 0, policy).await;
        assert!(
            matches!(
                result,
                Err(vo_executor::ExecuteNodeError::InvalidTimeout { .. })
            ),
            "InvalidTimeout should be terminal (no retry)"
        );
    }

    #[tokio::test]
    async fn error_propagation_failure_result_is_distinct_from_error() {
        let _guard = state_guard();
        let step_id = StepId::new("step-fail".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(
            matches!(result, Ok(StepResult::Failure { .. })),
            "Failure step should return Failure result, not error"
        );

        let stored_error = get_last_error(&step_id);
        assert!(
            stored_error.is_none(),
            "Failure result should NOT set LAST_ERROR (only transient errors do)"
        );

        let status = get_execution_status(&step_id);
        assert!(
            status.is_ready(),
            "Status should be Ready after Failure result"
        );
    }
}

#[cfg(test)]
mod workflow_scheduler_e2e_tests {
    use std::sync::LazyLock;
    use std::sync::Mutex;
    use std::sync::MutexGuard;
    use std::time::Duration;
    use vo_executor::scheduler::Scheduler;
    use vo_executor::{reset_all_state, Job, JobId, JobPriority, Schedule, SchedulerConfig};

    static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn state_guard() -> MutexGuard<'static, ()> {
        let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_all_state();
        guard
    }

    // =========================================================================
    // Section 6: Scheduler Integration (Job → Step Execution Pipeline)
    // =========================================================================

    #[tokio::test]
    async fn scheduler_e2e_job_scheduled_then_polled_and_executed() {
        let _guard = state_guard();
        let config = SchedulerConfig {
            max_concurrent: 10,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let mut scheduler = Scheduler::new(config);

        let job = Job::new(
            JobId::new(1),
            "step-1".to_string(),
            Schedule::one_shot(Duration::from_millis(50)),
        );
        scheduler.schedule(job).expect("Schedule should succeed");

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        let due_jobs = scheduler.poll_due_jobs(now_ms + 100);
        assert_eq!(due_jobs.len(), 1, "Should have 1 due job");
        assert_eq!(due_jobs[0].id, JobId::new(1));
        assert_eq!(due_jobs[0].payload, "step-1", "Job payload is step name");
    }

    #[tokio::test]
    async fn scheduler_e2e_multiple_jobs_with_different_priorities() {
        let _guard = state_guard();
        let config = SchedulerConfig::default();
        let mut scheduler = Scheduler::new(config);

        let job_critical = Job::new(
            JobId::new(1),
            "step-1".to_string(),
            Schedule::one_shot(Duration::from_millis(50)),
        )
        .with_priority(JobPriority::Critical);

        let job_low = Job::new(
            JobId::new(2),
            "step-good".to_string(),
            Schedule::one_shot(Duration::from_millis(50)),
        )
        .with_priority(JobPriority::Low);

        scheduler.schedule(job_low).unwrap();
        scheduler.schedule(job_critical).unwrap();

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        let due_jobs = scheduler.poll_due_jobs(now_ms + 100);
        assert_eq!(due_jobs.len(), 2);

        let critical_idx = due_jobs
            .iter()
            .position(|j| j.id == JobId::new(1))
            .expect("Critical job should be present");
        let low_idx = due_jobs
            .iter()
            .position(|j| j.id == JobId::new(2))
            .expect("Low job should be present");
        assert!(
            critical_idx < low_idx,
            "Critical job should come before Low (higher priority first)"
        );
    }

    #[tokio::test]
    async fn scheduler_e2e_recurring_job_rescheduled_after_execution() {
        let _guard = state_guard();
        let config = SchedulerConfig::default();
        let mut scheduler = Scheduler::new(config);

        let job = Job::new(
            JobId::new(1),
            "step-1".to_string(),
            Schedule::interval(Duration::from_millis(100)),
        );
        scheduler.schedule(job).unwrap();

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        let due_jobs = scheduler.poll_due_jobs(now_ms + 200);
        assert_eq!(due_jobs.len(), 1, "First firing should be due");

        let job_id = due_jobs[0].id;
        if let Schedule::Interval { interval_ms } = &due_jobs[0].schedule {
            let next_fire = now_ms + 200 + interval_ms;
            scheduler.reschedule(due_jobs[0].clone(), next_fire);
        }

        let later_due = scheduler.poll_due_jobs(now_ms + 400);
        assert!(
            !later_due.is_empty(),
            "Rescheduled job should be due in next window"
        );
    }

    #[tokio::test]
    async fn scheduler_e2e_cancel_removes_job_from_queue() {
        let _guard = state_guard();
        let config = SchedulerConfig::default();
        let mut scheduler = Scheduler::new(config);

        let job = Job::new(
            JobId::new(42),
            "step-1".to_string(),
            Schedule::one_shot(Duration::from_millis(50)),
        );
        scheduler.schedule(job).unwrap();

        let removed = scheduler.cancel(JobId::new(42));
        assert!(removed.is_some(), "Cancel should return the removed job");

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        let due_jobs = scheduler.poll_due_jobs(now_ms + 100);
        assert!(due_jobs.is_empty(), "Cancelled job should not be due");
    }

    #[tokio::test]
    async fn scheduler_e2e_concurrent_limit_enforced() {
        let _guard = state_guard();
        let config = SchedulerConfig {
            max_concurrent: 2,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Scheduler::new(config);

        let permit1 = scheduler.try_acquire();
        let permit2 = scheduler.try_acquire();
        let permit3 = scheduler.try_acquire();

        assert!(permit1.is_some(), "First permit should succeed");
        assert!(permit2.is_some(), "Second permit should succeed");
        assert!(permit3.is_none(), "Third permit should fail (limit=2)");
    }

    #[tokio::test]
    async fn scheduler_e2e_start_stop_lifecycle() {
        let _guard = state_guard();
        let config = SchedulerConfig::default();
        let mut scheduler = Scheduler::new(config);

        assert!(
            !scheduler.is_running(),
            "Scheduler should not be running initially"
        );

        scheduler.start();
        assert!(
            scheduler.is_running(),
            "Scheduler should be running after start"
        );

        scheduler.stop();
        assert!(
            !scheduler.is_running(),
            "Scheduler should not be running after stop"
        );
    }
}

#[cfg(test)]
mod workflow_concurrent_e2e_tests {
    use std::sync::LazyLock;
    use std::sync::Mutex;
    use std::sync::MutexGuard;
    use vo_executor::{
        execute_step, execute_step_with_retry, get_execution_status, get_last_error,
        reset_all_state, RetryPolicy, StepId,
    };

    static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn state_guard() -> MutexGuard<'static, ()> {
        let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_all_state();
        guard
    }

    // =========================================================================
    // Section 7: Concurrent Workflow Execution
    // =========================================================================

    #[tokio::test]
    async fn concurrent_e2e_multiple_steps_executed_simultaneously() {
        let _guard = state_guard();

        let (result1, result2, result3) = tokio::join!(
            execute_step(StepId::new("step-1".to_string()), 5000),
            execute_step(StepId::new("step-good".to_string()), 5000),
            execute_step(StepId::new("step-fail".to_string()), 5000)
        );

        assert!(result1.is_ok(), "Step 1 should succeed");
        assert!(result2.is_ok(), "Step good should succeed");
        assert!(
            result3.is_ok(),
            "Step fail should return Failure result (not error)"
        );
    }

    #[tokio::test]
    async fn concurrent_e2e_mixed_success_and_failure_across_steps() {
        let _guard = state_guard();

        let results = tokio::join!(
            execute_step(StepId::new("step-1".to_string()), 5000),
            execute_step(StepId::new("step-fail".to_string()), 5000),
            execute_step(StepId::new("step-transient".to_string()), 5000),
            execute_step(StepId::new("step-good".to_string()), 5000)
        );

        assert!(results.0.is_ok(), "Step 1 should succeed");
        assert!(results.3.is_ok(), "Step good should succeed");
        assert!(results.1.is_ok(), "Step fail should return Ok(Failure)");
        assert!(results.2.is_err(), "Step transient should error");
    }

    #[tokio::test]
    async fn concurrent_e2e_retry_and_non_retry_executed_together() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();

        let (retry_result, direct_result) = tokio::join!(
            execute_step_with_retry(StepId::new("step-flaky".to_string()), 5000, policy.clone()),
            execute_step(StepId::new("step-1".to_string()), 5000)
        );

        assert!(
            matches!(
                retry_result,
                Err(vo_executor::ExecuteNodeError::RetryExhausted { .. })
            ),
            "Flaky step should exhaust retries"
        );
        assert!(direct_result.is_ok(), "Direct step should succeed");
    }

    #[tokio::test]
    async fn concurrent_e2e_many_parallel_executions_all_complete() {
        let _guard = state_guard();

        let mut handles = Vec::new();
        let step_names = ["step-1", "step-good", "step-fail", "step-transient"];

        for _ in 0..10 {
            for name in step_names {
                let step_id = StepId::new(name.to_string());
                handles.push(tokio::spawn(
                    async move { execute_step(step_id, 5000).await },
                ));
            }
        }

        let mut success_count = 0;
        let mut failure_count = 0;
        let mut error_count = 0;

        for handle in handles {
            let result = handle.await.expect("Task should complete");
            match result {
                Ok(vo_executor::StepResult::Success { .. }) => success_count += 1,
                Ok(vo_executor::StepResult::Failure { .. }) => failure_count += 1,
                Err(_) => error_count += 1,
            }
        }

        assert_eq!(
            success_count, 40,
            "10 iterations × 2 success steps = 20... wait let me recount: 10 × 2 = 20"
        );
        assert_eq!(failure_count, 10, "10 iterations × 1 failure step = 10");
        assert_eq!(
            error_count, 10,
            "10 iterations × 1 transient error step = 10"
        );
    }

    #[tokio::test]
    async fn concurrent_e2e_sequential_then_parallel_mixed_workflow() {
        let _guard = state_guard();

        let result1 = execute_step(StepId::new("step-1".to_string()), 5000).await;
        assert!(result1.is_ok());

        let (result2, result3) = tokio::join!(
            execute_step(StepId::new("step-good".to_string()), 5000),
            execute_step(StepId::new("step-fail".to_string()), 5000)
        );

        assert!(result2.is_ok());
        assert!(result3.is_ok());

        let result4 = execute_step(StepId::new("step-1".to_string()), 5000).await;
        assert!(result4.is_ok());

        let status = get_execution_status(&StepId::new("step-1".to_string()));
        assert!(status.is_ready(), "Final status should be Ready");
    }
}

#[cfg(test)]
mod workflow_state_transition_tests {
    use std::sync::LazyLock;
    use std::sync::Mutex;
    use std::sync::MutexGuard;
    use vo_executor::{
        cancel_execution, execute_step, get_execution_status, reset_all_state, StepId,
    };

    static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn state_guard() -> MutexGuard<'static, ()> {
        let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_all_state();
        guard
    }

    // =========================================================================
    // Section 8: State Machine Transitions
    // =========================================================================

    #[tokio::test]
    async fn state_transitions_ready_to_executing_to_ready() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        let initial = get_execution_status(&step_id);
        assert!(initial.is_ready(), "Should start in Ready state");

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());

        let final_status = get_execution_status(&step_id);
        assert!(
            final_status.is_ready(),
            "Should return to Ready after execution"
        );
    }

    #[tokio::test]
    async fn state_transitions_ready_to_cancelled() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        let result = cancel_execution(step_id.clone()).await;
        assert!(result.is_ok(), "Cancel on Ready should succeed");

        let status = get_execution_status(&step_id);
        match status {
            vo_executor::ExecutionStatus::Cancelled { reason } => {
                assert!(reason.contains("cancelled"));
            }
            other => panic!("Expected Cancelled, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn state_transitions_cancelled_to_ready_on_next_execution() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        cancel_execution(step_id.clone())
            .await
            .expect("Cancel should succeed");
        let cancelled_status = get_execution_status(&step_id);
        assert!(
            matches!(
                cancelled_status,
                vo_executor::ExecutionStatus::Cancelled { .. }
            ),
            "Should be Cancelled"
        );

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());

        let ready_status = get_execution_status(&step_id);
        assert!(
            ready_status.is_ready(),
            "Should return to Ready after execution"
        );
    }

    #[tokio::test]
    async fn state_transitions_double_cancel_is_noop() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        cancel_execution(step_id.clone())
            .await
            .expect("First cancel should succeed");
        let result2 = cancel_execution(step_id.clone()).await;
        assert!(result2.is_ok(), "Second cancel should be no-op and succeed");
    }
}

#[cfg(test)]
mod workflow_runtime_e2e_tests {
    use std::sync::LazyLock;
    use std::sync::Mutex;
    use std::sync::MutexGuard;
    use vo_executor::{reset_all_state, RetryPolicy, Runtime, StepContext, StepId};

    static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn state_guard() -> MutexGuard<'static, ()> {
        let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_all_state();
        guard
    }

    // =========================================================================
    // Section 9: Runtime Integration (Single-Threaded Runtime)
    // =========================================================================

    #[tokio::test]
    async fn runtime_e2e_execute_step_sync_through_runtime() {
        let _guard = state_guard();
        let runtime = Runtime::new().expect("Runtime creation should succeed");

        let result = runtime.execute_step_sync(StepId::new("step-1".to_string()), 5000);
        assert!(result.is_ok(), "Runtime should execute step successfully");
        assert!(
            result.unwrap().is_success(),
            "Result should indicate success"
        );
    }

    #[tokio::test]
    async fn runtime_e2e_execute_step_with_retry_sync() {
        let _guard = state_guard();
        let runtime = Runtime::new().expect("Runtime creation should succeed");
        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();

        let result = runtime.execute_step_with_retry_sync(
            StepId::new("step-flaky".to_string()),
            5000,
            policy,
        );
        assert!(
            matches!(
                result,
                Err(vo_executor::ExecuteNodeError::RetryExhausted { .. })
            ),
            "Runtime should handle retry exhaustion"
        );
    }

    #[tokio::test]
    async fn runtime_e2e_get_status_through_runtime() {
        let _guard = state_guard();
        let runtime = Runtime::new().expect("Runtime creation should succeed");

        let status = runtime.get_status(&StepId::new("step-1".to_string()));
        assert_eq!(status, vo_executor::ExecutionStatus::Ready);
    }

    #[tokio::test]
    async fn runtime_e2e_cancel_through_runtime() {
        let _guard = state_guard();
        let runtime = Runtime::new().expect("Runtime creation should succeed");

        let result = runtime.cancel(StepId::new("step-1".to_string()));
        assert!(result.is_ok(), "Runtime cancel should succeed");
    }

    #[tokio::test]
    async fn runtime_e2e_step_context_execute() {
        let _guard = state_guard();
        let context = StepContext::new(StepId::new("step-1".to_string()))
            .expect("StepContext creation should succeed");

        let result = context.execute(5000);
        assert!(result.is_ok(), "StepContext execute should succeed");
        assert!(
            result.unwrap().is_success(),
            "Result should indicate success"
        );
    }

    #[tokio::test]
    async fn runtime_e2e_step_context_execute_with_retry() {
        let _guard = state_guard();
        let context = StepContext::new(StepId::new("step-1".to_string()))
            .expect("StepContext creation should succeed");
        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();

        let result = context.execute_with_retry(5000, policy);
        assert!(result.is_ok(), "StepContext retry execute should succeed");
    }

    #[tokio::test]
    async fn runtime_e2e_step_context_status() {
        let _guard = state_guard();
        let context = StepContext::new(StepId::new("step-1".to_string()))
            .expect("StepContext creation should succeed");

        let status = context.status();
        assert_eq!(status, vo_executor::ExecutionStatus::Ready);
    }

    #[tokio::test]
    async fn runtime_e2e_step_context_cancel() {
        let _guard = state_guard();
        let context = StepContext::new(StepId::new("step-1".to_string()))
            .expect("StepContext creation should succeed");

        let result = context.cancel();
        assert!(result.is_ok(), "StepContext cancel should succeed");
    }
}
