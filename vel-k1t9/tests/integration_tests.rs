// Integration tests for vel-k1t9
// These tests verify the actual async function implementations

#[cfg(test)]
mod integration_tests {
    use vel_k1t9::{
        cancel_execution, execute_step, execute_step_with_retry,
        get_execution_status, get_last_error, RetryPolicy,
    };

    #[tokio::test]
    async fn execute_step_rejects_zero_timeout() {
        let result = execute_step("step-zero-timeout".to_string(), 0).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, vel_k1t9::ExecuteNodeError::InvalidTimeout { .. }));
    }

    #[tokio::test]
    async fn execute_step_success_for_valid_step() {
        let result = execute_step("step-valid".to_string(), 5000).await;
        assert!(result.is_ok());
        let step_result = result.unwrap();
        assert!(step_result.is_success());
    }

    #[tokio::test]
    async fn execute_step_timeout_for_slow_step() {
        let result = execute_step("step-slow".to_string(), 1).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, vel_k1t9::ExecuteNodeError::TimeoutExceeded { .. }));
    }

    #[tokio::test]
    async fn execute_step_not_found_for_unknown_step() {
        let result = execute_step("unknown-step-xyz".to_string(), 5000).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, vel_k1t9::ExecuteNodeError::StepNotFound { .. }));
    }

    #[tokio::test]
    async fn execute_step_with_retry_success() {
        let policy = RetryPolicy::new(3, 100, 2.0).unwrap();
        let result = execute_step_with_retry("step-retry".to_string(), 5000, policy).await;
        assert!(result.is_ok());
        let step_result = result.unwrap();
        assert!(step_result.is_success());
    }

    #[tokio::test]
    async fn get_execution_status_returns_ready_for_new_step() {
        let status = get_execution_status("step-new-123");
        assert!(status.is_ready());
    }

    #[tokio::test]
    async fn get_last_error_returns_none_for_new_step() {
        let error = get_last_error("step-new-456");
        assert!(error.is_none());
    }

    #[tokio::test]
    async fn cancel_execution_returns_ok_for_ready_state() {
        // When nothing is executing, cancel returns Ok(()) for Ready state
        let result = cancel_execution("step-ready-789".to_string()).await;
        assert!(result.is_ok());
    }
}
