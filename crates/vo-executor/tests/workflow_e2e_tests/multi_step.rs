use super::common::prelude::*;

#[tokio::test]
async fn multi_step_workflow_sequential_success_steps() {
    let _guard = state_guard();
    let steps = ["workflow-step-1", "step-1", "step-good"];

    for step_name in steps {
        let step_id = StepId::new(step_name.to_string());
        let result = execute_step(step_id.clone(), 5000).await;
        assert!(
            result.is_ok() && matches!(result.unwrap(), StepResult::Success { .. }),
            "Step {} should succeed in sequential workflow",
            step_name
        );
        assert!(
            get_execution_status(&step_id).is_ready(),
            "Step {} status should be Ready after execution",
            step_name
        );
    }
}

#[tokio::test]
async fn multi_step_workflow_failure_stops_workflow() {
    let _guard = state_guard();
    let step_id = StepId::new("step-fail".to_string());

    let result = execute_step(step_id.clone(), 5000).await;
    assert!(
        matches!(result, Ok(StepResult::Failure { .. })),
        "Failure step should return Failure result"
    );

    let status = get_execution_status(&step_id);
    assert!(status.is_ready(), "Status should be Ready after failure");
}

#[tokio::test]
async fn multi_step_workflow_transient_error_stops_workflow_with_error_persisted() {
    let _guard = state_guard();
    let step_id = StepId::new("step-transient".to_string());

    let result = execute_step(step_id.clone(), 5000).await;
    assert!(result.is_err(), "Transient step should return error");

    let error = get_last_error(&step_id);
    assert!(error.is_some(), "Error should be persisted");
}

#[tokio::test]
async fn multi_step_workflow_mixed_results_accumulate_states() {
    let _guard = state_guard();
    let steps = vec![
        ("step-1", true),
        ("step-fail", false),
        ("step-transient", false),
        ("step-good", true),
    ];

    for (step_name, expect_success) in steps {
        let step_id = StepId::new(step_name.to_string());
        let result = execute_step(step_id.clone(), 5000).await;

        if expect_success {
            assert!(result.is_ok(), "Step {} should succeed", step_name);
        } else {
            assert!(
                result.is_err() || matches!(result, Ok(StepResult::Failure { .. })),
                "Step {} should fail or error",
                step_name
            );
        }
    }
}

#[tokio::test]
async fn multi_step_workflow_with_retry_handles_flaky_steps() {
    let _guard = state_guard();
    let policy = RetryPolicy::new(3, 10, 2.0).unwrap();
    let step_id = StepId::new("step-flaky".to_string());

    let result = execute_step_with_retry(step_id.clone(), 5000, policy).await;
    assert!(
        matches!(
            result,
            Err(ExecuteNodeError::RetryExhausted { .. })
        ),
        "Flaky step with retry should return RetryExhausted"
    );
}

#[tokio::test]
async fn multi_step_workflow_retry_with_successful_step() {
    let _guard = state_guard();
    let policy = RetryPolicy::new(3, 10, 2.0).unwrap();
    let step_id = StepId::new("step-1".to_string());

    let result = execute_step_with_retry(step_id.clone(), 5000, policy).await;
    assert!(
        result.is_ok() && matches!(result.unwrap(), StepResult::Success { .. }),
        "Successful step with retry should still succeed"
    );
}
