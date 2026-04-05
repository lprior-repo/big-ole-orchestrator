// Integration tests for vel-k1t9
// These tests verify the actual async function implementations

#[cfg(test)]
mod integration_tests {
    use vo_executor::{
        cancel_execution, execute_step, execute_step_with_retry, get_execution_status,
        get_last_error, RetryPolicy, StepId,
    };

    #[tokio::test]
    async fn execute_step_rejects_zero_timeout() {
        let result = execute_step(StepId::new("step-1".to_string()), 0).await;
        assert_eq!(
            result,
            Err(vo_executor::ExecuteNodeError::InvalidTimeout {
                value: 0,
                reason: "must be > 0ms".to_string(),
            })
        );
    }

    #[tokio::test]
    async fn execute_step_success_for_step_1() {
        let result = execute_step(StepId::new("step-1".to_string()), 5000).await;
        assert_eq!(
            result,
            Ok(vo_executor::StepResult::Success {
                output: "done".to_string(),
            })
        );
    }

    #[tokio::test]
    async fn execute_step_timeout_for_slow_step() {
        let result = execute_step(StepId::new("step-slow".to_string()), 1).await;
        assert_eq!(
            result,
            Err(vo_executor::ExecuteNodeError::TimeoutExceeded {
                elapsed_ms: 3000,
                limit_ms: 1,
            })
        );
    }

    #[tokio::test]
    async fn execute_step_not_found_for_unknown_step() {
        let result = execute_step(StepId::new("unknown-step".to_string()), 5000).await;
        assert_eq!(
            result,
            Err(vo_executor::ExecuteNodeError::StepNotFound {
                step_id: StepId::new("unknown-step".to_string()),
            })
        );
    }

    #[tokio::test]
    async fn execute_step_with_retry_success() {
        let policy = RetryPolicy::new(3, 100, 2.0).unwrap();
        let result = execute_step_with_retry(StepId::new("step-1".to_string()), 5000, policy).await;
        assert_eq!(
            result,
            Ok(vo_executor::StepResult::Success {
                output: "done".to_string(),
            })
        );
    }

    #[tokio::test]
    async fn get_execution_status_returns_ready() {
        let status = get_execution_status(&StepId::new("step-1".to_string()));
        assert!(status.is_ready());
    }

    #[tokio::test]
    async fn get_last_error_returns_none() {
        let error = get_last_error(&StepId::new("step-1".to_string()));
        assert!(error.is_none());
    }

    #[tokio::test]
    async fn cancel_execution_returns_ok_for_ready_state() {
        // When nothing is executing, cancel returns Ok (no-op)
        let result = cancel_execution(StepId::new("step-1".to_string())).await;
        assert_eq!(result, Ok(()));
    }
}
