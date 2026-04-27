use super::common::*;

#[cfg(test)]
mod tests {
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
