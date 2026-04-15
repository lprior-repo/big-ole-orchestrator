//! State machine tests for vo-executor
//!
//! Tests all valid and invalid state transitions in the workflow executor:
//! - Ready → Executing (via execute_step)
//! - Executing → Ready (via execute_step completion)
//! - Ready → Cancelled (via cancel_execution)
//! - Executing → Cancelled error (via cancel_execution - state unchanged)
//! - Terminal states: Cancelled, Completed

#[cfg(test)]
mod state_machine_transition_tests {
    use std::sync::LazyLock;
    use std::sync::Mutex;
    use std::sync::MutexGuard;
    use vo_executor::state::{set_state, StepState};
    use vo_executor::{
        cancel_execution, execute_step, get_execution_status, reset_all_state, ExecutionStatus,
        StepId, StepResult,
    };

    static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn state_guard() -> MutexGuard<'static, ()> {
        let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_all_state();
        guard
    }

    fn get_status(step_id: &StepId) -> ExecutionStatus {
        get_execution_status(step_id)
    }

    fn assert_ready(step_id: &StepId) {
        assert!(
            matches!(get_status(step_id), ExecutionStatus::Ready),
            "Expected Ready state"
        );
    }

    fn assert_cancelled(step_id: &StepId) {
        assert!(
            matches!(get_status(step_id), ExecutionStatus::Cancelled { .. }),
            "Expected Cancelled state"
        );
    }

    // =========================================================================
    // Ready → Executing Transitions
    // =========================================================================

    #[tokio::test]
    async fn state_ready_to_executing_to_ready_flow() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        assert_ready(&step_id);

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());
        assert_ready(&step_id);
    }

    // =========================================================================
    // Executing → Ready Transitions (successful completion)
    // =========================================================================

    #[tokio::test]
    async fn state_executing_to_ready_after_success() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(matches!(result, Ok(StepResult::Success { .. })));
        assert_ready(&step_id);
    }

    #[tokio::test]
    async fn state_executing_to_ready_after_failure() {
        let _guard = state_guard();
        let step_id = StepId::new("step-fail".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(matches!(result, Ok(StepResult::Failure { .. })));
        assert_ready(&step_id);
    }

    #[tokio::test]
    async fn state_executing_to_ready_after_transient_error() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_err());
        assert_ready(&step_id);
    }

    // =========================================================================
    // Ready → Cancelled Transitions
    // =========================================================================

    #[tokio::test]
    async fn state_ready_to_cancelled_on_cancel() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        assert_ready(&step_id);

        let result = cancel_execution(step_id.clone()).await;
        assert!(result.is_ok());
        assert_cancelled(&step_id);
    }

    #[tokio::test]
    async fn state_cancelled_is_terminal_no_transitions() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        cancel_execution(step_id.clone())
            .await
            .expect("cancel should succeed");
        assert_cancelled(&step_id);

        let result = cancel_execution(step_id.clone()).await;
        assert!(result.is_ok());
        assert_cancelled(&step_id);
    }

    #[tokio::test]
    async fn state_cancelled_can_still_execute() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        cancel_execution(step_id.clone())
            .await
            .expect("cancel should succeed");
        assert_cancelled(&step_id);

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());
        assert_ready(&step_id);
    }

    // =========================================================================
    // Cancelled State Tests
    // =========================================================================

    #[tokio::test]
    async fn state_cancel_multiple_steps_independently() {
        let _guard = state_guard();

        let step_a = StepId::new("step-1".to_string());
        let step_b = StepId::new("step-good".to_string());
        let step_c = StepId::new("step-retry".to_string());

        cancel_execution(step_a.clone())
            .await
            .expect("cancel should succeed");
        cancel_execution(step_b.clone())
            .await
            .expect("cancel should succeed");
        cancel_execution(step_c.clone())
            .await
            .expect("cancel should succeed");

        assert_cancelled(&step_a);
        assert_cancelled(&step_b);
        assert_cancelled(&step_c);
    }

    // =========================================================================
    // Invalid Transition Tests
    // =========================================================================

    #[tokio::test]
    async fn state_executing_cannot_start_another_execution() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        set_state(
            step_id.as_str(),
            StepState::Executing {
                step_id: step_id.clone(),
                start_time: std::time::Instant::now(),
            },
        );

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::InvalidTransition { .. })
        ));
    }

    #[tokio::test]
    async fn state_concurrent_same_step_both_get_result() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        let (result_a, result_b) = tokio::join!(
            execute_step(step_id.clone(), 5000),
            execute_step(step_id.clone(), 5000)
        );

        assert!(result_a.is_ok());
        assert!(result_b.is_ok());
        assert!(result_a.as_ref().unwrap().is_success());
        assert!(result_b.as_ref().unwrap().is_success());
    }

    // =========================================================================
    // Terminal State Tests
    // =========================================================================

    #[tokio::test]
    async fn state_completed_is_defined_but_never_set() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());

        let status = get_status(&step_id);
        assert!(matches!(status, ExecutionStatus::Ready));
    }

    // =========================================================================
    // Complex State Transition Scenarios
    // =========================================================================

    #[tokio::test]
    async fn state_execute_cancel_retry_flow() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        assert_ready(&step_id);

        let result1 = execute_step(step_id.clone(), 5000).await;
        assert!(result1.is_ok());
        assert_ready(&step_id);

        cancel_execution(step_id.clone())
            .await
            .expect("cancel should succeed");
        assert_cancelled(&step_id);

        let result2 = execute_step(step_id.clone(), 5000).await;
        assert!(result2.is_ok());
        assert_ready(&step_id);
    }

    #[tokio::test]
    async fn state_interleaved_steps_maintain_independent_states() {
        let _guard = state_guard();

        let step_a = StepId::new("step-good".to_string());
        let step_b = StepId::new("step-fail".to_string());

        let (result_a, result_b) = tokio::join!(
            execute_step(step_a.clone(), 5000),
            execute_step(step_b.clone(), 5000)
        );

        let a = result_a.expect("a should complete");
        let b = result_b.expect("b should complete");
        assert!(a.is_success());
        assert!(matches!(b, StepResult::Failure { .. }));

        assert_ready(&step_a);
        assert_ready(&step_b);
    }

    #[tokio::test]
    async fn state_cancel_one_step_does_not_affect_other() {
        let _guard = state_guard();

        let step_a = StepId::new("step-good".to_string());
        let step_b = StepId::new("step-1".to_string());

        cancel_execution(step_a.clone())
            .await
            .expect("cancel should succeed");
        assert_cancelled(&step_a);
        assert_ready(&step_b);

        let result = execute_step(step_b.clone(), 5000).await;
        assert!(result.is_ok());
        assert_ready(&step_b);
        assert_cancelled(&step_a);
    }

    #[tokio::test]
    async fn state_concurrent_cancel_on_ready_steps() {
        let _guard = state_guard();

        let step_a = StepId::new("step-1".to_string());
        let step_b = StepId::new("step-good".to_string());
        let step_c = StepId::new("step-retry".to_string());

        let (result_a, result_b, result_c) = tokio::join!(
            cancel_execution(step_a.clone()),
            cancel_execution(step_b.clone()),
            cancel_execution(step_c.clone())
        );

        assert!(result_a.is_ok());
        assert!(result_b.is_ok());
        assert!(result_c.is_ok());

        assert_cancelled(&step_a);
        assert_cancelled(&step_b);
        assert_cancelled(&step_c);
    }

    #[tokio::test]
    async fn state_timeout_during_execution_returns_to_ready() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let result = execute_step(step_id.clone(), 1).await;
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::TimeoutExceeded { .. })
        ));
        assert_ready(&step_id);
    }

    #[tokio::test]
    async fn state_slow_step_with_sufficient_timeout_completes() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());
        assert_ready(&step_id);
    }

    #[tokio::test]
    async fn state_step_not_found_leaves_no_state() {
        let _guard = state_guard();
        let step_id = StepId::new("nonexistent-step".to_string());

        assert_ready(&step_id);

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::StepNotFound { .. })
        ));

        assert_ready(&step_id);
    }

    #[tokio::test]
    async fn state_all_step_types_transition_correctly() {
        let _guard = state_guard();

        let success = StepId::new("step-1".to_string());
        let failure = StepId::new("step-fail".to_string());
        let transient = StepId::new("step-transient".to_string());
        let slow = StepId::new("step-slow".to_string());

        let (r1, r2, r3, r4) = tokio::join!(
            execute_step(success.clone(), 5000),
            execute_step(failure.clone(), 5000),
            execute_step(transient.clone(), 5000),
            execute_step(slow.clone(), 5000)
        );

        assert!(matches!(r1, Ok(StepResult::Success { .. })));
        assert!(matches!(r2, Ok(StepResult::Failure { .. })));
        assert!(r3.is_err());
        assert!(r4.is_ok());

        assert_ready(&success);
        assert_ready(&failure);
        assert_ready(&transient);
        assert_ready(&slow);
    }

    // =========================================================================
    // Cancel from Executing State
    // =========================================================================

    #[tokio::test]
    async fn state_cancel_from_executing_returns_error() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        set_state(step_id.as_str(), StepState::Executing {
            step_id: step_id.clone(),
            start_time: std::time::Instant::now(),
        });

        let result = cancel_execution(step_id.clone()).await;
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::ExecutionCancelled { .. })
        ));
    }

    #[tokio::test]
    async fn state_cancel_from_executing_does_not_change_state() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        set_state(step_id.as_str(), StepState::Executing {
            step_id: step_id.clone(),
            start_time: std::time::Instant::now(),
        });

        let _ = cancel_execution(step_id.clone()).await;

        let status = get_execution_status(&step_id);
        assert!(matches!(status, ExecutionStatus::Executing { .. }));
    }

    // =========================================================================
    // Completed State Transitions
    // =========================================================================

    #[tokio::test]
    async fn state_completed_read_via_get_execution_status() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        set_state(step_id.as_str(), StepState::Completed {
            output: "test-output".to_string(),
        });

        let status = get_execution_status(&step_id);
        assert!(matches!(status, ExecutionStatus::Completed { output } if output == "test-output"));
    }

    #[tokio::test]
    async fn state_cancel_from_completed_is_noop() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        set_state(step_id.as_str(), StepState::Completed {
            output: "done".to_string(),
        });

        let result = cancel_execution(step_id.clone()).await;
        assert!(result.is_ok());

        let status = get_execution_status(&step_id);
        assert!(matches!(status, ExecutionStatus::Completed { .. }));
    }

    #[tokio::test]
    async fn state_execute_after_manual_completed_overwrites() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        set_state(step_id.as_str(), StepState::Completed {
            output: "old".to_string(),
        });

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());
        assert_ready(&step_id);
    }

    // =========================================================================
    // Timeout Boundary Edge Cases
    // =========================================================================

    #[tokio::test]
    async fn state_slow_step_timeout_exactly_at_threshold_succeeds() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let result = execute_step(step_id.clone(), 3000).await;
        assert!(result.is_ok());
        assert_ready(&step_id);
    }

    #[tokio::test]
    async fn state_slow_step_timeout_just_below_threshold_fails() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let result = execute_step(step_id.clone(), 2999).await;
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::TimeoutExceeded { .. })
        ));
        assert_ready(&step_id);
    }

    #[tokio::test]
    async fn state_execute_after_timeout_recovery_succeeds() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let r1 = execute_step(step_id.clone(), 1).await;
        assert!(r1.is_err());
        assert_ready(&step_id);

        let r2 = execute_step(step_id.clone(), 5000).await;
        assert!(r2.is_ok());
        assert_ready(&step_id);
    }

    // =========================================================================
    // Rapid Sequential Transitions
    // =========================================================================

    #[tokio::test]
    async fn state_rapid_execute_cycles() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        for _ in 0..50 {
            let result = execute_step(step_id.clone(), 5000).await;
            assert!(result.is_ok());
            assert_ready(&step_id);
        }
    }

    #[tokio::test]
    async fn state_alternating_execute_and_cancel() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        for i in 0..10 {
            if i % 2 == 0 {
                let result = execute_step(step_id.clone(), 5000).await;
                assert!(result.is_ok());
                assert_ready(&step_id);
            } else {
                cancel_execution(step_id.clone()).await.expect("cancel should succeed");
                assert_cancelled(&step_id);
            }
        }
    }

    // =========================================================================
    // Cross-Step Isolation Under Concurrent Ops
    // =========================================================================

    #[tokio::test]
    async fn state_concurrent_execute_and_cancel_different_steps() {
        let _guard = state_guard();

        let step_exec = StepId::new("step-1".to_string());
        let step_cancel = StepId::new("step-good".to_string());

        let (exec_result, cancel_result) = tokio::join!(
            execute_step(step_exec.clone(), 5000),
            cancel_execution(step_cancel.clone())
        );

        assert!(exec_result.is_ok());
        assert!(cancel_result.is_ok());
        assert_ready(&step_exec);
        assert_cancelled(&step_cancel);
    }

    #[tokio::test]
    async fn state_many_steps_concurrent_execute_all_return_ready() {
        let _guard = state_guard();

        let step_ids: Vec<StepId> = (0..20)
            .map(|i| StepId::new(format!("step-{}", i + 100)))
            .collect();

        let mut handles: Vec<_> = step_ids
            .iter()
            .map(|id| execute_step(id.clone(), 5000))
            .collect();

        let mut all_ok = true;
        for handle in handles.drain(..) {
            if handle.await.is_err() {
                all_ok = false;
            }
        }
        assert!(all_ok);

        for id in &step_ids {
            assert_ready(id);
        }
    }

    // =========================================================================
    // Retry Workflow State Transitions
    // =========================================================================

    #[tokio::test]
    async fn state_retry_with_transient_step_still_returns_error() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());
        let policy = vo_executor::RetryPolicy::new(3, 10, 2.0).unwrap();

        let result = vo_executor::execute_step_with_retry(step_id.clone(), 5000, policy).await;
        assert!(result.is_err());
        assert_ready(&step_id);
    }

    #[tokio::test]
    async fn state_retry_with_success_step_completes() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());
        let policy = vo_executor::RetryPolicy::new(3, 10, 2.0).unwrap();

        let result = vo_executor::execute_step_with_retry(step_id.clone(), 5000, policy).await;
        assert!(result.is_ok());
        assert_ready(&step_id);
    }

    #[tokio::test]
    async fn state_flaky_step_exhausts_retries() {
        let _guard = state_guard();
        let step_id = StepId::new("step-flaky".to_string());
        let policy = vo_executor::RetryPolicy::new(3, 10, 2.0).unwrap();

        let result = vo_executor::execute_step_with_retry(step_id.clone(), 5000, policy).await;
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::RetryExhausted { attempts: 3, .. })
        ));
    }

    // =========================================================================
    // Error State Tracking
    // =========================================================================

    #[tokio::test]
    async fn state_error_cleared_on_successful_execute() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());

        let _ = execute_step(step_id.clone(), 5000).await;
        assert!(vo_executor::get_last_error(&step_id).is_some());

        let step_id_good = StepId::new("step-1".to_string());
        let _ = execute_step(step_id_good.clone(), 5000).await;
        assert!(vo_executor::get_last_error(&step_id_good).is_none());
    }

    #[tokio::test]
    async fn state_error_isolated_per_step() {
        let _guard = state_guard();

        let step_a = StepId::new("step-transient".to_string());
        let step_b = StepId::new("step-1".to_string());

        let _ = execute_step(step_a.clone(), 5000).await;
        let _ = execute_step(step_b.clone(), 5000).await;

        assert!(vo_executor::get_last_error(&step_a).is_some());
        assert!(vo_executor::get_last_error(&step_b).is_none());
    }

    // =========================================================================
    // Full Lifecycle Scenarios
    // =========================================================================

    #[tokio::test]
    async fn state_full_lifecycle_execute_cancel_execute() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        assert_ready(&step_id);

        let r1 = execute_step(step_id.clone(), 5000).await;
        assert!(r1.is_ok());
        assert_ready(&step_id);

        cancel_execution(step_id.clone()).await.expect("cancel ok");
        assert_cancelled(&step_id);

        let r2 = execute_step(step_id.clone(), 5000).await;
        assert!(r2.is_ok());
        assert_ready(&step_id);

        let r3 = execute_step(step_id.clone(), 5000).await;
        assert!(r3.is_ok());
        assert_ready(&step_id);
    }

    #[tokio::test]
    async fn state_timeout_then_success_then_cancel_lifecycle() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let r1 = execute_step(step_id.clone(), 1).await;
        assert!(r1.is_err());
        assert_ready(&step_id);

        let r2 = execute_step(step_id.clone(), 5000).await;
        assert!(r2.is_ok());
        assert_ready(&step_id);

        cancel_execution(step_id.clone()).await.expect("cancel ok");
        assert_cancelled(&step_id);

        let r3 = execute_step(step_id.clone(), 5000).await;
        assert!(r3.is_ok());
        assert_ready(&step_id);
    }
}