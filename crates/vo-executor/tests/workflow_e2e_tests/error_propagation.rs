use super::common::prelude::*;

#[tokio::test]
async fn error_propagation_transient_error_is_recoverable() {
    let _guard = state_guard();
    let step_id = StepId::new("step-transient".to_string());

    let result = execute_step(step_id.clone(), 5000).await;
    assert!(result.is_err(), "Transient step should error");

    let err = result.unwrap_err();
    match err {
        ExecuteNodeError::TransientError {
            reason,
            recoverable,
        } => {
            assert!(reason.contains("network timeout"));
            assert!(recoverable, "Transient error should be recoverable");
        }
        other => panic!("Expected TransientError, got {:?}", other),
    }

    let stored_error = get_last_error(&step_id);
    assert!(
        matches!(
            stored_error,
            Some(ExecuteNodeError::TransientError {
                recoverable: true,
                ..
            })
        ),
        "Stored error should indicate recoverable"
    );
}

#[tokio::test]
async fn error_propagation_retry_exhausted_contains_all_attempts() {
    let _guard = state_guard();
    let policy = RetryPolicy::new(3, 10, 2.0).unwrap();
    let step_id = StepId::new("step-flaky".to_string());

    let result = execute_step_with_retry(step_id.clone(), 5000, policy).await;
    assert!(
        matches!(
            result,
            Err(ExecuteNodeError::RetryExhausted { .. })
        ),
        "Flaky step should exhaust retries"
    );

    match result.unwrap_err() {
        ExecuteNodeError::RetryExhausted {
            attempts,
            last_error,
        } => {
            assert_eq!(attempts, 3, "Should report 3 attempts");
            assert!(
                matches!(
                    *last_error,
                    ExecuteNodeError::TransientError { .. }
                ),
                "Last error should be TransientError"
            );
        }
        other => panic!("Expected RetryExhausted, got {:?}", other),
    }
}

#[tokio::test]
async fn error_propagation_step_not_found_is_terminal() {
    let _guard = state_guard();
    let policy = RetryPolicy::new(3, 10, 2.0).unwrap();
    let step_id = StepId::new("nonexistent-step".to_string());

    let result = execute_step_with_retry(step_id.clone(), 5000, policy).await;
    assert!(
        matches!(
            result,
            Err(ExecuteNodeError::StepNotFound { .. })
        ),
        "StepNotFound should be terminal (no retry)"
    );

    let stored_error = get_last_error(&step_id);
    assert!(
        stored_error.is_none(),
        "StepNotFound should not persist error (handled before state set)"
    );
}

#[tokio::test]
async fn error_propagation_invalid_timeout_is_terminal() {
    let _guard = state_guard();
    let policy = RetryPolicy::new(3, 10, 2.0).unwrap();
    let step_id = StepId::new("step-1".to_string());

    let result = execute_step_with_retry(step_id.clone(), 0, policy).await;
    assert!(
        matches!(
            result,
            Err(ExecuteNodeError::InvalidTimeout { .. })
        ),
        "InvalidTimeout should be terminal (no retry)"
    );
}

#[tokio::test]
async fn error_propagation_failure_result_is_distinct_from_error() {
    let _guard = state_guard();
    let step_id = StepId::new("step-fail".to_string());

    let result = execute_step(step_id.clone(), 5000).await;
    assert!(
        matches!(result, Ok(StepResult::Failure { .. })),
        "Failure step should return Failure result, not error"
    );

    let stored_error = get_last_error(&step_id);
    assert!(
        stored_error.is_none(),
        "Failure result should NOT set LAST_ERROR (only transient errors do)"
    );

    let status = get_execution_status(&step_id);
    assert!(
        status.is_ready(),
        "Status should be Ready after Failure result"
    );
}
