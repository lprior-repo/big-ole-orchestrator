//! Expanded ADR contract tests for vo-executor
//!
//! Fills coverage gaps identified in test review (ve-t6dm):
//! - ADR-006/015: Semaphore contention, drain/refill, lifecycle
//! - ADR-012: StepId validation, boundary edge cases, transition guards
//! - FD3: Output format contracts, serialization, isolation
//! - ADR-023: Stderr bounds at capacity boundary, accumulation guard
//! - ADR-019: Cancel lifecycle, post-completion cancel, multi-cycle
//! - Stale completion: Concurrent rejection, interleaved reset
//! - Crash injection: Orphaned state, mid-retry crash, error overwrite

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::atomic::AtomicUsize;
use std::time::{Duration, Instant};

use vo_executor::{
    cancel_execution, clear_error, execute_step, execute_step_with_retry,
    get_execution_status, get_last_error, reset_all_state, set_error,
    scheduler::SchedulerConfig, ExecuteNodeError, ExecutionStatus, RetryPolicy,
    StepId, StepResult,
};
use vo_executor::state::set_state;
use vo_executor::state::StepState;

static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn state_guard() -> MutexGuard<'static, ()> {
    let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset_all_state();
    guard
}

// ============================================================================
// ADR-006/015: Semaphore Contention & Lifecycle
// ============================================================================

#[cfg(test)]
mod semaphore_contention_tests {
    use super::*;

    #[tokio::test]
    async fn semaphore_contention_many_tasks_few_permits() {
        let _guard = state_guard();
        let config = SchedulerConfig {
            max_concurrent: 3,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Arc::new(vo_executor::scheduler::Scheduler::new(config));

        let acquired_count = Arc::new(AtomicUsize::new(0));
        let rejected_count = Arc::new(AtomicUsize::new(0));

        let barrier = Arc::new(tokio::sync::Barrier::new(20));
        let holds = Arc::new(Mutex::new(Vec::new()));

        let handles: Vec<_> = (0..20)
            .map(|_| {
                let sched = Arc::clone(&scheduler);
                let acq = Arc::clone(&acquired_count);
                let rej = Arc::clone(&rejected_count);
                let barrier = Arc::clone(&barrier);
                let holds = Arc::clone(&holds);
                tokio::spawn(async move {
                    barrier.wait().await;
                    let permit = sched.try_acquire();
                    if permit.is_some() {
                        acq.fetch_add(1, Ordering::SeqCst);
                        holds.lock().unwrap().push(permit);
                    } else {
                        rej.fetch_add(1, Ordering::SeqCst);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.await.unwrap();
        }

        let acquired = acquired_count.load(Ordering::SeqCst);
        assert!(acquired <= 3, "At most 3 permits should be acquired, got {}", acquired);
        assert!(
            acquired + rejected_count.load(Ordering::SeqCst) == 20,
            "All tasks should be accounted for"
        );
    }

    #[tokio::test]
    async fn semaphore_drain_and_refill() {
        let _guard = state_guard();
        let config = SchedulerConfig {
            max_concurrent: 5,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = vo_executor::scheduler::Scheduler::new(config);

        let mut permits = Vec::new();
        for _ in 0..5 {
            permits.push(scheduler.try_acquire().unwrap());
        }
        assert!(scheduler.try_acquire().is_none());

        for p in permits.drain(..) {
            drop(p);
        }
        drop(permits);

        let mut refilled = Vec::new();
        for _ in 0..5 {
            let p = scheduler.try_acquire();
            assert!(p.is_some(), "Should reacquire after drain");
            refilled.push(p);
        }
        assert!(scheduler.try_acquire().is_none(), "6th acquire should be blocked after refill");
    }

    #[tokio::test]
    async fn scheduler_lifecycle_start_stop_semaphore() {
        let _guard = state_guard();
        let config = SchedulerConfig {
            max_concurrent: 2,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let mut scheduler = vo_executor::scheduler::Scheduler::new(config);

        assert!(!scheduler.is_running());
        scheduler.start();
        assert!(scheduler.is_running());

        let p1 = scheduler.try_acquire();
        let p2 = scheduler.try_acquire();
        assert!(p1.is_some());
        assert!(p2.is_some());
        assert!(scheduler.try_acquire().is_none());

        scheduler.stop();
        assert!(!scheduler.is_running());

        drop(p1);
        drop(p2);

        let p3 = scheduler.try_acquire();
        assert!(p3.is_some(), "Semaphore should still work after stop");
    }

    #[tokio::test]
    async fn semaphore_single_permit_sequential_cycles() {
        let _guard = state_guard();
        let config = SchedulerConfig {
            max_concurrent: 1,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = vo_executor::scheduler::Scheduler::new(config);

        for cycle in 0..50 {
            let permit = scheduler.try_acquire();
            assert!(permit.is_some(), "Cycle {}: should acquire single permit", cycle);
            assert!(scheduler.try_acquire().is_none(), "Cycle {}: second acquire blocked", cycle);
            drop(permit);
        }
    }

    #[tokio::test]
    async fn semaphore_large_burst_pressure() {
        let _guard = state_guard();
        let config = SchedulerConfig {
            max_concurrent: 10,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = vo_executor::scheduler::Scheduler::new(config);

        let mut permits = Vec::with_capacity(10);
        for i in 0..10 {
            let p = scheduler.try_acquire();
            assert!(p.is_some(), "Permit {} should acquire", i);
            permits.push(p);
        }
        assert!(scheduler.try_acquire().is_none());

        for (i, p) in permits.drain(..).enumerate() {
            drop(p);
            if i < 9 {
                assert!(scheduler.try_acquire().is_some(), "Refill at drop {}", i);
            }
        }
    }

    #[tokio::test]
    async fn semaphore_zero_blocks_try_acquire() {
        let _guard = state_guard();
        let config = SchedulerConfig {
            max_concurrent: 0,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = vo_executor::scheduler::Scheduler::new(config);

        for _ in 0..100 {
            assert!(scheduler.try_acquire().is_none());
        }
    }

    #[tokio::test]
    async fn semaphore_default_config_ten_permits() {
        let _guard = state_guard();
        let config = SchedulerConfig::default();
        assert_eq!(config.max_concurrent, 10);

        let scheduler = vo_executor::scheduler::Scheduler::new(config);

        let mut permits = Vec::new();
        for _ in 0..10 {
            permits.push(scheduler.try_acquire().unwrap());
        }
        assert!(scheduler.try_acquire().is_none());
        drop(permits);
    }
}

// ============================================================================
// ADR-012: Subprocess Boundary Expansion
// ============================================================================

#[cfg(test)]
mod subprocess_boundary_expansion_tests {
    use super::*;

    #[tokio::test]
    async fn step_id_parse_empty_rejected() {
        let result = StepId::parse("");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn step_id_parse_special_chars_rejected() {
        for invalid in ["step with spaces", "step/with/slash", "step.with.dot", "step@hash"] {
            let result = StepId::parse(invalid);
            assert!(result.is_err(), "StepId::parse({:?}) should be rejected", invalid);
        }
    }

    #[tokio::test]
    async fn step_id_parse_valid_chars() {
        for valid in ["step-1", "step_2", "Step3", "a-b_c-0", "workflow-step-123"] {
            let result = StepId::parse(valid);
            assert!(result.is_ok(), "StepId::parse({:?}) should succeed", valid);
            assert_eq!(result.unwrap().as_str(), valid);
        }
    }

    #[tokio::test]
    async fn step_id_display_format() {
        let id = StepId::new("my-step".to_string());
        assert_eq!(format!("{}", id), "my-step");
    }

    #[tokio::test]
    async fn timeout_one_ms_minimum_valid() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-good".to_string()), 1).await;
        assert!(result.is_ok(), "1ms timeout is valid (> 0)");
    }

    #[tokio::test]
    async fn timeout_large_value_accepted() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-1".to_string()), u64::MAX - 1).await;
        assert!(
            result.is_ok(),
            "u64::MAX - 1 should be accepted as valid timeout (only 0 and u64::MAX are rejected)"
        );
    }

    #[tokio::test]
    async fn step_not_found_preserves_step_id_in_error() {
        let _guard = state_guard();
        let step_id = StepId::new("ghost-step".to_string());
        let result = execute_step(step_id.clone(), 5000).await;
        match result {
            Err(ExecuteNodeError::StepNotFound { step_id: err_id }) => {
                assert_eq!(err_id, step_id);
            }
            other => panic!("Expected StepNotFound, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn invalid_timeout_preserves_value_in_error() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-1".to_string()), 0).await;
        match result {
            Err(ExecuteNodeError::InvalidTimeout { value, reason }) => {
                assert_eq!(value, 0);
                assert!(reason.contains("> 0ms"));
            }
            other => panic!("Expected InvalidTimeout, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn retry_with_zero_attempts_rejected() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(0, 100, 2.0).unwrap_err();
        assert!(matches!(policy, vo_executor::RetryPolicyError::ZeroAttempts));
    }

    #[tokio::test]
    async fn retry_with_nan_multiplier_rejected() {
        let result = RetryPolicy::new(3, 100, f64::NAN);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn retry_with_infinite_multiplier_rejected() {
        let result = RetryPolicy::new(3, 100, f64::INFINITY);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn retry_with_negative_multiplier_rejected() {
        let result = RetryPolicy::new(3, 100, -1.0);
        assert!(result.is_err());
    }
}

// ============================================================================
// FD3 Contract Expansion
// ============================================================================

#[cfg(test)]
mod fd3_contract_expansion_tests {
    use super::*;

    #[tokio::test]
    async fn fd3_success_output_matches_expected() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-1".to_string()), 5000).await;
        if let Ok(StepResult::Success { output }) = result {
            assert_eq!(output, "done");
        } else {
            panic!("Expected Success with 'done' output");
        }
    }

    #[tokio::test]
    async fn fd3_failure_output_contains_error_code() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-fail".to_string()), 5000).await;
        if let Ok(StepResult::Failure { output }) = result {
            assert!(output.contains("error"));
            assert!(output.contains("exit code 1"));
        } else {
            panic!("Expected Failure result");
        }
    }

    #[tokio::test]
    async fn fd3_concurrent_steps_independent_outputs() {
        let _guard = state_guard();
        let (r1, r2, r3) = tokio::join!(
            execute_step(StepId::new("step-1".to_string()), 5000),
            execute_step(StepId::new("step-fail".to_string()), 5000),
            execute_step(StepId::new("step-good".to_string()), 5000),
        );

        assert!(r1.is_ok());
        assert!(r2.is_ok());
        assert!(r3.is_ok());

        assert!(r1.unwrap().is_success());
        assert!(!r2.unwrap().is_success());
        assert!(r3.unwrap().is_success());
    }

    #[tokio::test]
    async fn fd3_step_result_is_success_correctness() {
        assert!(StepResult::Success { output: "x".to_string() }.is_success());
        assert!(!StepResult::Failure { output: "x".to_string() }.is_success());
    }

    #[tokio::test]
    async fn fd3_step_result_equality() {
        let a = StepResult::Success { output: "done".to_string() };
        let b = StepResult::Success { output: "done".to_string() };
        let c = StepResult::Failure { output: "done".to_string() };

        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[tokio::test]
    async fn fd3_step_result_serialization_roundtrip() {
        let result = StepResult::Success { output: "test-output".to_string() };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: StepResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);

        let failure = StepResult::Failure { output: "err".to_string() };
        let json_fail = serde_json::to_string(&failure).unwrap();
        let deserialized_fail: StepResult = serde_json::from_str(&json_fail).unwrap();
        assert_eq!(failure, deserialized_fail);
    }

    #[tokio::test]
    async fn fd3_multiple_success_steps_identical_output() {
        let _guard = state_guard();
        let ids = ["step-1", "step-good", "step-valid", "step-retry", "workflow-step-1"];

        for id in ids {
            let result = execute_step(StepId::new(id.to_string()), 5000).await;
            if let Ok(StepResult::Success { output }) = result {
                assert_eq!(output, "done", "Step {} should produce 'done'", id);
            } else {
                panic!("Step {} should succeed", id);
            }
        }
    }

    #[tokio::test]
    async fn fd3_failure_output_not_empty() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-fail".to_string()), 5000).await;
        if let Ok(StepResult::Failure { output }) = result {
            assert!(!output.is_empty(), "Failure output should not be empty");
        } else {
            panic!("Expected Failure");
        }
    }
}

// ============================================================================
// ADR-023: Stderr Bounds Expansion
// ============================================================================

#[cfg(test)]
mod stderr_bounds_expansion_tests {
    use super::*;

    const MAX_STDERR_BYTES: usize = 1_000_000;

    #[tokio::test]
    async fn stderr_success_output_well_under_bound() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-1".to_string()), 5000).await;
        if let Ok(StepResult::Success { output }) = result {
            assert!(output.len() < MAX_STDERR_BYTES);
            assert!(output.len() < 1000, "Success output should be small, got {} bytes", output.len());
        }
    }

    #[tokio::test]
    async fn stderr_failure_output_well_under_bound() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-fail".to_string()), 5000).await;
        if let Ok(StepResult::Failure { output }) = result {
            assert!(output.len() < MAX_STDERR_BYTES);
            assert!(output.len() < 1000, "Failure output should be small, got {} bytes", output.len());
        }
    }

    #[tokio::test]
    async fn stderr_transient_error_reason_well_under_bound() {
        let _guard = state_guard();
        let _ = execute_step(StepId::new("step-transient".to_string()), 5000).await;

        if let Some(ExecuteNodeError::TransientError { reason, .. }) =
            get_last_error(&StepId::new("step-transient".to_string()))
        {
            assert!(reason.len() < MAX_STDERR_BYTES);
            assert!(reason.len() < 1000, "Error reason should be small, got {} bytes", reason.len());
        }
    }

    #[tokio::test]
    async fn stderr_timeout_error_well_under_bound() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-slow".to_string()), 1).await;
        if let Err(ExecuteNodeError::TimeoutExceeded { .. }) = result {
            let err_str = format!("{:?}", result.unwrap_err());
            assert!(err_str.len() < MAX_STDERR_BYTES);
        }
    }

    #[tokio::test]
    async fn stderr_no_accumulation_across_executions() {
        let _guard = state_guard();

        for i in 0..20 {
            let result = execute_step(StepId::new("step-good".to_string()), 5000).await;
            if let Ok(StepResult::Success { output }) = result {
                assert_eq!(output.len(), 4, "Output should not grow on iteration {}", i);
            }
        }
    }

    #[tokio::test]
    async fn stderr_no_truncation_marker_in_simulated_output() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-fail".to_string()), 5000).await;
        if let Ok(StepResult::Failure { output }) = result {
            assert!(
                !output.contains("TRUNCATED"),
                "Simulated output should not contain truncation marker"
            );
        }
    }

    #[tokio::test]
    async fn stderr_error_display_format_bounded() {
        let _guard = state_guard();
        let _ = execute_step(StepId::new("step-transient".to_string()), 5000).await;

        if let Some(err) = get_last_error(&StepId::new("step-transient".to_string())) {
            let display = format!("{}", err);
            assert!(display.len() < MAX_STDERR_BYTES);
            assert!(display.len() < 1000, "Error display should be compact");
        }
    }

    #[tokio::test]
    async fn stderr_rapid_sequential_all_bounded() {
        let _guard = state_guard();
        let step_ids = [
            "step-1", "step-good", "step-fail", "step-transient", "step-valid",
        ];

        for id in step_ids {
            let result = execute_step(StepId::new(id.to_string()), 5000).await;
            match result {
                Ok(StepResult::Success { output }) => {
                    assert!(output.len() < MAX_STDERR_BYTES);
                }
                Ok(StepResult::Failure { output }) => {
                    assert!(output.len() < MAX_STDERR_BYTES);
                }
                Err(e) => {
                    let err_str = format!("{:?}", e);
                    assert!(err_str.len() < MAX_STDERR_BYTES);
                }
            }
        }
    }
}

// ============================================================================
// ADR-019: Termination Signal Expansion
// ============================================================================

#[cfg(test)]
mod termination_signal_expansion_tests {
    use super::*;

    #[tokio::test]
    async fn cancel_during_completed_state_is_noop() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        execute_step(step_id.clone(), 5000).await.unwrap();

        let cancel_result = cancel_execution(step_id.clone()).await;
        assert!(cancel_result.is_ok(), "Cancel after completion should be no-op");
    }

    #[tokio::test]
    async fn cancel_after_state_reset_is_clean() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        execute_step(step_id.clone(), 5000).await.unwrap();
        reset_all_state();

        let status = get_execution_status(&step_id);
        assert!(status.is_ready());

        let cancel_result = cancel_execution(step_id.clone()).await;
        assert!(cancel_result.is_ok());

        let status_after = get_execution_status(&step_id);
        assert!(matches!(status_after, ExecutionStatus::Cancelled { .. }));
    }

    #[tokio::test]
    async fn multiple_cancel_execute_cycles() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        for cycle in 0..5 {
            cancel_execution(step_id.clone())
                .await
                .unwrap_or_else(|_| panic!("Cancel failed on cycle {}", cycle));

            let status = get_execution_status(&step_id);
            assert!(
                matches!(status, ExecutionStatus::Cancelled { .. }),
                "Cycle {}: should be cancelled",
                cycle
            );

            let result = execute_step(step_id.clone(), 5000).await;
            assert!(result.is_ok(), "Cycle {}: should succeed after cancel", cycle);
        }
    }

    #[tokio::test]
    async fn cancel_different_steps_independently() {
        let _guard = state_guard();
        let step_a = StepId::new("step-1".to_string());
        let step_b = StepId::new("step-good".to_string());

        cancel_execution(step_a.clone()).await.unwrap();

        let status_a = get_execution_status(&step_a);
        let status_b = get_execution_status(&step_b);

        assert!(matches!(status_a, ExecutionStatus::Cancelled { .. }));
        assert!(status_b.is_ready(), "Step B should remain unaffected");
    }

    #[tokio::test]
    async fn cancel_returns_fresh_results_on_reexecution() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        let r1 = execute_step(step_id.clone(), 5000).await.unwrap();
        cancel_execution(step_id.clone()).await.unwrap();

        let r2 = execute_step(step_id.clone(), 5000).await.unwrap();
        assert_eq!(r1, r2, "Reexecution should produce identical result");
    }

    #[tokio::test]
    async fn cancel_then_status_shows_cancelled() {
        let _guard = state_guard();
        let step_id = StepId::new("step-valid".to_string());

        cancel_execution(step_id.clone()).await.unwrap();

        let status = get_execution_status(&step_id);
        assert!(matches!(status, ExecutionStatus::Cancelled { reason } if !reason.is_empty()));
    }

    #[tokio::test]
    async fn cancel_preserves_step_identity() {
        let _guard = state_guard();
        let step_id = StepId::new("workflow-step-1".to_string());

        cancel_execution(step_id.clone()).await.unwrap();

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok(), "Step identity preserved after cancel");
    }

    #[tokio::test]
    async fn cancel_after_failure_is_noop() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());

        let exec_result = execute_step(step_id.clone(), 5000).await;
        assert!(exec_result.is_err());

        let cancel_result = cancel_execution(step_id.clone()).await;
        assert!(cancel_result.is_ok(), "Cancel after failure should succeed");
    }
}

// ============================================================================
// Stale Completion Expansion
// ============================================================================

#[cfg(test)]
mod stale_completion_expansion_tests {
    use super::*;

    #[tokio::test]
    async fn stale_error_cleared_by_subsequent_success() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());

        execute_step(step_id.clone(), 5000)
            .await
            .expect_err("transient should fail");
        assert!(get_last_error(&step_id).is_some());

        execute_step(StepId::new("step-good".to_string()), 5000)
            .await
            .unwrap();

        assert!(
            get_last_error(&StepId::new("step-good".to_string())).is_none(),
            "Good step should have no error"
        );
    }

    #[tokio::test]
    async fn interleaved_reset_and_execution() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        execute_step(step_id.clone(), 5000).await.unwrap();
        reset_all_state();

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok(), "Should work after interleaved reset");
    }

    #[tokio::test]
    async fn stale_timeout_not_returned_on_reexecution() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let _ = execute_step(step_id.clone(), 1).await;

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(
            result.is_ok(),
            "Timeout from previous run should not affect re-execution"
        );
    }

    #[tokio::test]
    async fn multiple_resets_no_corruption() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        for i in 0..10 {
            reset_all_state();
            let result = execute_step(step_id.clone(), 5000).await;
            assert!(result.is_ok(), "Should succeed after reset {}", i);
        }
    }

    #[tokio::test]
    async fn error_set_and_cleared_independently_per_step() {
        let _guard = state_guard();
        let step_a = StepId::new("step-a-test".to_string());
        let step_b = StepId::new("step-b-test".to_string());

        set_error(
            step_a.as_str(),
            ExecuteNodeError::TransientError {
                reason: "error-a".to_string(),
                recoverable: true,
            },
        );
        set_error(
            step_b.as_str(),
            ExecuteNodeError::TransientError {
                reason: "error-b".to_string(),
                recoverable: false,
            },
        );

        clear_error(step_a.as_str());

        assert!(get_last_error(&step_a).is_none(), "Error A should be cleared");
        assert!(get_last_error(&step_b).is_some(), "Error B should persist");
    }

    #[tokio::test]
    async fn reset_during_retry_sequence_clears_state() {
        let _guard = state_guard();
        let step_id = StepId::new("step-flaky".to_string());
        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();

        set_error(
            step_id.as_str(),
            ExecuteNodeError::TransientError {
                reason: "pre-retry error".to_string(),
                recoverable: true,
            },
        );
        assert!(get_last_error(&step_id).is_some());

        reset_all_state();
        assert!(get_last_error(&step_id).is_none());

        let result = execute_step_with_retry(step_id.clone(), 5000, policy).await;
        assert!(matches!(result, Err(ExecuteNodeError::RetryExhausted { attempts: 3, .. })));
    }

    #[tokio::test]
    async fn stale_completion_different_step_ids_independent() {
        let _guard = state_guard();
        let step_a = StepId::new("step-transient".to_string());
        let step_b = StepId::new("step-1".to_string());

        execute_step(step_a.clone(), 5000).await.expect_err("transient fails");
        execute_step(step_b.clone(), 5000).await.unwrap();

        assert!(get_last_error(&step_a).is_some());
        assert!(get_last_error(&step_b).is_none());
    }
}

// ============================================================================
// Crash Injection Expansion
// ============================================================================

#[cfg(test)]
mod crash_injection_expansion_tests {
    use super::*;

    #[tokio::test]
    async fn crash_orphaned_executing_state_recovered() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        set_state(
            step_id.as_str(),
            StepState::Executing {
                step_id: step_id.clone(),
                start_time: Instant::now(),
            },
        );

        let status = get_execution_status(&step_id);
        assert!(matches!(status, ExecutionStatus::Executing { .. }));

        reset_all_state();

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok(), "Should recover from orphaned executing state");
    }

    #[tokio::test]
    async fn crash_error_overwrite_on_reexecution() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        set_error(
            step_id.as_str(),
            ExecuteNodeError::TransientError {
                reason: "crash remnant".to_string(),
                recoverable: true,
            },
        );

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok(), "Should overwrite crash error on successful reexecution");
        assert!(
            get_last_error(&step_id).is_none(),
            "Error should be cleared after successful execution"
        );
    }

    #[tokio::test]
    async fn crash_concurrent_recovery_multiple_steps() {
        let _guard = state_guard();

        let steps = vec![
            StepId::new("step-good".to_string()),
            StepId::new("step-valid".to_string()),
            StepId::new("step-retry".to_string()),
        ];

        for step in &steps {
            set_error(
                step.as_str(),
                ExecuteNodeError::ExecutionCancelled {
                    reason: "simulated crash".to_string(),
                },
            );
        }

        let handles: Vec<_> = steps
            .into_iter()
            .map(|sid| {
                tokio::spawn(async move { execute_step(sid, 5000).await })
            })
            .collect();

        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok(), "Should recover from concurrent crash");
        }
    }

    #[tokio::test]
    async fn crash_mid_retry_with_backoff() {
        let _guard = state_guard();
        let step_id = StepId::new("step-flaky".to_string());

        let policy = RetryPolicy::new(5, 100, 2.0).unwrap();
        let start = Instant::now();
        let result = execute_step_with_retry(step_id.clone(), 5000, policy).await;
        let elapsed = start.elapsed();

        assert!(matches!(result, Err(ExecuteNodeError::RetryExhausted { attempts: 5, .. })));
        assert!(elapsed < Duration::from_secs(2), "Should not hang on crash during retry");
    }

    #[tokio::test]
    async fn crash_full_state_reset_after_partial_execution() {
        let _guard = state_guard();
        let step_a = StepId::new("step-transient".to_string());
        let step_b = StepId::new("step-1".to_string());

        let _ = execute_step(step_a.clone(), 5000).await;
        execute_step(step_b.clone(), 5000).await.unwrap();

        assert!(get_last_error(&step_a).is_some());
        assert!(get_last_error(&step_b).is_none());

        reset_all_state();

        assert!(get_last_error(&step_a).is_none());
        assert!(get_last_error(&step_b).is_none());
        assert!(get_execution_status(&step_a).is_ready());
        assert!(get_execution_status(&step_b).is_ready());

        let result = execute_step(step_b.clone(), 5000).await;
        assert!(result.is_ok(), "Clean recovery after full reset");
    }

    #[tokio::test]
    async fn crash_multiple_steps_orphaned_states() {
        let _guard = state_guard();

        let steps = ["step-a", "step-b", "step-c"];
        for id in steps {
            set_state(
                id,
                StepState::Executing {
                    step_id: StepId::new(id.to_string()),
                    start_time: Instant::now(),
                },
            );
        }

        reset_all_state();

        for id in steps {
            let status = get_execution_status(&StepId::new(id.to_string()));
            assert!(status.is_ready(), "Step {} should recover to ready", id);
        }
    }

    #[tokio::test]
    async fn crash_error_persistence_across_state_transitions() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());

        execute_step(step_id.clone(), 5000)
            .await
            .expect_err("transient should fail");

        let err_before = get_last_error(&step_id).clone();
        assert!(err_before.is_some());

        cancel_execution(step_id.clone()).await.unwrap();

        let err_after = get_last_error(&step_id);
        assert!(err_after.is_some(), "Error should persist through cancel");
    }

    #[tokio::test]
    async fn crash_between_set_error_and_set_state() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        set_error(
            step_id.as_str(),
            ExecuteNodeError::TransientError {
                reason: "partial crash".to_string(),
                recoverable: false,
            },
        );

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok(), "Should recover from partial crash state");
        assert!(get_last_error(&step_id).is_none());
    }

    #[tokio::test]
    async fn crash_recovery_alternating_timeout_and_success() {
        let _guard = state_guard();
        let step_slow = StepId::new("step-slow".to_string());

        for cycle in 0..3 {
            let r_timeout = execute_step(step_slow.clone(), 1).await;
            assert!(r_timeout.is_err(), "Cycle {}: should timeout", cycle);

            let r_success = execute_step(step_slow.clone(), 5000).await;
            assert!(r_success.is_ok(), "Cycle {}: should succeed with adequate timeout", cycle);
        }
    }

    #[tokio::test]
    async fn crash_recovery_scheduler_state_independent() {
        let _guard = state_guard();
        let config = SchedulerConfig {
            max_concurrent: 2,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let mut scheduler = vo_executor::scheduler::Scheduler::new(config);

        let job = vo_executor::Job::new(
            vo_executor::JobId::new(1),
            "test".to_string(),
            vo_executor::Schedule::one_shot(Duration::from_millis(50)),
        );
        scheduler.schedule(job).unwrap();

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        let due = scheduler.poll_due_jobs(now_ms + 100);
        assert_eq!(due.len(), 1);

        reset_all_state();

        let result = execute_step(StepId::new("step-1".to_string()), 5000).await;
        assert!(result.is_ok(), "Execution should work after state reset");

        let due2 = scheduler.poll_due_jobs(now_ms + 100);
        assert_eq!(due2.len(), 1, "Scheduler state should be independent");
    }
}

// ============================================================================
// StepId Edge Cases
// ============================================================================

#[cfg(test)]
mod step_id_edge_cases {
    use super::*;

    #[test]
    fn step_id_new_any_string_accepted() {
        let id = StepId::new("any string with spaces!".to_string());
        assert_eq!(id.as_str(), "any string with spaces!");
    }

    #[test]
    fn step_id_parse_rejects_empty() {
        assert!(StepId::parse("").is_err());
    }

    #[test]
    fn step_id_parse_rejects_whitespace() {
        assert!(StepId::parse(" ").is_err());
        assert!(StepId::parse("\t").is_err());
        assert!(StepId::parse("\n").is_err());
    }

    #[test]
    fn step_id_parse_accepts_underscores_and_hyphens() {
        assert!(StepId::parse("my_step-123").is_ok());
        assert!(StepId::parse("_leading").is_ok());
        assert!(StepId::parse("trailing_").is_ok());
        assert!(StepId::parse("-dash").is_ok());
    }

    #[test]
    fn step_id_parse_rejects_special_chars() {
        assert!(StepId::parse("step.test").is_err());
        assert!(StepId::parse("step:test").is_err());
        assert!(StepId::parse("step/test").is_err());
        assert!(StepId::parse("step@test").is_err());
        assert!(StepId::parse("step#test").is_err());
        assert!(StepId::parse("step test").is_err());
    }

    #[test]
    fn step_id_clone_and_equality() {
        let a = StepId::new("test".to_string());
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn step_id_hash_consistency() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(StepId::new("step-1".to_string()));
        set.insert(StepId::new("step-1".to_string()));
        set.insert(StepId::new("step-2".to_string()));

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn step_id_into_string() {
        let id = StepId::new("my-step".to_string());
        let s: String = id.clone().into();
        assert_eq!(s, "my-step");
        assert_eq!(id.as_str(), "my-step");
    }

    #[test]
    fn step_id_as_ref_str() {
        let id = StepId::new("ref-test".to_string());
        assert_eq!(id.as_ref(), "ref-test");
    }
}

// ============================================================================
// ExecutionStatus Edge Cases
// ============================================================================

#[cfg(test)]
mod execution_status_edge_cases {
    use super::*;

    #[test]
    fn execution_status_equality() {
        assert_eq!(
            ExecutionStatus::Ready,
            ExecutionStatus::Ready
        );
        assert_eq!(
            ExecutionStatus::Cancelled { reason: "x".to_string() },
            ExecutionStatus::Cancelled { reason: "x".to_string() }
        );
        assert_ne!(
            ExecutionStatus::Cancelled { reason: "a".to_string() },
            ExecutionStatus::Cancelled { reason: "b".to_string() }
        );
    }

    #[test]
    fn execution_status_completed_with_empty_output() {
        let status = ExecutionStatus::Completed { output: String::new() };
        assert!(!status.is_ready());
        let debug = format!("{:?}", status);
        assert!(debug.contains("Completed"));
    }

    #[test]
    fn execution_status_executing_elapsed_monotonic() {
        let step_id = StepId::new("test".to_string());
        let start_time = Instant::now();
        set_state(
            step_id.as_str(),
            StepState::Executing {
                step_id: step_id.clone(),
                start_time,
            },
        );

        let status = get_execution_status(&step_id);
        if let ExecutionStatus::Executing { elapsed_ms, .. } = status {
            std::thread::sleep(Duration::from_millis(10));
            let status2 = get_execution_status(&step_id);
            if let ExecutionStatus::Executing { elapsed_ms: elapsed_ms2, .. } = status2 {
                assert!(elapsed_ms2 >= elapsed_ms, "Elapsed should be monotonically increasing");
            }
        }

        reset_all_state();
    }
}

// ============================================================================
// RetryPolicy Construction Edge Cases
// ============================================================================

#[cfg(test)]
mod retry_policy_construction_tests {
    use super::*;

    #[test]
    fn retry_policy_new_with_multiplier_1_accepted() {
        let policy = RetryPolicy::new(3, 100, 1.0);
        assert!(policy.is_ok());
    }

    #[test]
    fn retry_policy_new_with_multiplier_1_0_accepted() {
        let policy = RetryPolicy::new(3, 100, 1.0000001);
        assert!(policy.is_ok());
    }

    #[test]
    fn retry_policy_new_with_multiplier_0_9_rejected() {
        let policy = RetryPolicy::new(3, 100, 0.9);
        assert!(policy.is_err());
    }

    #[test]
    fn retry_policy_new_negative_infinity_rejected() {
        let policy = RetryPolicy::new(3, 100, f64::NEG_INFINITY);
        assert!(policy.is_err());
    }

    #[test]
    fn retry_policy_with_max_backoff_equal_to_backoff_accepted() {
        let policy = RetryPolicy::with_max_backoff(3, 500, 2.0, 500);
        assert!(policy.is_ok());
    }

    #[test]
    fn retry_policy_default_max_backoff_is_u64_max() {
        let policy = RetryPolicy::new(3, 100, 2.0).unwrap();
        assert_eq!(policy.max_backoff_ms, u64::MAX);
    }

    #[test]
    fn retry_policy_clone_and_equality() {
        let a = RetryPolicy::new(3, 100, 2.0).unwrap();
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn retry_policy_debug_format() {
        let policy = RetryPolicy::new(5, 200, 1.5).unwrap();
        let debug = format!("{:?}", policy);
        assert!(debug.contains("5"));
        assert!(debug.contains("200"));
    }
}
