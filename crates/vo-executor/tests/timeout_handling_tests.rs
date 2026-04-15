//! Timeout handling and recovery tests for vo-executor
//!
//! Tests cover:
//! - Timeout validation edge cases
//! - Slow step timeout detection and enforcement
//! - Step state after timeout
//! - Recovery and re-execution after timeout
//! - Timeout with retry policy interaction

use vo_executor::{
    cancel_execution, execute_step, execute_step_with_retry,
    get_execution_status, get_last_error,
    ExecutionStatus, RetryPolicy, StepId,
};
use vo_executor::errors::ExecuteNodeError;
use vo_executor::state::{get_state, reset_all_state, set_state, set_executing_state_for_test, StepState};

const SLOW_STEP_DURATION_MS: u64 = 3000;

use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::MutexGuard;

static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn setup() -> MutexGuard<'static, ()> {
    let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset_all_state();
    guard
}

mod timeout_validation {
    use super::*;

    #[tokio::test]
    async fn timeout_zero_rejected() {
        let _guard = setup();
        let result = execute_step(StepId::new("step-1".to_string()), 0).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ExecuteNodeError::InvalidTimeout { value: 0, .. }));
    }

    #[tokio::test]
    async fn timeout_max_rejected() {
        let _guard = setup();
        let result = execute_step(StepId::new("step-1".to_string()), u64::MAX).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ExecuteNodeError::InvalidTimeout { value: u64::MAX, .. }));
    }

    #[tokio::test]
    async fn timeout_one_is_valid() {
        let _guard = setup();
        let result = execute_step(StepId::new("step-1".to_string()), 1).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn timeout_u64_max_minus_one_is_valid() {
        let _guard = setup();
        let result = execute_step(StepId::new("step-1".to_string()), u64::MAX - 1).await;
        assert!(result.is_ok());
    }
}

mod slow_step_timeout {
    use super::*;

    #[tokio::test]
    async fn slow_step_with_sufficient_timeout_succeeds() {
        let _guard = setup();
        let timeout = SLOW_STEP_DURATION_MS + 100;
        let result = execute_step(StepId::new("step-slow".to_string()), timeout).await;
        assert!(result.is_ok());
        let step_result = result.unwrap();
        assert!(step_result.is_success());
    }

    #[tokio::test]
    async fn slow_step_with_exactly_threshold_timeout_succeeds() {
        let _guard = setup();
        let timeout = SLOW_STEP_DURATION_MS;
        let result = execute_step(StepId::new("step-slow".to_string()), timeout).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn slow_step_with_insufficient_timeout_fails() {
        let _guard = setup();
        let timeout = SLOW_STEP_DURATION_MS - 1;
        let result = execute_step(StepId::new("step-slow".to_string()), timeout).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ExecuteNodeError::TimeoutExceeded { elapsed_ms: 3000, limit_ms } if limit_ms < 3000));
    }

    #[tokio::test]
    async fn slow_step_timeout_error_contains_correct_values() {
        let _guard = setup();
        let timeout = 100;
        let result = execute_step(StepId::new("step-slow".to_string()), timeout).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        let err_string = err.to_string();
        assert!(err_string.contains("3000"));
        assert!(err_string.contains("100"));
    }

    #[tokio::test]
    async fn non_slow_step_with_small_timeout_succeeds() {
        let _guard = setup();
        let result = execute_step(StepId::new("step-1".to_string()), 1).await;
        assert!(result.is_ok());
    }
}

mod state_after_timeout {
    use super::*;

    #[tokio::test]
    async fn state_is_ready_after_slow_step_timeout() {
        let _guard = setup();
        let timeout = 100;
        let _ = execute_step(StepId::new("step-slow".to_string()), timeout).await;
        let state = get_state("step-slow");
        assert!(matches!(state, StepState::Ready));
    }

    #[tokio::test]
    async fn state_is_ready_after_successful_execution() {
        let _guard = setup();
        let _ = execute_step(StepId::new("step-1".to_string()), 5000).await;
        let state = get_state("step-1");
        assert!(matches!(state, StepState::Ready));
    }

    #[tokio::test]
    async fn execution_status_ready_after_timeout() {
        let _guard = setup();
        let _ = execute_step(StepId::new("step-slow".to_string()), 100).await;
        let status = get_execution_status(&StepId::new("step-slow".to_string()));
        assert!(matches!(status, ExecutionStatus::Ready));
    }
}

mod recovery_after_timeout {
    use super::*;

    #[tokio::test]
    async fn can_retry_step_after_timeout() {
        let _guard = setup();
        let timeout = 100;
        let result1 = execute_step(StepId::new("step-slow".to_string()), timeout).await;
        assert!(result1.is_err());
        assert!(matches!(result1.unwrap_err(), ExecuteNodeError::TimeoutExceeded { .. }));

        let result2 = execute_step(StepId::new("step-slow".to_string()), 5000).await;
        assert!(result2.is_ok());
        assert!(result2.unwrap().is_success());
    }

    #[tokio::test]
    async fn timeout_does_not_set_last_error() {
        let _guard = setup();
        let _ = execute_step(StepId::new("step-slow".to_string()), 100).await;
        assert!(get_last_error(&StepId::new("step-slow".to_string())).is_none());
    }

    #[tokio::test]
    async fn transient_step_sets_last_error() {
        let _guard = setup();
        let _ = execute_step(StepId::new("step-transient".to_string()), 5000).await;
        assert!(get_last_error(&StepId::new("step-transient".to_string())).is_some());
    }
}

mod timeout_with_retry {
    use super::*;

    #[tokio::test]
    async fn retry_with_insufficient_timeout_times_out() {
        let _guard = setup();
        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();
        let result = execute_step_with_retry(
            StepId::new("step-slow".to_string()),
            100,
            policy,
        ).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ExecuteNodeError::TimeoutExceeded { .. }));
    }

    #[tokio::test]
    async fn retry_with_sufficient_timeout_succeeds() {
        let _guard = setup();
        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();
        let result = execute_step_with_retry(
            StepId::new("step-1".to_string()),
            5000,
            policy,
        ).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn flaky_step_exhausts_retries_regardless_of_timeout() {
        let _guard = setup();
        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();
        let result = execute_step_with_retry(
            StepId::new("step-flaky".to_string()),
            5000,
            policy,
        ).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ExecuteNodeError::RetryExhausted { attempts: 3, .. }));
    }

    #[tokio::test]
    async fn retry_exhausted_error_contains_last_error() {
        let _guard = setup();
        let policy = RetryPolicy::new(2, 10, 2.0).unwrap();
        let result = execute_step_with_retry(
            StepId::new("step-flaky".to_string()),
            5000,
            policy,
        ).await;
        assert!(result.is_err());
        if let ExecuteNodeError::RetryExhausted { last_error, .. } = result.unwrap_err() {
            assert!(matches!(*last_error, ExecuteNodeError::TransientError { .. }));
        } else {
            panic!("Expected RetryExhausted error");
        }
    }
}

mod cancel_during_execution {
    use super::*;

    #[tokio::test]
    async fn cancel_returns_error_for_executing_step() {
        let _guard = setup();
        set_executing_state_for_test("step-1");
        let result = cancel_execution(StepId::new("step-1".to_string())).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ExecuteNodeError::ExecutionCancelled { .. }));
    }

    #[tokio::test]
    async fn cancel_from_executing_does_not_change_state() {
        let _guard = setup();
        set_executing_state_for_test("step-1");
        let _ = cancel_execution(StepId::new("step-1".to_string())).await;
        let status = get_execution_status(&StepId::new("step-1".to_string()));
        assert!(matches!(status, ExecutionStatus::Executing { .. }));
    }

    #[tokio::test]
    async fn cancel_from_ready_succeeds() {
        let _guard = setup();
        let result = cancel_execution(StepId::new("step-1".to_string())).await;
        assert!(result.is_ok());
        let status = get_execution_status(&StepId::new("step-1".to_string()));
        assert!(matches!(status, ExecutionStatus::Cancelled { .. }));
    }

    #[tokio::test]
    async fn cancel_from_cancelled_is_noop() {
        let _guard = setup();
        set_state("step-1", StepState::Cancelled { reason: "first".to_string() });
        let result = cancel_execution(StepId::new("step-1".to_string())).await;
        assert!(result.is_ok());
    }
}

mod concurrent_execution_prevention {
    use super::*;

    #[tokio::test]
    async fn cannot_execute_step_while_executing() {
        let _guard = setup();
        set_executing_state_for_test("step-1");
        let result = execute_step(StepId::new("step-1".to_string()), 5000).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ExecuteNodeError::InvalidTransition { .. }));
    }

    #[tokio::test]
    async fn invalid_transition_contains_correct_state() {
        let _guard = setup();
        set_executing_state_for_test("step-1");
        let result = execute_step(StepId::new("step-1".to_string()), 5000).await;
        let err = result.unwrap_err();
        let err_string = err.to_string();
        assert!(err_string.contains("Executing"));
        assert!(err_string.contains("execute_step"));
    }

    #[tokio::test]
    async fn can_execute_after_cancellation() {
        let _guard = setup();
        let _ = cancel_execution(StepId::new("step-1".to_string())).await;
        let result = execute_step(StepId::new("step-1".to_string()), 5000).await;
        assert!(result.is_ok());
    }
}

mod timeout_error_display {
    use vo_executor::errors::ExecuteNodeError;

    #[test]
    fn timeout_exceeded_display_format() {
        let err = ExecuteNodeError::TimeoutExceeded {
            elapsed_ms: 5000,
            limit_ms: 3000,
        };
        let msg = err.to_string();
        assert!(msg.contains("5000"));
        assert!(msg.contains("3000"));
        assert!(msg.contains("Timeout exceeded"));
    }

    #[test]
    fn invalid_timeout_display_shows_value_and_reason() {
        let err = ExecuteNodeError::InvalidTimeout {
            value: 0,
            reason: "must be > 0ms".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("0"));
        assert!(msg.contains("must be > 0ms"));
        assert!(msg.contains("Invalid timeout"));
    }

    #[test]
    fn timeout_exceeded_error_equality() {
        let err1 = ExecuteNodeError::TimeoutExceeded {
            elapsed_ms: 100,
            limit_ms: 50,
        };
        let err2 = ExecuteNodeError::TimeoutExceeded {
            elapsed_ms: 100,
            limit_ms: 50,
        };
        let err3 = ExecuteNodeError::TimeoutExceeded {
            elapsed_ms: 200,
            limit_ms: 50,
        };
        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }
}