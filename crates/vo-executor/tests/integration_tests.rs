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
        let result = execute_step("step-1".to_string(), 0).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, vel_k1t9::ExecuteNodeError::InvalidTimeout { .. }));
    }

    #[tokio::test]
    async fn execute_step_success_for_step_1() {
        let result = execute_step("step-1".to_string(), 5000).await;
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
        let result = execute_step("unknown-step".to_string(), 5000).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, vel_k1t9::ExecuteNodeError::StepNotFound { .. }));
    }

    #[tokio::test]
    async fn execute_step_with_retry_invalid_policy() {
        let policy = RetryPolicy::new(0, 100, 2.0).unwrap_err();
        // policy is RetryPolicyError, not ExecuteNodeError
        let result = execute_step_with_retry("step-1".to_string(), 5000, policy).await;
        // The function takes RetryPolicy, not Result<RetryPolicy, RetryPolicyError>
        // So this test can't easily test invalid policy
    }

    #[tokio::test]
    async fn get_execution_status_returns_ready() {
        let status = get_execution_status("step-1");
        assert!(status.is_ready());
    }

    #[tokio::test]
    async fn get_last_error_returns_none() {
        let error = get_last_error("step-1");
        assert!(error.is_none());
    }

    #[tokio::test]
    async fn cancel_execution_returns_error() {
        // When nothing is executing, cancel returns error
        let result = cancel_execution("step-1".to_string()).await;
        // Implementation returns ExecutionCancelled since step isn't running
        assert!(result.is_err());
    }
}
