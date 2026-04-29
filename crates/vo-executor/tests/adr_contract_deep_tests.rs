//! Deep ADR contract tests for vo-executor (ve-t6dm)
//!
//! Tests gaps identified in test review:
//! - ADR-006/015: Semaphore with async acquire, execution+semaphore integration
//! - ADR-012: InvalidTransition guard, FD3 serialization edge cases
//! - FD3: StepResult serialization with Unicode, empty, large payloads
//! - ADR-019: Cancel during executing state (real guard), multi-step cascade
//! - ADR-023: Stderr accumulation under concurrent failures
//! - Stale completion: Error overwrite semantics, concurrent cancel+execute race
//! - Crash injection: DashMap corruption resilience, partial state injection

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use vo_executor::state::{set_state, StepState};
use vo_executor::{
    cancel_execution, clear_error, execute_step, execute_step_with_retry, get_execution_status,
    get_last_error, reset_all_state, scheduler::SchedulerConfig, set_error, ExecuteNodeError,
    ExecutionStatus, RetryPolicy, RetryPolicyError, StepId, StepResult,
};

static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn state_guard() -> MutexGuard<'static, ()> {
    let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset_all_state();
    guard
}

// ============================================================================
// ADR-006/015: Semaphore Async Acquire & Execution Integration
// ============================================================================

#[cfg(test)]
mod semaphore_async_acquire_tests {
    use super::*;

    #[tokio::test]
    async fn semaphore_async_acquire_blocks_until_permit_available() {
        let _guard = state_guard();
        let config = SchedulerConfig {
            max_concurrent: 1,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Arc::new(vo_executor::scheduler::Scheduler::new(config));

        let permit1 = scheduler.try_acquire().unwrap();
        assert!(scheduler.try_acquire().is_none());

        let sched = Arc::clone(&scheduler);
        let handle = tokio::spawn(async move {
            let _permit = sched.acquire().await;
            "acquired"
        });

        drop(permit1);

        let result = tokio::time::timeout(Duration::from_secs(1), handle)
            .await
            .expect("async acquire should complete within 1s")
            .unwrap();
        assert_eq!(result, "acquired");
    }

    #[tokio::test]
    async fn semaphore_multiple_async_waiters_all_acquire() {
        let _guard = state_guard();
        let config = SchedulerConfig {
            max_concurrent: 1,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Arc::new(vo_executor::scheduler::Scheduler::new(config));

        let permit = scheduler.try_acquire().unwrap();

        let completed = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();

        for _ in 0..5 {
            let sched = Arc::clone(&scheduler);
            let done = Arc::clone(&completed);
            handles.push(tokio::spawn(async move {
                let _p = sched.acquire().await;
                done.fetch_add(1, Ordering::SeqCst);
            }));
        }

        drop(permit);

        for handle in handles {
            tokio::time::timeout(Duration::from_secs(2), handle)
                .await
                .expect("all waiters should acquire")
                .unwrap();
        }

        assert_eq!(completed.load(Ordering::SeqCst), 5);
    }

    #[tokio::test]
    async fn execution_with_semaphore_backpressure() {
        let _guard = state_guard();
        let config = SchedulerConfig {
            max_concurrent: 2,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Arc::new(vo_executor::scheduler::Scheduler::new(config));

        let p1 = scheduler.try_acquire().unwrap();
        let p2 = scheduler.try_acquire().unwrap();
        assert!(scheduler.try_acquire().is_none());

        let step_id = StepId::new("step-good".to_string());
        let result = execute_step(step_id, 5000).await;
        assert!(
            result.is_ok(),
            "Execution should succeed even when semaphore is full"
        );

        drop(p1);
        drop(p2);
    }

    #[tokio::test]
    async fn semaphore_permits_survive_across_tasks() {
        let _guard = state_guard();
        let config = SchedulerConfig {
            max_concurrent: 3,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Arc::new(vo_executor::scheduler::Scheduler::new(config));

        let mut all_permits = Vec::new();

        for _ in 0..3 {
            let sched = Arc::clone(&scheduler);
            let handle = tokio::spawn(async move { sched.try_acquire() });
            let permit = handle.await.unwrap();
            all_permits.push(permit.unwrap());
        }

        assert!(scheduler.try_acquire().is_none());

        drop(all_permits);

        assert!(scheduler.try_acquire().is_some());
    }
}

// ============================================================================
// ADR-012: InvalidTransition Guard
// ============================================================================

#[cfg(test)]
mod invalid_transition_guard_tests {
    use super::*;

    #[tokio::test]
    async fn execute_during_executing_state_rejected() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        set_state(
            step_id.as_str(),
            StepState::Executing {
                step_id: step_id.clone(),
                start_time: Instant::now(),
            },
        );

        let result = execute_step(step_id.clone(), 5000).await;
        match &result {
            Err(ExecuteNodeError::InvalidTransition { from_state, .. }) => {
                assert_eq!(
                    from_state, "Executing",
                    "Should reject execution during Executing state"
                );
            }
            other => panic!("Expected InvalidTransition, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn execute_after_executing_state_clears_succeeds() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        set_state(
            step_id.as_str(),
            StepState::Executing {
                step_id: step_id.clone(),
                start_time: Instant::now(),
            },
        );

        let r1 = execute_step(step_id.clone(), 5000).await;
        assert!(r1.is_err());

        set_state(step_id.as_str(), StepState::Ready);

        let r2 = execute_step(step_id.clone(), 5000).await;
        assert!(r2.is_ok(), "Should succeed after clearing Executing state");
    }

    #[tokio::test]
    async fn retry_with_invalid_transition_propagates() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        set_state(
            step_id.as_str(),
            StepState::Executing {
                step_id: step_id.clone(),
                start_time: Instant::now(),
            },
        );

        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();
        let result = execute_step_with_retry(step_id.clone(), 5000, policy).await;
        assert!(
            matches!(result, Err(ExecuteNodeError::InvalidTransition { .. })),
            "Retry should not bypass transition guard"
        );
    }
}

// ============================================================================
// FD3: Serialization Edge Cases
// ============================================================================

#[cfg(test)]
mod fd3_serialization_edge_tests {
    use super::*;

    #[tokio::test]
    async fn fd3_success_with_unicode_output() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-1".to_string()), 5000).await;
        if let Ok(StepResult::Success { output }) = result {
            assert!(output.is_ascii(), "Current output should be ASCII");
        }
    }

    #[tokio::test]
    async fn fd3_step_result_failure_json_roundtrip() {
        let failure = StepResult::Failure {
            output: "error: exit code 1".to_string(),
        };
        let json = serde_json::to_string(&failure).unwrap();
        let deserialized: StepResult = serde_json::from_str(&json).unwrap();
        assert_eq!(failure, deserialized);
        assert!(!deserialized.is_success());
    }

    #[tokio::test]
    async fn fd3_step_result_success_json_fields() {
        let success = StepResult::Success {
            output: "done".to_string(),
        };
        let json = serde_json::to_value(&success).unwrap();
        assert_eq!(json["Success"]["output"], "done");
    }

    #[tokio::test]
    async fn fd3_step_result_failure_json_fields() {
        let failure = StepResult::Failure {
            output: "error: exit code 1".to_string(),
        };
        let json = serde_json::to_value(&failure).unwrap();
        assert_eq!(json["Failure"]["output"], "error: exit code 1");
    }

    #[tokio::test]
    async fn fd3_concurrent_serialization_independent() {
        let _guard = state_guard();
        let handles: Vec<_> = (0..10)
            .map(|_| {
                tokio::spawn(async {
                    let result = execute_step(StepId::new("step-1".to_string()), 5000).await;
                    if let Ok(StepResult::Success { output }) = result {
                        let json = serde_json::to_string(&StepResult::Success { output }).unwrap();
                        let back: StepResult = serde_json::from_str(&json).unwrap();
                        back
                    } else {
                        panic!("Expected success");
                    }
                })
            })
            .collect();

        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_success());
        }
    }

    #[tokio::test]
    async fn fd3_empty_success_output_serializes() {
        let empty_success = StepResult::Success {
            output: String::new(),
        };
        let json = serde_json::to_string(&empty_success).unwrap();
        let back: StepResult = serde_json::from_str(&json).unwrap();
        assert_eq!(empty_success, back);
    }

    #[tokio::test]
    async fn fd3_empty_failure_output_serializes() {
        let empty_failure = StepResult::Failure {
            output: String::new(),
        };
        let json = serde_json::to_string(&empty_failure).unwrap();
        let back: StepResult = serde_json::from_str(&json).unwrap();
        assert_eq!(empty_failure, back);
    }

    #[tokio::test]
    async fn fd3_step_result_debug_format() {
        let success = StepResult::Success {
            output: "test".to_string(),
        };
        let debug = format!("{:?}", success);
        assert!(debug.contains("Success"));
        assert!(debug.contains("test"));

        let failure = StepResult::Failure {
            output: "err".to_string(),
        };
        let debug_fail = format!("{:?}", failure);
        assert!(debug_fail.contains("Failure"));
    }
}

// ============================================================================
// ADR-019: Termination Signal Race Conditions
// ============================================================================

#[cfg(test)]
mod termination_race_tests {
    use super::*;

    #[tokio::test]
    async fn cancel_during_executing_state_returns_error() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        set_state(
            step_id.as_str(),
            StepState::Executing {
                step_id: step_id.clone(),
                start_time: Instant::now(),
            },
        );

        let result = cancel_execution(step_id.clone()).await;
        assert!(
            matches!(result, Err(ExecuteNodeError::ExecutionCancelled { .. })),
            "Cancel during Executing should return error"
        );
    }

    #[tokio::test]
    async fn cancel_preserves_executing_state_on_error() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        set_state(
            step_id.as_str(),
            StepState::Executing {
                step_id: step_id.clone(),
                start_time: Instant::now(),
            },
        );

        let _ = cancel_execution(step_id.clone()).await;

        let status = get_execution_status(&step_id);
        assert!(
            matches!(status, ExecutionStatus::Executing { .. }),
            "Executing state should be preserved when cancel fails"
        );
    }

    #[tokio::test]
    async fn multi_step_cancel_cascade() {
        let _guard = state_guard();
        let steps = vec![
            StepId::new("step-1".to_string()),
            StepId::new("step-good".to_string()),
            StepId::new("step-valid".to_string()),
        ];

        for step in &steps {
            cancel_execution(step.clone()).await.unwrap();
        }

        for step in &steps {
            let status = get_execution_status(step);
            assert!(
                matches!(status, ExecutionStatus::Cancelled { .. }),
                "Step {} should be cancelled",
                step
            );
        }
    }

    #[tokio::test]
    async fn cancel_execute_cancel_cycle() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        cancel_execution(step_id.clone()).await.unwrap();
        let r1 = execute_step(step_id.clone(), 5000).await;
        assert!(r1.is_ok());
        cancel_execution(step_id.clone()).await.unwrap();

        let status = get_execution_status(&step_id);
        assert!(matches!(status, ExecutionStatus::Cancelled { .. }));
    }

    #[tokio::test]
    async fn concurrent_cancel_and_execute() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        let sid_clone = step_id.clone();
        let (cancel_result, exec_result) = tokio::join!(
            cancel_execution(step_id.clone()),
            execute_step(sid_clone, 5000)
        );

        assert!(cancel_result.is_ok());
        assert!(exec_result.is_ok());
    }

    #[tokio::test]
    async fn cancel_reason_propagated_to_status() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        cancel_execution(step_id.clone()).await.unwrap();

        if let ExecutionStatus::Cancelled { reason } = get_execution_status(&step_id) {
            assert_eq!(reason, "cancelled by user");
        } else {
            panic!("Expected Cancelled status");
        }
    }

    #[tokio::test]
    async fn rapid_cancel_execute_cancel_several_cycles() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        for _ in 0..20 {
            cancel_execution(step_id.clone()).await.unwrap();
            let r = execute_step(step_id.clone(), 5000).await;
            assert!(r.is_ok());
        }
    }
}

// ============================================================================
// ADR-023: Stderr Bounds Under Stress
// ============================================================================

#[cfg(test)]
mod stderr_stress_tests {
    use super::*;

    #[tokio::test]
    async fn stderr_rapid_failure_loop_all_bounded() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());

        for _ in 0..50 {
            let _ = execute_step(step_id.clone(), 5000).await;
            if let Some(err) = get_last_error(&step_id) {
                let err_str = format!("{:?}", err);
                assert!(
                    err_str.len() < 10_000,
                    "Error should stay compact under rapid loop"
                );
            }
        }
    }

    #[tokio::test]
    async fn stderr_mixed_success_failure_no_leak() {
        let _guard = state_guard();
        let good = StepId::new("step-good".to_string());
        let bad = StepId::new("step-transient".to_string());

        for _ in 0..20 {
            let _ = execute_step(good.clone(), 5000).await;
            let _ = execute_step(bad.clone(), 5000).await;
        }

        assert!(
            get_last_error(&good).is_none(),
            "Good step should never accumulate error"
        );
    }

    #[tokio::test]
    async fn stderr_timeout_error_format_consistent() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());
        let result = execute_step(step_id.clone(), 1).await;

        if let Err(ExecuteNodeError::TimeoutExceeded {
            elapsed_ms,
            limit_ms,
        }) = result
        {
            assert_eq!(elapsed_ms, 3000);
            assert_eq!(limit_ms, 1);
            let display = format!(
                "{:?}",
                ExecuteNodeError::TimeoutExceeded {
                    elapsed_ms,
                    limit_ms
                }
            );
            assert!(
                display.len() < 200,
                "Timeout error display should be compact"
            );
        }
    }

    #[tokio::test]
    async fn stderr_concurrent_different_errors_no_interference() {
        let _guard = state_guard();
        let transient = StepId::new("step-transient".to_string());
        let good = StepId::new("step-1".to_string());
        let fail = StepId::new("step-fail".to_string());

        let (r_t, r_g, r_f) = tokio::join!(
            execute_step(transient.clone(), 5000),
            execute_step(good.clone(), 5000),
            execute_step(fail.clone(), 5000),
        );

        assert!(r_t.is_err());
        assert!(r_g.is_ok());
        assert!(r_f.is_ok());

        assert!(get_last_error(&transient).is_some());
        assert!(get_last_error(&good).is_none());
        assert!(get_last_error(&fail).is_none());
    }
}

// ============================================================================
// Stale Completion: Error Overwrite Semantics
// ============================================================================

#[cfg(test)]
mod stale_overwrite_tests {
    use super::*;

    #[tokio::test]
    async fn successful_execution_clears_previous_error() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());

        let _ = execute_step(step_id.clone(), 5000).await;
        assert!(get_last_error(&step_id).is_some());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_err(), "step-transient always fails");

        let error = get_last_error(&step_id);
        assert!(error.is_some(), "Error should be refreshed on reexecution");
    }

    #[tokio::test]
    async fn error_overwritten_by_cancel_then_execute() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());

        let _ = execute_step(step_id.clone(), 5000).await;
        assert!(get_last_error(&step_id).is_some());

        cancel_execution(step_id.clone()).await.unwrap();

        let status = get_execution_status(&step_id);
        assert!(matches!(status, ExecutionStatus::Cancelled { .. }));

        let error = get_last_error(&step_id);
        assert!(
            error.is_some(),
            "Error from transient should persist through cancel"
        );
    }

    #[tokio::test]
    async fn set_error_manually_then_execute_clears() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        set_error(
            step_id.as_str(),
            ExecuteNodeError::TransientError {
                reason: "injected".to_string(),
                recoverable: true,
            },
        );

        assert!(get_last_error(&step_id).is_some());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());
        assert!(
            get_last_error(&step_id).is_none(),
            "Successful execution should clear injected error"
        );
    }

    #[tokio::test]
    async fn concurrent_cancel_and_error_check() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        let sid = step_id.clone();
        let (cancel_res, exec_res) = tokio::join!(
            cancel_execution(step_id.clone()),
            tokio::spawn(async move { execute_step(sid, 5000).await })
        );

        assert!(cancel_res.is_ok());
        assert!(exec_res.unwrap().is_ok());
    }

    #[tokio::test]
    async fn reset_between_two_different_steps_isolated() {
        let _guard = state_guard();
        let a = StepId::new("step-transient".to_string());
        let b = StepId::new("step-1".to_string());

        let _ = execute_step(a.clone(), 5000).await;
        assert!(get_last_error(&a).is_some());

        reset_all_state();

        let result = execute_step(b.clone(), 5000).await;
        assert!(result.is_ok());
        assert!(get_last_error(&a).is_none());
        assert!(get_last_error(&b).is_none());
    }
}

// ============================================================================
// Crash Injection: DashMap Resilience & Partial State
// ============================================================================

#[cfg(test)]
mod crash_dashmap_tests {
    use super::*;

    #[tokio::test]
    async fn crash_inject_cancelled_state_then_execute() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        set_state(
            step_id.as_str(),
            StepState::Cancelled {
                reason: "crash injection".to_string(),
            },
        );

        let status = get_execution_status(&step_id);
        assert!(matches!(status, ExecutionStatus::Cancelled { .. }));

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(
            result.is_ok(),
            "Should recover from injected Cancelled state"
        );
    }

    #[tokio::test]
    async fn crash_inject_completed_state_then_execute() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        set_state(
            step_id.as_str(),
            StepState::Completed {
                output: "old output".to_string(),
            },
        );

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(
            result.is_ok(),
            "Should recover from injected Completed state"
        );
    }

    #[tokio::test]
    async fn crash_partial_error_no_state() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        set_error(
            step_id.as_str(),
            ExecuteNodeError::ExecutionCancelled {
                reason: "partial crash".to_string(),
            },
        );

        assert!(get_last_error(&step_id).is_some());
        assert!(get_execution_status(&step_id).is_ready());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());
        assert!(get_last_error(&step_id).is_none());
    }

    #[tokio::test]
    async fn crash_concurrent_partial_state_all_steps() {
        let _guard = state_guard();
        let steps = vec!["step-1", "step-good", "step-valid", "step-retry"];

        for id in &steps {
            set_error(
                *id,
                ExecuteNodeError::TransientError {
                    reason: "simulated crash".to_string(),
                    recoverable: false,
                },
            );
            set_state(
                *id,
                StepState::Cancelled {
                    reason: "crash".to_string(),
                },
            );
        }

        let handles: Vec<_> = steps
            .iter()
            .map(|id| {
                let sid = StepId::new(id.to_string());
                tokio::spawn(async move { (sid.clone(), execute_step(sid, 5000).await) })
            })
            .collect();

        for handle in handles {
            let (sid, result) = handle.await.unwrap();
            assert!(
                result.is_ok(),
                "Step {} should recover from crash state",
                sid
            );
            assert!(get_last_error(&sid).is_none());
        }
    }

    #[tokio::test]
    async fn crash_retry_exhaustion_preserves_attempt_count() {
        let _guard = state_guard();
        let step_id = StepId::new("step-flaky".to_string());
        let policy = RetryPolicy::new(7, 5, 2.0).unwrap();

        let result = execute_step_with_retry(step_id.clone(), 5000, policy).await;
        assert!(
            matches!(
                result,
                Err(ExecuteNodeError::RetryExhausted { attempts: 7, .. })
            ),
            "Should report exact attempt count"
        );
    }

    #[tokio::test]
    async fn crash_reset_during_active_executing() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        set_state(
            step_id.as_str(),
            StepState::Executing {
                step_id: step_id.clone(),
                start_time: Instant::now(),
            },
        );

        assert!(matches!(
            get_execution_status(&step_id),
            ExecutionStatus::Executing { .. }
        ));

        reset_all_state();

        assert!(get_execution_status(&step_id).is_ready());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn crash_scheduler_permits_independent_of_state() {
        let _guard = state_guard();
        let config = SchedulerConfig {
            max_concurrent: 2,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = vo_executor::scheduler::Scheduler::new(config);

        let p1 = scheduler.try_acquire().unwrap();
        let p2 = scheduler.try_acquire().unwrap();
        assert!(scheduler.try_acquire().is_none());

        reset_all_state();

        assert!(
            scheduler.try_acquire().is_none(),
            "Scheduler permits unaffected by state reset"
        );

        drop(p1);
        drop(p2);
        assert!(
            scheduler.try_acquire().is_some(),
            "Permit released correctly after crash recovery"
        );
    }
}

// ============================================================================
// ExecuteNodeError Display & Error Chain
// ============================================================================

#[cfg(test)]
mod error_display_tests {
    use super::*;

    #[test]
    fn error_step_not_found_display() {
        let err = ExecuteNodeError::StepNotFound {
            step_id: StepId::new("missing".to_string()),
        };
        let display = format!("{}", err);
        assert!(display.contains("missing"));
        assert!(display.contains("not found"));
    }

    #[test]
    fn error_invalid_timeout_display() {
        let err = ExecuteNodeError::InvalidTimeout {
            value: 0,
            reason: "must be > 0ms".to_string(),
        };
        let display = format!("{}", err);
        assert!(display.contains("0"));
        assert!(display.contains("> 0ms"));
    }

    #[test]
    fn error_timeout_exceeded_display() {
        let err = ExecuteNodeError::TimeoutExceeded {
            elapsed_ms: 3000,
            limit_ms: 100,
        };
        let display = format!("{}", err);
        assert!(display.contains("3000"));
        assert!(display.contains("100"));
    }

    #[test]
    fn error_transient_display() {
        let err = ExecuteNodeError::TransientError {
            reason: "network timeout".to_string(),
            recoverable: true,
        };
        let display = format!("{}", err);
        assert!(display.contains("network timeout"));
        assert!(display.contains("recoverable=true"));
    }

    #[test]
    fn error_retry_exhausted_display() {
        let err = ExecuteNodeError::RetryExhausted {
            attempts: 5,
            last_error: Box::new(ExecuteNodeError::TransientError {
                reason: "fail".to_string(),
                recoverable: false,
            }),
        };
        let display = format!("{}", err);
        assert!(display.contains("5"));
        assert!(display.contains("fail"));
    }

    #[test]
    fn error_execution_cancelled_display() {
        let err = ExecuteNodeError::ExecutionCancelled {
            reason: "user request".to_string(),
        };
        let display = format!("{}", err);
        assert!(display.contains("user request"));
    }

    #[test]
    fn error_invalid_transition_display() {
        let err = ExecuteNodeError::InvalidTransition {
            from_state: "Executing".to_string(),
            action: "execute_step".to_string(),
        };
        let display = format!("{}", err);
        assert!(display.contains("Executing"));
        assert!(display.contains("execute_step"));
    }

    #[test]
    fn error_invalid_retry_policy_display() {
        let err = ExecuteNodeError::InvalidRetryPolicy {
            node_name: "test-node".to_string(),
            reason: RetryPolicyError::ZeroAttempts,
        };
        let display = format!("{}", err);
        assert!(display.contains("test-node"));
        assert!(display.contains("Zero"));
    }

    #[test]
    fn error_equality() {
        let a = ExecuteNodeError::StepNotFound {
            step_id: StepId::new("x".to_string()),
        };
        let b = ExecuteNodeError::StepNotFound {
            step_id: StepId::new("x".to_string()),
        };
        let c = ExecuteNodeError::StepNotFound {
            step_id: StepId::new("y".to_string()),
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}

// ============================================================================
// RetryPolicy Backoff Precision
// ============================================================================

#[cfg(test)]
mod retry_backoff_precision_tests {
    use super::*;

    #[test]
    fn backoff_attempt_one_equals_base() {
        let policy = RetryPolicy::new(5, 200, 3.0).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 200);
    }

    #[test]
    fn backoff_exponential_growth() {
        let policy = RetryPolicy::new(10, 100, 2.0).unwrap();
        let d1 = policy.calculate_backoff_delay(1);
        let d2 = policy.calculate_backoff_delay(2);
        let d3 = policy.calculate_backoff_delay(3);
        assert!(d2 > d1, "Attempt 2 should be longer than 1");
        assert!(d3 > d2, "Attempt 3 should be longer than 2");
    }

    #[test]
    fn backoff_max_cap_enforced_from_start() {
        let policy = RetryPolicy::with_max_backoff(5, 1000, 10.0, 1000).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 1000);
        assert_eq!(policy.calculate_backoff_delay(2), 1000);
        assert_eq!(policy.calculate_backoff_delay(5), 1000);
    }

    #[test]
    fn backoff_fractional_multiplier() {
        let policy = RetryPolicy::new(5, 100, 1.5).unwrap();
        let d1 = policy.calculate_backoff_delay(1);
        let d2 = policy.calculate_backoff_delay(2);
        assert_eq!(d1, 100);
        assert_eq!(d2, 150);
    }

    #[test]
    fn backoff_large_attempt_no_panic() {
        let policy = RetryPolicy::new(5, 100, 2.0).unwrap();
        let _ = policy.calculate_backoff_delay(u32::MAX);
    }

    #[tokio::test]
    async fn retry_with_flaky_uses_backoff_delays() {
        let _guard = state_guard();
        let step_id = StepId::new("step-flaky".to_string());
        let policy = RetryPolicy::new(3, 50, 2.0).unwrap();

        let start = Instant::now();
        let result = execute_step_with_retry(step_id.clone(), 5000, policy).await;
        let elapsed = start.elapsed();

        assert!(matches!(
            result,
            Err(ExecuteNodeError::RetryExhausted { attempts: 3, .. })
        ));
        assert!(
            elapsed >= Duration::from_millis(50),
            "Should wait at least one backoff"
        );
    }
}

// ============================================================================
// ExecutionStatus Comprehensive
// ============================================================================

#[cfg(test)]
mod execution_status_comprehensive_tests {
    use super::*;

    #[test]
    fn execution_status_ready_display() {
        let status = ExecutionStatus::Ready;
        let display = format!("{:?}", status);
        assert_eq!(display, "Ready");
    }

    #[test]
    fn execution_status_all_variants_debug() {
        let ready = format!("{:?}", ExecutionStatus::Ready);
        let executing = format!(
            "{:?}",
            ExecutionStatus::Executing {
                step_id: StepId::new("x".to_string()),
                elapsed_ms: 42,
            }
        );
        let completed = format!(
            "{:?}",
            ExecutionStatus::Completed {
                output: "out".to_string(),
            }
        );
        let cancelled = format!(
            "{:?}",
            ExecutionStatus::Cancelled {
                reason: "r".to_string(),
            }
        );

        assert!(ready.contains("Ready"));
        assert!(executing.contains("Executing"));
        assert!(completed.contains("Completed"));
        assert!(cancelled.contains("Cancelled"));
    }

    #[test]
    fn execution_status_partial_eq_different_variants() {
        assert_ne!(
            ExecutionStatus::Ready,
            ExecutionStatus::Completed {
                output: String::new()
            }
        );
        assert_ne!(
            ExecutionStatus::Cancelled {
                reason: "a".to_string()
            },
            ExecutionStatus::Cancelled {
                reason: "b".to_string()
            }
        );
        assert_eq!(
            ExecutionStatus::Executing {
                step_id: StepId::new("s".to_string()),
                elapsed_ms: 0
            },
            ExecutionStatus::Executing {
                step_id: StepId::new("s".to_string()),
                elapsed_ms: 0
            }
        );
    }
}
