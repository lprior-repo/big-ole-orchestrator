use super::common::prelude::*;

#[tokio::test]
async fn e2e_timeout_slow_step_with_sufficient_timeout_succeeds() {
    let _guard = state_guard();
    let step_id = StepId::new("step-slow".to_string());

    let result = execute_step(step_id.clone(), 5000).await;
    assert!(
        result.is_ok() && matches!(result.unwrap(), StepResult::Success { .. }),
        "Slow step with 5000ms timeout should succeed"
    );

    let status = get_execution_status(&step_id);
    assert!(status.is_ready(), "Status should be Ready after success");
}

#[tokio::test]
async fn e2e_timeout_slow_step_with_insufficient_timeout_fails() {
    let _guard = state_guard();
    let step_id = StepId::new("step-slow".to_string());

    let result = execute_step(step_id.clone(), 1).await;
    assert!(
        matches!(
            result,
            Err(ExecuteNodeError::TimeoutExceeded { .. })
        ),
        "Slow step with 1ms timeout should return TimeoutExceeded"
    );

    let status = get_execution_status(&step_id);
    assert!(status.is_ready(), "Status should be Ready after timeout");
}

#[tokio::test]
async fn e2e_timeout_boundary_condition_exactly_3000ms() {
    let _guard = state_guard();
    let step_id = StepId::new("step-slow".to_string());

    let result = execute_step(step_id.clone(), 3000).await;
    assert!(
        result.is_ok() && matches!(result.unwrap(), StepResult::Success { .. }),
        "Slow step with exactly 3000ms timeout should succeed (boundary)"
    );
}

#[tokio::test]
async fn e2e_timeout_boundary_condition_2999ms() {
    let _guard = state_guard();
    let step_id = StepId::new("step-slow".to_string());

    let result = execute_step(step_id.clone(), 2999).await;
    assert!(
        matches!(
            result,
            Err(ExecuteNodeError::TimeoutExceeded { .. })
        ),
        "Slow step with 2999ms timeout should timeout (just under boundary)"
    );
}

#[tokio::test]
async fn e2e_timeout_with_retry_respects_timeout_on_each_attempt() {
    let _guard = state_guard();
    let policy = RetryPolicy::new(3, 100, 2.0).unwrap();
    let step_id = StepId::new("step-slow".to_string());

    let result = execute_step_with_retry(step_id.clone(), 1, policy).await;
    assert!(
        matches!(
            result,
            Err(ExecuteNodeError::TimeoutExceeded { .. })
        ),
        "Retry with insufficient timeout should return TimeoutExceeded"
    );
}

#[tokio::test]
async fn e2e_timeout_invalid_zero_immediately_rejected() {
    let _guard = state_guard();
    let step_id = StepId::new("step-1".to_string());

    let result = execute_step(step_id.clone(), 0).await;
    assert!(
        matches!(
            result,
            Err(ExecuteNodeError::InvalidTimeout { value: 0, .. })
        ),
        "Zero timeout should be immediately rejected"
    );
}

#[tokio::test]
async fn e2e_timeout_invalid_max_u64_immediately_rejected() {
    let _guard = state_guard();
    let step_id = StepId::new("step-1".to_string());

    let result = execute_step(step_id.clone(), u64::MAX).await;
    assert!(
        matches!(
            result,
            Err(ExecuteNodeError::InvalidTimeout {
                value: u64::MAX,
                ..
            })
        ),
        "u64::MAX timeout should be immediately rejected"
    );
}
