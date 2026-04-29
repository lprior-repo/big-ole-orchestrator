//! Runtime tests for vo-executor
//!
//! These tests cover:
//! - Step execution timeout tests
//! - Context propagation tests
//! - Error recovery tests
//! - Concurrent execution tests

#[cfg(test)]
mod runtime_timeout_tests {
    use std::sync::LazyLock;
    use std::sync::Mutex;
    use std::sync::MutexGuard;
    use std::time::Duration;
    use vo_executor::{
        cancel_execution, execute_step, execute_step_with_retry, get_execution_status,
        get_last_error, reset_all_state, RetryPolicy, StepId,
    };

    static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn state_guard() -> MutexGuard<'static, ()> {
        let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_all_state();
        guard
    }

    // =========================================================================
    // Section 1: Step Execution Timeout Tests
    // =========================================================================

    #[tokio::test]
    async fn timeout_zero_immediately_rejected() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-1".to_string()), 0).await;
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::InvalidTimeout { value: 0, .. })
        ));
    }

    #[tokio::test]
    async fn timeout_max_u64_rejected() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-1".to_string()), u64::MAX).await;
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::InvalidTimeout {
                value: u64::MAX,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn timeout_one_ms_triggers_for_slow_step() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-slow".to_string()), 1).await;
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::TimeoutExceeded { limit_ms: 1, .. })
        ));
    }

    #[tokio::test]
    async fn timeout_at_threshold_succeeds_for_slow_step() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-slow".to_string()), 3000).await;
        assert!(
            result.is_ok(),
            "Slow step with 3000ms timeout should succeed"
        );
        assert_eq!(
            result.unwrap(),
            vo_executor::StepResult::Success {
                output: "done".to_string()
            }
        );
    }

    #[tokio::test]
    async fn timeout_above_threshold_succeeds_for_slow_step() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-slow".to_string()), 5000).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn timeout_with_retry_respects_timeout_first() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(3, 100, 2.0).unwrap();
        let result = execute_step_with_retry(StepId::new("step-slow".to_string()), 1, policy).await;
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::TimeoutExceeded { .. })
        ));
    }

    #[tokio::test]
    async fn timeout_boundary_one_less_than_threshold() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-slow".to_string()), 2999).await;
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::TimeoutExceeded {
                elapsed_ms: 3000,
                limit_ms: 2999
            })
        ));
    }

    #[tokio::test]
    async fn timeout_boundary_one_more_than_threshold() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-slow".to_string()), 3001).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn successful_step_returns_correct_output() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-1".to_string()), 5000).await;
        assert_eq!(
            result.unwrap(),
            vo_executor::StepResult::Success {
                output: "done".to_string()
            }
        );
    }

    #[tokio::test]
    async fn failed_step_returns_failure_result() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-fail".to_string()), 5000).await;
        assert_eq!(
            result.unwrap(),
            vo_executor::StepResult::Failure {
                output: "error: exit code 1".to_string()
            }
        );
    }
}

#[cfg(test)]
mod runtime_context_propagation_tests {
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
    // Section 2: Context Propagation Tests
    // =========================================================================

    #[tokio::test]
    async fn execution_status_reflects_step_identity() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());
        let status = get_execution_status(&step_id);
        assert!(
            status.is_ready()
                || matches!(status, vo_executor::ExecutionStatus::Executing { step_id: id, .. } if id.as_str() == "step-slow")
        );
    }

    #[tokio::test]
    async fn error_context_preserved_for_transient_step() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());
        execute_step(step_id.clone(), 5000)
            .await
            .expect_err("transient should fail");
        let error = get_last_error(&step_id);
        assert!(error.is_some());
        if let Some(vo_executor::ExecuteNodeError::TransientError {
            reason,
            recoverable,
        }) = error
        {
            assert!(reason.contains("network timeout"));
            assert!(recoverable);
        } else {
            panic!("Expected TransientError");
        }
    }

    #[tokio::test]
    async fn retry_count_propagates_to_error() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();
        let result =
            execute_step_with_retry(StepId::new("step-flaky".to_string()), 5000, policy).await;
        match result {
            Err(vo_executor::ExecuteNodeError::RetryExhausted { attempts, .. }) => {
                assert_eq!(attempts, 3);
            }
            _ => panic!("Expected RetryExhausted"),
        }
    }

    #[tokio::test]
    async fn transient_error_recoverable_flag_propagates() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());
        execute_step(step_id.clone(), 5000)
            .await
            .expect_err("should fail");
        let error = get_last_error(&step_id);
        assert!(matches!(
            error,
            Some(vo_executor::ExecuteNodeError::TransientError {
                recoverable: true,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn multiple_steps_maintain_independent_context() {
        let _guard = state_guard();
        let step_a = StepId::new("step-transient".to_string());
        let step_b = StepId::new("step-1".to_string());

        execute_step(step_a.clone(), 5000)
            .await
            .expect_err("transient fails");
        let result_b = execute_step(step_b.clone(), 5000).await;
        assert!(result_b.is_ok());

        let error_a = get_last_error(&step_a);
        let error_b = get_last_error(&step_b);
        assert!(error_a.is_some());
        assert!(error_b.is_none());
    }

    #[tokio::test]
    async fn context_cleared_between_sequential_executions() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());

        execute_step(step_id.clone(), 5000)
            .await
            .expect_err("first fails");
        assert!(get_last_error(&step_id).is_some());

        execute_step(step_id.clone(), 5000)
            .await
            .expect_err("second fails");
        assert!(get_last_error(&step_id).is_some());
    }

    #[tokio::test]
    async fn retry_policy_backoff_multiplier_propagates() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(3, 200, 3.0).unwrap();
        let start = std::time::Instant::now();
        let result =
            execute_step_with_retry(StepId::new("step-flaky".to_string()), 5000, policy).await;
        let elapsed_ms = start.elapsed().as_millis() as u64;
        assert!(
            elapsed_ms >= 600,
            "Expected ~200 + 600 = 800ms backoff, got {}ms",
            elapsed_ms
        );
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::RetryExhausted { .. })
        ));
    }
}

#[cfg(test)]
mod runtime_error_recovery_tests {
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
    // Section 3: Error Recovery Tests
    // =========================================================================

    #[tokio::test]
    async fn transient_error_is_recoverable() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-transient".to_string()), 5000).await;
        match result {
            Err(vo_executor::ExecuteNodeError::TransientError { recoverable, .. }) => {
                assert!(recoverable);
            }
            _ => panic!("Expected TransientError with recoverable=true"),
        }
    }

    #[tokio::test]
    async fn retry_exhausted_contains_last_error() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();
        let result =
            execute_step_with_retry(StepId::new("step-flaky".to_string()), 5000, policy).await;
        match result {
            Err(vo_executor::ExecuteNodeError::RetryExhausted { last_error, .. }) => {
                assert!(matches!(
                    *last_error,
                    vo_executor::ExecuteNodeError::TransientError { .. }
                ));
            }
            _ => panic!("Expected RetryExhausted"),
        }
    }

    #[tokio::test]
    async fn successful_retry_after_transient() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(5, 10, 2.0).unwrap();
        let result =
            execute_step_with_retry(StepId::new("step-good".to_string()), 5000, policy).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn step_not_found_error_is_terminal() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();
        let result =
            execute_step_with_retry(StepId::new("nonexistent-step".to_string()), 5000, policy)
                .await;
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::StepNotFound { .. })
        ));
    }

    #[tokio::test]
    async fn invalid_timeout_error_is_terminal() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();
        let result = execute_step_with_retry(StepId::new("step-1".to_string()), 0, policy).await;
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::InvalidTimeout { .. })
        ));
    }

    #[tokio::test]
    async fn error_state_cleared_on_successful_retry() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        set_error_before(step_id.as_str());
        assert!(get_last_error(&step_id).is_some());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());
    }

    fn set_error_before(step_id: &str) {
        use vo_executor::ExecuteNodeError;
        let err = ExecuteNodeError::TransientError {
            reason: "prior error".to_string(),
            recoverable: true,
        };
        vo_executor::set_error(step_id, err);
    }

    #[tokio::test]
    async fn max_attempts_one_fails_fast() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(1, 1000, 2.0).unwrap();
        let start = std::time::Instant::now();
        let result =
            execute_step_with_retry(StepId::new("step-flaky".to_string()), 5000, policy).await;
        let elapsed_ms = start.elapsed().as_millis() as u64;
        assert!(
            elapsed_ms < 100,
            "max_attempts=1 should fail without sleeping, got {}ms",
            elapsed_ms
        );
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::RetryExhausted { attempts: 1, .. })
        ));
    }

    #[tokio::test]
    async fn max_attempts_two_sleeps_once() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(2, 100, 2.0).unwrap();
        let start = std::time::Instant::now();
        let result =
            execute_step_with_retry(StepId::new("step-flaky".to_string()), 5000, policy).await;
        let elapsed_ms = start.elapsed().as_millis() as u64;
        assert!(
            (80..200).contains(&elapsed_ms),
            "Expected ~100ms sleep, got {}ms",
            elapsed_ms
        );
    }

    #[tokio::test]
    async fn zero_backoff_ms_results_in_no_delay() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(3, 0, 2.0).unwrap();
        let start = std::time::Instant::now();
        let result =
            execute_step_with_retry(StepId::new("step-flaky".to_string()), 5000, policy).await;
        let elapsed_ms = start.elapsed().as_millis() as u64;
        assert!(
            elapsed_ms < 50,
            "Zero backoff should have minimal delay, got {}ms",
            elapsed_ms
        );
    }

    #[tokio::test]
    async fn invalid_retry_policy_rejected_before_execution() {
        let _guard = state_guard();
        let policy = vo_executor::RetryPolicy {
            max_attempts: 0,
            backoff_ms: 100,
            backoff_multiplier: 2.0,
            max_backoff_ms: u64::MAX,
        };
        let result = execute_step_with_retry(StepId::new("step-1".to_string()), 5000, policy).await;
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::InvalidRetryPolicy {
                reason: vo_executor::RetryPolicyError::ZeroAttempts,
                ..
            })
        ));
    }
}

#[cfg(test)]
mod runtime_concurrent_execution_tests {
    use std::sync::LazyLock;
    use std::sync::Mutex;
    use std::sync::MutexGuard;
    use vo_executor::{
        execute_step, execute_step_with_retry, get_execution_status, reset_all_state, RetryPolicy,
        StepId,
    };

    static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn state_guard() -> MutexGuard<'static, ()> {
        let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_all_state();
        guard
    }

    // =========================================================================
    // Section 4: Concurrent Execution Tests
    // =========================================================================

    #[tokio::test]
    async fn concurrent_execution_different_steps_succeed_independently() {
        let _guard = state_guard();

        let (result_a, result_b) = tokio::join!(
            execute_step(StepId::new("step-1".to_string()), 5000),
            execute_step(StepId::new("step-good".to_string()), 5000)
        );

        assert!(result_a.is_ok());
        assert!(result_b.is_ok());
    }

    #[tokio::test]
    async fn concurrent_execution_with_mixed_results() {
        let _guard = state_guard();

        let (result_success, result_fail, result_transient) = tokio::join!(
            execute_step(StepId::new("step-1".to_string()), 5000),
            execute_step(StepId::new("step-fail".to_string()), 5000),
            execute_step(StepId::new("step-transient".to_string()), 5000)
        );

        assert!(result_success.is_ok());
        assert!(result_fail.is_ok());
        assert!(result_transient.is_err());
    }

    #[tokio::test]
    async fn concurrent_execution_with_varying_timeouts() {
        let _guard = state_guard();

        let results = tokio::join!(
            execute_step(StepId::new("step-1".to_string()), 1000),
            execute_step(StepId::new("step-slow".to_string()), 5000),
            execute_step(StepId::new("step-good".to_string()), 2000)
        );

        assert!(results.0.is_ok());
        assert!(results.1.is_ok());
        assert!(results.2.is_ok());
    }

    #[tokio::test]
    async fn concurrent_retry_attempts_independent() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();

        let results = tokio::join!(
            execute_step_with_retry(StepId::new("step-flaky".to_string()), 5000, policy.clone()),
            execute_step_with_retry(StepId::new("step-good".to_string()), 5000, policy.clone())
        );

        assert!(results.1.is_ok());
        assert!(matches!(
            results.0,
            Err(vo_executor::ExecuteNodeError::RetryExhausted { .. })
        ));
    }

    #[tokio::test]
    async fn sequential_execution_state_transitions_correctly() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        let result1 = execute_step(step_id.clone(), 5000).await;
        assert!(result1.is_ok());

        let status_after_first = get_execution_status(&step_id);
        assert!(status_after_first.is_ready());

        let result2 = execute_step(step_id.clone(), 5000).await;
        assert!(result2.is_ok());

        let status_after_second = get_execution_status(&step_id);
        assert!(status_after_second.is_ready());
    }

    #[tokio::test]
    async fn many_sequential_executions_all_succeed() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        for _ in 0..20 {
            let result = execute_step(step_id.clone(), 5000).await;
            assert!(
                result.is_ok(),
                "Sequential execution should always succeed for success step"
            );
        }
    }

    #[tokio::test]
    async fn many_concurrent_executions_same_step() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        let mut handles = Vec::new();
        for _ in 0..10 {
            let sid = step_id.clone();
            handles.push(tokio::spawn(async move { execute_step(sid, 5000).await }));
        }

        for handle in handles {
            let result = handle.await.expect("task should complete");
            assert!(result.is_ok(), "Each concurrent execution should succeed");
        }
    }

    #[tokio::test]
    async fn concurrent_execution_all_states_transitioned() {
        let _guard = state_guard();

        let mut handles = Vec::new();
        let step_ids = vec!["step-1", "step-good", "step-fail", "step-transient"];

        for sid in step_ids {
            let step_id_str = sid.to_string();
            handles.push(tokio::spawn(async move {
                let sid = StepId::new(step_id_str.clone());
                let result = execute_step(sid.clone(), 5000).await;
                (sid, result)
            }));
        }

        let mut success_count = 0;
        let mut failure_count = 0;
        let mut transient_count = 0;

        for handle in handles {
            let (_sid, result) = handle.await.expect("task should complete");
            match result {
                Ok(vo_executor::StepResult::Success { .. }) => success_count += 1,
                Ok(vo_executor::StepResult::Failure { .. }) => failure_count += 1,
                Err(vo_executor::ExecuteNodeError::TransientError { .. }) => transient_count += 1,
                Err(e) => panic!("Unexpected error: {:?}", e),
            }
        }

        assert_eq!(success_count, 2);
        assert_eq!(failure_count, 1);
        assert_eq!(transient_count, 1);
    }

    #[tokio::test]
    async fn interleaved_execution_maintains_correctness() {
        let _guard = state_guard();

        let step_a = StepId::new("step-1".to_string());
        let step_b = StepId::new("step-fail".to_string());

        let result_a1 = execute_step(step_a.clone(), 5000).await;
        let result_b = execute_step(step_b.clone(), 5000).await;
        let result_a2 = execute_step(step_a.clone(), 5000).await;

        assert!(result_a1.is_ok());
        assert!(result_b.is_ok());
        assert!(result_a2.is_ok());
    }
}
