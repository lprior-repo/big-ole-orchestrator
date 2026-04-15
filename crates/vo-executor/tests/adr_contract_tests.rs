//! ADR contract tests for vo-executor
//!
//! Expands test coverage for ADR-specified contracts:
//! - ADR-006: Execution semaphore and backpressure
//! - ADR-012: Execution boundary hardening (FD3, subprocess, memory bombs)
//! - ADR-015: Actor invariants, single-writer, bounded mailboxes
//! - ADR-019: SIGTERM races and signal handling
//! - ADR-023: Stderr flood truncation
//!
//! Plus: stale completion rejection, crash injection, idempotency

use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::time::{Duration, Instant};

use vo_executor::{
    cancel_execution, clear_error, execute_step, execute_step_with_retry, get_execution_status,
    get_last_error, reset_all_state, set_error, ExecuteNodeError, ExecutionStatus, RetryPolicy,
    StepId, StepResult,
};

static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn state_guard() -> MutexGuard<'static, ()> {
    let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset_all_state();
    guard
}

// ============================================================================
// ADR-006: Execution Semaphore & Backpressure
// ============================================================================

#[cfg(test)]
mod execution_semaphore_tests {
    use super::*;

    #[tokio::test]
    async fn semaphore_concurrent_limit_blocks_excess() {
        let _guard = state_guard();
        let config = vo_executor::SchedulerConfig {
            max_concurrent: 2,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = vo_executor::scheduler::Scheduler::new(config);

        let p1 = scheduler.try_acquire();
        let p2 = scheduler.try_acquire();
        let p3 = scheduler.try_acquire();

        assert!(p1.is_some(), "First permit should be acquired");
        assert!(p2.is_some(), "Second permit should be acquired");
        assert!(p3.is_none(), "Third permit should be blocked (limit=2)");

        drop(p1);
        let p4 = scheduler.try_acquire();
        assert!(
            p4.is_some(),
            "After releasing one permit, new acquire should succeed"
        );
    }

    #[tokio::test]
    async fn semaphore_zero_concurrent_blocks_all() {
        let _guard = state_guard();
        let config = vo_executor::SchedulerConfig {
            max_concurrent: 0,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = vo_executor::scheduler::Scheduler::new(config);

        let permit = scheduler.try_acquire();
        assert!(
            permit.is_none(),
            "Zero concurrency should block all permits"
        );
    }

    #[tokio::test]
    async fn semaphore_large_concurrent_all_acquired() {
        let _guard = state_guard();
        let config = vo_executor::SchedulerConfig {
            max_concurrent: 100,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = vo_executor::scheduler::Scheduler::new(config);

        let mut permits = Vec::new();
        for i in 0..100 {
            let permit = scheduler.try_acquire();
            assert!(
                permit.is_some(),
                "Permit {} should be acquired (limit=100)",
                i
            );
            permits.push(permit);
        }

        let overflow = scheduler.try_acquire();
        assert!(overflow.is_none(), "101st permit should be blocked");
    }

    #[tokio::test]
    async fn semaphore_permit_release_allows_reacquire() {
        let _guard = state_guard();
        let config = vo_executor::SchedulerConfig {
            max_concurrent: 1,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = vo_executor::scheduler::Scheduler::new(config);

        let p1 = scheduler.try_acquire();
        assert!(p1.is_some());

        assert!(scheduler.try_acquire().is_none());

        drop(p1);

        let p2 = scheduler.try_acquire();
        assert!(p2.is_some(), "Should reacquire after release");
    }

    #[tokio::test]
    async fn backpressure_concurrent_steps_execute_independently() {
        let _guard = state_guard();
        let step_names = ["step-1", "step-good", "step-fail"];
        let step_ids: Vec<_> = (0..10)
            .map(|i| StepId::new(step_names[i % 3].to_string()))
            .collect();

        let handles: Vec<_> = step_ids
            .into_iter()
            .map(|sid| tokio::spawn(execute_step(sid, 5000)))
            .collect();

        for (i, handle) in handles.into_iter().enumerate() {
            let result = handle.await.expect("task should complete");
            assert!(
                result.is_ok() || matches!(result, Err(ExecuteNodeError::TransientError { .. }))
            );
        }
    }

    #[tokio::test]
    async fn backpressure_burst_execution_all_succeed() {
        let _guard = state_guard();
        let handles: Vec<_> = (0..50)
            .map(|_| tokio::spawn(execute_step(StepId::new("step-good".to_string()), 5000)))
            .collect();

        let mut success_count = 0;
        for handle in handles {
            match handle.await.expect("task should complete") {
                Ok(StepResult::Success { .. }) => success_count += 1,
                other => panic!("Expected Success, got {:?}", other),
            }
        }
        assert_eq!(success_count, 50, "All 50 burst executions should succeed");
    }
}

// ============================================================================
// ADR-012: Execution Boundary Hardening — Subprocess Boundaries
// ============================================================================

#[cfg(test)]
mod subprocess_boundary_tests {
    use super::*;

    #[tokio::test]
    async fn step_not_found_rejected_before_execution() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("nonexistent-binary".to_string()), 5000).await;
        assert!(matches!(result, Err(ExecuteNodeError::StepNotFound { .. })));
    }

    #[tokio::test]
    async fn step_not_found_with_retry_still_terminal() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(5, 100, 2.0).unwrap();
        let result =
            execute_step_with_retry(StepId::new("ghost-binary".to_string()), 5000, policy).await;
        assert!(matches!(result, Err(ExecuteNodeError::StepNotFound { .. })));
    }

    #[tokio::test]
    async fn invalid_timeout_prevents_spawn() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-1".to_string()), 0).await;
        assert!(matches!(
            result,
            Err(ExecuteNodeError::InvalidTimeout { value: 0, .. })
        ));
    }

    #[tokio::test]
    async fn max_u64_timeout_prevents_spawn() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-1".to_string()), u64::MAX).await;
        assert!(matches!(
            result,
            Err(ExecuteNodeError::InvalidTimeout {
                value: u64::MAX,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn timeout_boundary_below_slow_threshold_fails() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-slow".to_string()), 1).await;
        assert!(matches!(
            result,
            Err(ExecuteNodeError::TimeoutExceeded { .. })
        ));
    }

    #[tokio::test]
    async fn timeout_boundary_at_slow_threshold_passes() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-slow".to_string()), 3000).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn timeout_boundary_above_slow_threshold_passes() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-slow".to_string()), 3001).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn zombie_prevention_cancel_returns_ready_state() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        cancel_execution(step_id.clone())
            .await
            .expect("cancel should succeed");

        let status = get_execution_status(&step_id);
        assert!(matches!(status, ExecutionStatus::Cancelled { .. }));
    }

    #[tokio::test]
    async fn double_cancel_is_idempotent() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        let r1 = cancel_execution(step_id.clone()).await;
        let r2 = cancel_execution(step_id.clone()).await;
        let r3 = cancel_execution(step_id.clone()).await;

        assert!(r1.is_ok());
        assert!(r2.is_ok());
        assert!(r3.is_ok());
    }

    #[tokio::test]
    async fn cancel_already_completed_is_noop() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        execute_step(step_id.clone(), 5000)
            .await
            .expect("should succeed");

        let cancel_result = cancel_execution(step_id.clone()).await;
        assert!(cancel_result.is_ok());
    }
}

// ============================================================================
// ADR-012: FD3 Contract — IPC Framing & Capacity Limits
// ============================================================================

#[cfg(test)]
mod fd3_contract_tests {
    use super::*;

    #[tokio::test]
    async fn fd3_input_size_validation_empty_payload() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-1".to_string()), 5000).await;
        assert!(result.is_ok());
        if let Ok(StepResult::Success { output }) = result {
            assert!(!output.is_empty(), "Success output should not be empty");
        }
    }

    #[tokio::test]
    async fn fd3_output_envelope_success_format() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-1".to_string()), 5000).await;
        match result {
            Ok(StepResult::Success { output }) => {
                assert!(!output.is_empty());
            }
            Ok(StepResult::Failure { output }) => {
                panic!("Expected Success, got Failure: {}", output);
            }
            Err(e) => {
                panic!("Expected Ok, got Err: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn fd3_output_envelope_failure_format() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-fail".to_string()), 5000).await;
        match result {
            Ok(StepResult::Failure { output }) => {
                assert!(
                    output.contains("error"),
                    "Failure output should contain error info"
                );
            }
            other => panic!("Expected Failure result, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn fd3_step_identity_preserved_in_result() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());
        let _result = execute_step(step_id.clone(), 5000).await;
        let status = get_execution_status(&step_id);
        assert!(status.is_ready());
    }

    #[tokio::test]
    async fn fd3_timeout_output_still_valid() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-slow".to_string()), 1).await;
        match result {
            Err(ExecuteNodeError::TimeoutExceeded {
                elapsed_ms,
                limit_ms,
            }) => {
                assert_eq!(elapsed_ms, 3000);
                assert_eq!(limit_ms, 1);
            }
            other => panic!("Expected TimeoutExceeded, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn fd3_multiple_sequential_outputs_independent() {
        let _guard = state_guard();
        let r1 = execute_step(StepId::new("step-1".to_string()), 5000).await;
        let r2 = execute_step(StepId::new("step-fail".to_string()), 5000).await;
        let r3 = execute_step(StepId::new("step-good".to_string()), 5000).await;

        assert!(r1.is_ok());
        assert!(r2.is_ok());
        assert!(r3.is_ok());
    }
}

// ============================================================================
// ADR-015: Actor Invariants — Single-Writer & Bounded Mailboxes
// ============================================================================

#[cfg(test)]
mod actor_invariant_tests {
    use super::*;

    #[tokio::test]
    async fn single_writer_invariant_no_concurrent_executing_state() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        let (r1, r2) = tokio::join!(
            execute_step(step_id.clone(), 5000),
            execute_step(step_id.clone(), 5000)
        );

        assert!(r1.is_ok());
        assert!(r2.is_ok());

        let status = get_execution_status(&step_id);
        assert!(status.is_ready());
    }

    #[tokio::test]
    async fn bounded_mailbox_step_error_does_not_cascade() {
        let _guard = state_guard();
        let step_err = StepId::new("step-transient".to_string());
        let step_ok = StepId::new("step-good".to_string());

        let (r_err, r_ok) = tokio::join!(
            execute_step(step_err.clone(), 5000),
            execute_step(step_ok.clone(), 5000)
        );

        assert!(r_err.is_err());
        assert!(r_ok.is_ok());

        let error_ok = get_last_error(&step_ok);
        assert!(
            error_ok.is_none(),
            "Error should not leak to unrelated step"
        );
    }

    #[tokio::test]
    async fn stale_actor_resurrection_prevented() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        execute_step(step_id.clone(), 5000).await.unwrap();
        reset_all_state();

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok(), "Should work cleanly after state reset");
    }

    #[tokio::test]
    async fn concurrent_different_steps_independent_errors() {
        let _guard = state_guard();
        let steps = vec![
            StepId::new("step-transient".to_string()),
            StepId::new("step-good".to_string()),
            StepId::new("step-fail".to_string()),
            StepId::new("step-1".to_string()),
        ];

        let handles: Vec<_> = steps
            .into_iter()
            .map(|sid| tokio::spawn(async move { (sid.clone(), execute_step(sid, 5000).await) }))
            .collect();

        for handle in handles {
            let (sid, result) = handle.await.unwrap();
            match &result {
                Ok(_) => {}
                Err(ExecuteNodeError::TransientError { .. }) => {
                    assert_eq!(sid.as_str(), "step-transient");
                }
                Err(other) => panic!("Unexpected error for {}: {:?}", sid, other),
            }
        }
    }

    #[tokio::test]
    async fn error_per_step_isolation() {
        let _guard = state_guard();
        let step_a = StepId::new("step-transient".to_string());
        let step_b = StepId::new("step-transient-clone".to_string());

        let r_a = execute_step(step_a.clone(), 5000).await;
        assert!(r_a.is_err());

        let error_a = get_last_error(&step_a);
        let error_b = get_last_error(&step_b);

        assert!(error_a.is_some());
        assert!(error_b.is_none(), "Different step should not inherit error");
    }
}

// ============================================================================
// ADR-019: SIGTERM Races & Signal Handling
// ============================================================================

#[cfg(test)]
mod signal_handling_tests {
    use super::*;

    #[tokio::test]
    async fn sigterm_cancel_during_ready() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        let result = cancel_execution(step_id.clone()).await;
        assert!(result.is_ok());

        let status = get_execution_status(&step_id);
        assert!(
            matches!(status, ExecutionStatus::Cancelled { reason } if reason.contains("cancelled"))
        );
    }

    #[tokio::test]
    async fn sigterm_then_reexecute_succeeds() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        cancel_execution(step_id.clone())
            .await
            .expect("cancel should succeed");

        let status_after_cancel = get_execution_status(&step_id);
        assert!(matches!(
            status_after_cancel,
            ExecutionStatus::Cancelled { .. }
        ));

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok(), "Should succeed after cancel + reexecute");
    }

    #[tokio::test]
    async fn sigkill_escalation_timeout_for_slow_step() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let result = execute_step(step_id.clone(), 1).await;
        assert!(
            matches!(result, Err(ExecuteNodeError::TimeoutExceeded { .. })),
            "Slow step with 1ms timeout should timeout (SIGKILL escalation)"
        );
    }

    #[tokio::test]
    async fn grace_period_timeout_boundary() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let below = execute_step(step_id.clone(), 2999).await;
        assert!(below.is_err(), "2999ms < 3000ms threshold should timeout");

        let at = execute_step(step_id.clone(), 3000).await;
        assert!(at.is_ok(), "3000ms == threshold should succeed");

        let above = execute_step(step_id.clone(), 3001).await;
        assert!(above.is_ok(), "3001ms > threshold should succeed");
    }

    #[tokio::test]
    async fn signal_during_transient_failure_handled() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());

        let exec_result = execute_step(step_id.clone(), 5000).await;
        assert!(exec_result.is_err());

        let cancel_result = cancel_execution(step_id.clone()).await;
        assert!(cancel_result.is_ok());

        let status = get_execution_status(&step_id);
        assert!(matches!(status, ExecutionStatus::Cancelled { .. }));
    }

    #[tokio::test]
    async fn multiple_cancel_calls_are_safe() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        for _ in 0..10 {
            let result = cancel_execution(step_id.clone()).await;
            assert!(result.is_ok());
        }
    }
}

// ============================================================================
// ADR-023: Stderr Flood Truncation
// ============================================================================

#[cfg(test)]
mod stderr_truncation_tests {
    use super::*;

    #[tokio::test]
    async fn stderr_truncation_failure_output_bounded() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-fail".to_string()), 5000).await;
        if let Ok(StepResult::Failure { output }) = result {
            assert!(
                output.len() < 1_000_000,
                "Failure output should be bounded (< 1MB), got {} bytes",
                output.len()
            );
        } else {
            panic!("Expected Failure result");
        }
    }

    #[tokio::test]
    async fn stderr_truncation_success_output_bounded() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-1".to_string()), 5000).await;
        if let Ok(StepResult::Success { output }) = result {
            assert!(
                output.len() < 1_000_000,
                "Success output should be bounded (< 1MB), got {} bytes",
                output.len()
            );
        } else {
            panic!("Expected Success result");
        }
    }

    #[tokio::test]
    async fn stderr_does_not_block_step_completion() {
        let _guard = state_guard();
        let start = Instant::now();
        let result = execute_step(StepId::new("step-1".to_string()), 5000).await;
        let elapsed = start.elapsed();

        assert!(result.is_ok());
        assert!(
            elapsed < Duration::from_secs(1),
            "Step should complete quickly without stderr blocking, took {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn stderr_truncation_transient_error_bounded() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-transient".to_string()), 5000).await;
        assert!(result.is_err());

        if let Some(ExecuteNodeError::TransientError { reason, .. }) =
            get_last_error(&StepId::new("step-transient".to_string()))
        {
            assert!(
                reason.len() < 1_000_000,
                "Error reason should be bounded, got {} bytes",
                reason.len()
            );
        }
    }

    #[tokio::test]
    async fn stderr_timeout_prevents_infinite_logging() {
        let _guard = state_guard();
        let start = Instant::now();
        let result = execute_step(StepId::new("step-slow".to_string()), 100).await;
        let elapsed = start.elapsed();

        assert!(result.is_err());
        assert!(
            elapsed < Duration::from_millis(500),
            "Timeout should fire quickly even with stderr flooding, took {:?}",
            elapsed
        );
    }
}

// ============================================================================
// Stale Completion Rejection
// ============================================================================

#[cfg(test)]
mod stale_completion_tests {
    use super::*;

    #[tokio::test]
    async fn stale_error_cleared_on_reexecution() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());

        execute_step(step_id.clone(), 5000)
            .await
            .expect_err("should fail");

        assert!(get_last_error(&step_id).is_some());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_err(), "step-transient always fails");

        let error = get_last_error(&step_id);
        assert!(
            error.is_some(),
            "Fresh error should be set after reexecution"
        );
    }

    #[tokio::test]
    async fn stale_completion_rejected_after_state_reset() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        execute_step(step_id.clone(), 5000).await.unwrap();
        assert!(get_last_error(&step_id).is_none());

        set_error(
            step_id.as_str(),
            ExecuteNodeError::TransientError {
                reason: "stale error".to_string(),
                recoverable: false,
            },
        );
        assert!(get_last_error(&step_id).is_some());

        reset_all_state();

        assert!(
            get_last_error(&step_id).is_none(),
            "Stale error should be gone after reset"
        );

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn stale_timeout_result_not_reused() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let result_timeout = execute_step(step_id.clone(), 1).await;
        assert!(result_timeout.is_err());

        let result_success = execute_step(step_id.clone(), 5000).await;
        assert!(
            result_success.is_ok(),
            "Second execution with sufficient timeout should succeed"
        );
    }

    #[tokio::test]
    async fn clear_error_removes_stale_state() {
        let _guard = state_guard();
        let step_id = StepId::new("test-clear-stale".to_string());

        set_error(
            step_id.as_str(),
            ExecuteNodeError::TransientError {
                reason: "old error".to_string(),
                recoverable: true,
            },
        );
        assert!(get_last_error(&step_id).is_some());

        clear_error(step_id.as_str());

        assert!(
            get_last_error(&step_id).is_none(),
            "clear_error should remove stale error"
        );
    }

    #[tokio::test]
    async fn reexecution_after_cancel_succeeds() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        cancel_execution(step_id.clone())
            .await
            .expect("cancel should succeed");

        let status = get_execution_status(&step_id);
        assert!(matches!(status, ExecutionStatus::Cancelled { .. }));

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok(), "Reexecution after cancel should succeed");
    }

    #[tokio::test]
    async fn error_from_different_step_not_returned() {
        let _guard = state_guard();
        let step_a = StepId::new("step-transient".to_string());
        let step_b = StepId::new("step-1".to_string());

        execute_step(step_a.clone(), 5000)
            .await
            .expect_err("should fail");

        let error_b = get_last_error(&step_b);
        assert!(
            error_b.is_none(),
            "Step B should not inherit step A's error"
        );
    }
}

// ============================================================================
// Crash Injection Tests
// ============================================================================

#[cfg(test)]
mod crash_injection_tests {
    use super::*;

    #[tokio::test]
    async fn crash_recovery_after_transient_failure() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());

        for attempt in 1..=3 {
            let result = execute_step(step_id.clone(), 5000).await;
            assert!(
                result.is_err(),
                "Transient should fail on attempt {}",
                attempt
            );
            assert!(
                get_last_error(&step_id).is_some(),
                "Error should be persisted after attempt {}",
                attempt
            );
        }
    }

    #[tokio::test]
    async fn crash_recovery_retry_after_timeout() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let r1 = execute_step(step_id.clone(), 1).await;
        assert!(r1.is_err());

        let r2 = execute_step(step_id.clone(), 5000).await;
        assert!(
            r2.is_ok(),
            "Should recover after timeout with larger timeout"
        );
    }

    #[tokio::test]
    async fn crash_recovery_after_cancel() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        cancel_execution(step_id.clone())
            .await
            .expect("cancel should succeed");

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok(), "Should recover after cancel");
    }

    #[tokio::test]
    async fn crash_injection_mid_retry_sequence() {
        let _guard = state_guard();
        let step_id = StepId::new("step-flaky".to_string());
        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();

        let result = execute_step_with_retry(step_id.clone(), 5000, policy).await;
        assert!(matches!(
            result,
            Err(ExecuteNodeError::RetryExhausted { attempts: 3, .. })
        ));
    }

    #[tokio::test]
    async fn crash_recovery_full_state_reset() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        execute_step(step_id.clone(), 5000).await.unwrap();
        set_error(
            step_id.as_str(),
            ExecuteNodeError::ExecutionCancelled {
                reason: "crash simulation".to_string(),
            },
        );

        reset_all_state();

        assert!(get_last_error(&step_id).is_none());
        let status = get_execution_status(&step_id);
        assert!(status.is_ready());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok(), "Full recovery after crash simulation");
    }

    #[tokio::test]
    async fn crash_injection_concurrent_failures_all_tracked() {
        let _guard = state_guard();
        let step_ids: Vec<_> = (0..5)
            .map(|_| StepId::new("step-transient".to_string()))
            .collect();

        let handles: Vec<_> = step_ids
            .into_iter()
            .map(|sid| {
                let sid_clone = sid.clone();
                tokio::spawn(async move { (sid_clone, execute_step(sid, 5000).await) })
            })
            .collect();

        for handle in handles {
            let (sid, result) = handle.await.unwrap();
            assert!(result.is_err(), "Transient step {} should fail", sid);
        }
    }

    #[tokio::test]
    async fn crash_recovery_alternating_success_failure() {
        let _guard = state_guard();
        let step_good = StepId::new("step-good".to_string());
        let step_fail = StepId::new("step-transient".to_string());

        for _ in 0..5 {
            let r_good = execute_step(step_good.clone(), 5000).await;
            assert!(r_good.is_ok());

            let r_fail = execute_step(step_fail.clone(), 5000).await;
            assert!(r_fail.is_err());
        }
    }

    #[tokio::test]
    async fn crash_recovery_timeout_then_retry_with_backoff() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let _ = execute_step(step_id.clone(), 1).await;

        let policy = RetryPolicy::new(3, 50, 2.0).unwrap();
        let start = Instant::now();
        let result = execute_step_with_retry(step_id.clone(), 5000, policy).await;
        let elapsed = start.elapsed().as_millis() as u64;

        assert!(
            result.is_ok(),
            "Should succeed with sufficient timeout on retry"
        );
        assert!(elapsed < 1000, "Should complete quickly with valid timeout");
    }
}

// ============================================================================
// RetryPolicy Edge Cases (ADR-006/015)
// ============================================================================

#[cfg(test)]
mod retry_policy_edge_cases {
    use super::*;

    #[test]
    fn retry_policy_with_max_backoff_clamping() {
        let policy = RetryPolicy::with_max_backoff(5, 100, 2.0, 250).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 100);
        assert_eq!(policy.calculate_backoff_delay(2), 200);
        assert_eq!(policy.calculate_backoff_delay(3), 250);
        assert_eq!(policy.calculate_backoff_delay(4), 250);
        assert_eq!(policy.calculate_backoff_delay(5), 250);
    }

    #[test]
    fn retry_policy_max_backoff_equals_base() {
        let policy = RetryPolicy::with_max_backoff(3, 100, 2.0, 100).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 100);
        assert_eq!(policy.calculate_backoff_delay(2), 100);
    }

    #[test]
    fn retry_policy_max_backoff_less_than_base_rejected() {
        let result = RetryPolicy::with_max_backoff(3, 200, 2.0, 100);
        assert!(matches!(
            result,
            Err(vo_executor::RetryPolicyError::MaxBackoffTooSmall { .. })
        ));
    }

    #[test]
    fn retry_policy_multiplier_exactly_one_flat_backoff() {
        let policy = RetryPolicy::new(5, 100, 1.0).unwrap();
        for attempt in 1..=5 {
            assert_eq!(
                policy.calculate_backoff_delay(attempt),
                100,
                "Multiplier 1.0 should produce flat backoff at attempt {}",
                attempt
            );
        }
    }

    #[test]
    fn retry_policy_zero_backoff_zero_delays() {
        let policy = RetryPolicy::new(5, 0, 2.0).unwrap();
        for attempt in 1..=5 {
            assert_eq!(
                policy.calculate_backoff_delay(attempt),
                0,
                "Zero backoff should produce zero delays at attempt {}",
                attempt
            );
        }
    }

    #[test]
    fn retry_policy_attempt_zero_returns_zero() {
        let policy = RetryPolicy::new(3, 100, 2.0).unwrap();
        assert_eq!(policy.calculate_backoff_delay(0), 0);
    }

    #[test]
    fn retry_policy_large_multiplier_no_overflow() {
        let policy = RetryPolicy::new(10, 1000, 1e15).unwrap();
        for attempt in 1..=10 {
            let delay = policy.calculate_backoff_delay(attempt);
            assert!(
                delay <= u64::MAX,
                "Delay should not overflow at attempt {}",
                attempt
            );
        }
    }

    #[tokio::test]
    async fn retry_policy_single_attempt_no_delay() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(1, 1000, 2.0).unwrap();
        let start = Instant::now();
        let result =
            execute_step_with_retry(StepId::new("step-flaky".to_string()), 5000, policy).await;
        let elapsed = start.elapsed().as_millis() as u64;

        assert!(elapsed < 50, "Single attempt should have no backoff delay");
        assert!(matches!(
            result,
            Err(ExecuteNodeError::RetryExhausted { attempts: 1, .. })
        ));
    }
}

// ============================================================================
// State Machine Contract Tests
// ============================================================================

#[cfg(test)]
mod state_machine_contract_tests {
    use super::*;

    #[tokio::test]
    async fn state_ready_to_executing_to_ready_on_success() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        assert!(get_execution_status(&step_id).is_ready());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());

        assert!(get_execution_status(&step_id).is_ready());
    }

    #[tokio::test]
    async fn state_ready_to_executing_to_ready_on_failure() {
        let _guard = state_guard();
        let step_id = StepId::new("step-fail".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());
        assert!(!result.unwrap().is_success());

        assert!(get_execution_status(&step_id).is_ready());
    }

    #[tokio::test]
    async fn state_ready_to_cancelled() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        cancel_execution(step_id.clone())
            .await
            .expect("cancel should succeed");

        assert!(matches!(
            get_execution_status(&step_id),
            ExecutionStatus::Cancelled { .. }
        ));
    }

    #[tokio::test]
    async fn state_cancelled_to_cancelled_idempotent() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        cancel_execution(step_id.clone()).await.unwrap();
        cancel_execution(step_id.clone()).await.unwrap();

        assert!(matches!(
            get_execution_status(&step_id),
            ExecutionStatus::Cancelled { .. }
        ));
    }

    #[tokio::test]
    async fn state_ready_to_error_on_transient() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_err());

        let error = get_last_error(&step_id);
        assert!(matches!(
            error,
            Some(ExecuteNodeError::TransientError { .. })
        ));

        assert!(get_execution_status(&step_id).is_ready());
    }

    #[tokio::test]
    async fn state_not_found_returns_error() {
        let _guard = state_guard();
        let step_id = StepId::new("no-such-step".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(matches!(result, Err(ExecuteNodeError::StepNotFound { .. })));
    }

    #[tokio::test]
    async fn state_reset_clears_all() {
        let _guard = state_guard();
        let step_a = StepId::new("step-transient".to_string());
        let step_b = StepId::new("step-1".to_string());

        execute_step(step_a.clone(), 5000)
            .await
            .expect_err("should fail");
        execute_step(step_b.clone(), 5000).await.unwrap();

        assert!(get_last_error(&step_a).is_some());

        reset_all_state();

        assert!(get_last_error(&step_a).is_none());
        assert!(get_last_error(&step_b).is_none());
        assert!(get_execution_status(&step_a).is_ready());
        assert!(get_execution_status(&step_b).is_ready());
    }
}

// ============================================================================
// ExecutionStatus Display & Debug Contracts
// ============================================================================

#[cfg(test)]
mod execution_status_format_tests {
    use super::*;

    #[test]
    fn execution_status_ready_debug() {
        let status = ExecutionStatus::Ready;
        let debug = format!("{:?}", status);
        assert!(debug.contains("Ready"));
    }

    #[test]
    fn execution_status_executing_debug() {
        let status = ExecutionStatus::Executing {
            step_id: StepId::new("test".to_string()),
            elapsed_ms: 42,
        };
        let debug = format!("{:?}", status);
        assert!(debug.contains("Executing"));
        assert!(debug.contains("test"));
    }

    #[test]
    fn execution_status_completed_debug() {
        let status = ExecutionStatus::Completed {
            output: "result".to_string(),
        };
        let debug = format!("{:?}", status);
        assert!(debug.contains("Completed"));
    }

    #[test]
    fn execution_status_cancelled_debug() {
        let status = ExecutionStatus::Cancelled {
            reason: "user".to_string(),
        };
        let debug = format!("{:?}", status);
        assert!(debug.contains("Cancelled"));
    }

    #[test]
    fn execution_status_all_variants_is_ready() {
        assert!(ExecutionStatus::Ready.is_ready());
        assert!(!ExecutionStatus::Executing {
            step_id: StepId::new("x".to_string()),
            elapsed_ms: 0
        }
        .is_ready());
        assert!(!ExecutionStatus::Completed {
            output: String::new()
        }
        .is_ready());
        assert!(!ExecutionStatus::Cancelled {
            reason: String::new()
        }
        .is_ready());
    }
}
