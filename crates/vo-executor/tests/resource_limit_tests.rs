//! Resource limit tests for vo-executor
//!
//! These tests verify enforcement of:
//! - Timeout limits (rejection of invalid timeouts, slow step threshold)
//! - Retry policy limits (max_attempts, backoff capping)
//! - Execution boundaries (state transitions, concurrent limits)

#[cfg(test)]
mod timeout_limit_tests {
    use std::sync::LazyLock;
    use std::sync::Mutex;
    use std::sync::MutexGuard;
    use vo_executor::{
        execute_step, reset_all_state, StepId,
    };

    static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    const SLOW_STEP_DURATION_MS: u64 = 3000;

    fn state_guard() -> MutexGuard<'static, ()> {
        let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_all_state();
        guard
    }

    // =========================================================================
    // Section 1: Invalid Timeout Rejection Tests
    // =========================================================================

    #[tokio::test]
    async fn timeout_zero_rejected_with_invalid_timeout_error() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-1".to_string()), 0).await;
        let err = result.unwrap_err();
        assert!(matches!(err, vo_executor::ExecuteNodeError::InvalidTimeout { value: 0, .. }));
    }

    #[tokio::test]
    async fn timeout_max_u64_rejected_with_invalid_timeout_error() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-1".to_string()), u64::MAX).await;
        let err = result.unwrap_err();
        assert!(matches!(err, vo_executor::ExecuteNodeError::InvalidTimeout { value: u64::MAX, .. }));
    }

    #[tokio::test]
    async fn timeout_rejected_error_contains_reason() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-1".to_string()), 0).await;
        let err = result.unwrap_err();
        let err_str = err.to_string();
        assert!(err_str.contains("must be > 0ms"));
    }

    // =========================================================================
    // Section 2: Slow Step Threshold Tests
    // =========================================================================

    #[tokio::test]
    async fn slow_step_threshold_is_3000ms() {
        assert_eq!(SLOW_STEP_DURATION_MS, 3000);
    }

    #[tokio::test]
    async fn slow_step_with_timeout_below_threshold_times_out() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-slow".to_string()), 100).await;
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::TimeoutExceeded { elapsed_ms: 3000, limit_ms: 100 })
        ));
    }

    #[tokio::test]
    async fn slow_step_with_timeout_equal_to_threshold_succeeds() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-slow".to_string()), 3000).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn slow_step_with_timeout_above_threshold_succeeds() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-slow".to_string()), 5000).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn non_slow_step_with_small_timeout_succeeds() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-1".to_string()), 1).await;
        assert!(result.is_ok());
    }

    // =========================================================================
    // Section 3: Timeout Boundary Tests
    // =========================================================================

    #[tokio::test]
    async fn timeout_boundary_one_less_than_slow_threshold_fails() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-slow".to_string()), 2999).await;
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::TimeoutExceeded { elapsed_ms: 3000, limit_ms: 2999 })
        ));
    }

    #[tokio::test]
    async fn timeout_boundary_one_more_than_slow_threshold_succeeds() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-slow".to_string()), 3001).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn timeout_at_exactly_one_ms_triggers_for_slow_step() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-slow".to_string()), 1).await;
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::TimeoutExceeded { limit_ms: 1, .. })
        ));
    }
}

#[cfg(test)]
mod retry_policy_limit_tests {
    use std::sync::LazyLock;
    use std::sync::Mutex;
    use std::sync::MutexGuard;
    use vo_executor::{
        execute_step_with_retry, reset_all_state, RetryPolicy, StepId,
    };

    static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn state_guard() -> MutexGuard<'static, ()> {
        let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_all_state();
        guard
    }

    // =========================================================================
    // Section 4: Retry max_attempts Limit Tests
    // =========================================================================

    #[tokio::test]
    async fn retry_with_zero_attempts_rejected() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(0, 100, 2.0);
        assert!(policy.is_err());
        assert!(matches!(
            policy.unwrap_err(),
            vo_executor::RetryPolicyError::ZeroAttempts
        ));
    }

    #[tokio::test]
    async fn retry_with_one_attempt_fails_fast() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(1, 1000, 2.0).unwrap();
        let start = std::time::Instant::now();
        let result = execute_step_with_retry(StepId::new("step-flaky".to_string()), 5000, policy).await;
        let elapsed_ms = start.elapsed().as_millis() as u64;
        assert!(elapsed_ms < 100, "max_attempts=1 should fail without sleeping, got {}ms", elapsed_ms);
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::RetryExhausted { attempts: 1, .. })
        ));
    }

    #[tokio::test]
    async fn retry_with_two_attempts_sleeps_once() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(2, 100, 2.0).unwrap();
        let start = std::time::Instant::now();
        let result = execute_step_with_retry(StepId::new("step-flaky".to_string()), 5000, policy).await;
        let elapsed_ms = start.elapsed().as_millis() as u64;
        assert!((80..300).contains(&elapsed_ms), "Expected ~100ms sleep, got {}ms", elapsed_ms);
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::RetryExhausted { attempts: 2, .. })
        ));
    }

    #[tokio::test]
    async fn retry_exhausted_error_contains_attempt_count() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();
        let result = execute_step_with_retry(StepId::new("step-flaky".to_string()), 5000, policy).await;
        match result {
            Err(vo_executor::ExecuteNodeError::RetryExhausted { attempts, .. }) => {
                assert_eq!(attempts, 3);
            }
            _ => panic!("Expected RetryExhausted"),
        }
    }

    // =========================================================================
    // Section 5: Backoff Cap Tests
    // =========================================================================

    #[tokio::test]
    async fn backoff_capped_at_max_backoff_ms() {
        let _guard = state_guard();
        let policy = RetryPolicy::with_max_backoff(5, 1000, 10.0, 5000).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 1000);
        assert_eq!(policy.calculate_backoff_delay(2), 5000);
        assert_eq!(policy.calculate_backoff_delay(3), 5000);
    }

    #[tokio::test]
    async fn zero_backoff_ms_results_in_no_delay() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(3, 0, 2.0).unwrap();
        let start = std::time::Instant::now();
        let _result = execute_step_with_retry(StepId::new("step-flaky".to_string()), 5000, policy).await;
        let elapsed_ms = start.elapsed().as_millis() as u64;
        assert!(elapsed_ms < 50, "Zero backoff should have minimal delay, got {}ms", elapsed_ms);
    }

    #[tokio::test]
    async fn backoff_multiplier_exponential_growth() {
        let policy = RetryPolicy::new(5, 100, 2.0).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 100);
        assert_eq!(policy.calculate_backoff_delay(2), 200);
        assert_eq!(policy.calculate_backoff_delay(3), 400);
        assert_eq!(policy.calculate_backoff_delay(4), 800);
        assert_eq!(policy.calculate_backoff_delay(5), 1600);
    }

    #[tokio::test]
    async fn backoff_multiplier_one_results_in_constant_delay() {
        let policy = RetryPolicy::new(5, 100, 1.0).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 100);
        assert_eq!(policy.calculate_backoff_delay(5), 100);
    }

    #[tokio::test]
    async fn backoff_calculation_attempt_zero_returns_zero() {
        let policy = RetryPolicy::new(3, 100, 2.0).unwrap();
        assert_eq!(policy.calculate_backoff_delay(0), 0);
    }

    // =========================================================================
    // Section 6: Retry Policy Validation Tests
    // =========================================================================

    #[tokio::test]
    async fn retry_policy_nan_multiplier_rejected() {
        let result = RetryPolicy::new(3, 100, f64::NAN);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            vo_executor::RetryPolicyError::InvalidMultiplier { .. }
        ));
    }

    #[tokio::test]
    async fn retry_policy_infinity_multiplier_rejected() {
        let result = RetryPolicy::new(3, 100, f64::INFINITY);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn retry_policy_multiplier_below_one_rejected() {
        let result = RetryPolicy::new(3, 100, 0.99);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn retry_policy_multiplier_exactly_one_accepted() {
        let result = RetryPolicy::new(3, 100, 1.0);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn retry_policy_with_max_backoff_validates_max_greater_than_backoff() {
        let result = RetryPolicy::with_max_backoff(3, 100, 2.0, 50);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            vo_executor::RetryPolicyError::MaxBackoffTooSmall { max: 50, ms: 100 }
        ));
    }

    #[tokio::test]
    async fn retry_policy_with_equal_max_and_backoff_accepted() {
        let result = RetryPolicy::with_max_backoff(3, 100, 2.0, 100);
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod execution_boundary_tests {
    use std::sync::LazyLock;
    use std::sync::Mutex;
    use std::sync::MutexGuard;
    use vo_executor::{
        cancel_execution, execute_step, get_execution_status, reset_all_state,
        StepId,
    };

    static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn state_guard() -> MutexGuard<'static, ()> {
        let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_all_state();
        guard
    }

    // =========================================================================
    // Section 7: State Transition Boundary Tests
    // =========================================================================

    #[tokio::test]
    async fn successful_step_returns_to_ready_state() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());
        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());
        let status = get_execution_status(&step_id);
        assert!(matches!(status, vo_executor::ExecutionStatus::Ready));
    }

    #[tokio::test]
    async fn failed_step_returns_to_ready_state() {
        let _guard = state_guard();
        let step_id = StepId::new("step-fail".to_string());
        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());
        let status = get_execution_status(&step_id);
        assert!(matches!(status, vo_executor::ExecutionStatus::Ready));
    }

    #[tokio::test]
    async fn cancelled_state_is_terminal() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());
        cancel_execution(step_id.clone()).await.expect("cancel should succeed");
        let status = get_execution_status(&step_id);
        assert!(matches!(status, vo_executor::ExecutionStatus::Cancelled { .. }));
    }

    #[tokio::test]
    async fn cancelled_execution_is_idempotent() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());
        cancel_execution(step_id.clone()).await.expect("first cancel should succeed");
        cancel_execution(step_id.clone()).await.expect("second cancel should succeed");
        let status = get_execution_status(&step_id);
        assert!(matches!(status, vo_executor::ExecutionStatus::Cancelled { .. }));
    }

    #[tokio::test]
    async fn completed_execution_is_idempotent() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());
        execute_step(step_id.clone(), 5000).await.expect("first exec should succeed");
        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod error_limit_enforcement_tests {
    use std::sync::LazyLock;
    use std::sync::Mutex;
    use std::sync::MutexGuard;
    use vo_executor::{
        execute_step, execute_step_with_retry, get_last_error, reset_all_state,
        RetryPolicy, StepId,
    };

    static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn state_guard() -> MutexGuard<'static, ()> {
        let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_all_state();
        guard
    }

    // =========================================================================
    // Section 8: Error Propagation Limit Tests
    // =========================================================================

    #[tokio::test]
    async fn transient_error_contains_recoverable_flag() {
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
        let result = execute_step_with_retry(StepId::new("step-flaky".to_string()), 5000, policy).await;
        match result {
            Err(vo_executor::ExecuteNodeError::RetryExhausted { last_error, .. }) => {
                assert!(matches!(*last_error, vo_executor::ExecuteNodeError::TransientError { .. }));
            }
            _ => panic!("Expected RetryExhausted"),
        }
    }

    #[tokio::test]
    async fn last_error_cleared_after_successful_execution() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());
        execute_step(step_id.clone(), 5000).await.expect_err("transient should fail");
        assert!(get_last_error(&step_id).is_some());
        execute_step(StepId::new("step-1".to_string()), 5000).await.expect("success should clear");
    }

    #[tokio::test]
    async fn invalid_timeout_is_terminal_not_retried() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();
        let result = execute_step_with_retry(StepId::new("step-1".to_string()), 0, policy).await;
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::InvalidTimeout { .. })
        ));
    }

    #[tokio::test]
    async fn step_not_found_is_terminal_not_retried() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();
        let result = execute_step_with_retry(StepId::new("nonexistent-step".to_string()), 5000, policy).await;
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::StepNotFound { .. })
        ));
    }
}