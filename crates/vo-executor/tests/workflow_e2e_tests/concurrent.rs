use super::common::prelude::*;

#[tokio::test]
async fn concurrent_e2e_multiple_steps_executed_simultaneously() {
    let _guard = state_guard();

    let result1 = execute_step(StepId::new("step-1".to_string()), 5000).await;
    let result2 = execute_step(StepId::new("step-good".to_string()), 5000).await;
    let result3 = execute_step(StepId::new("step-fail".to_string()), 5000).await;

    assert!(result1.is_ok(), "Step 1 should succeed");
    assert!(result2.is_ok(), "Step good should succeed");
    assert!(
        result3.is_ok(),
        "Step fail should return Failure result (not error)"
    );
}

#[tokio::test]
async fn concurrent_e2e_mixed_success_and_failure_across_steps() {
    let _guard = state_guard();

    let result1 = execute_step(StepId::new("step-1".to_string()), 5000).await;
    let result2 = execute_step(StepId::new("step-fail".to_string()), 5000).await;
    let result3 = execute_step(StepId::new("step-transient".to_string()), 5000).await;
    let result4 = execute_step(StepId::new("step-good".to_string()), 5000).await;

    assert!(result1.is_ok(), "Step 1 should succeed");
    assert!(result4.is_ok(), "Step good should succeed");
    assert!(result2.is_ok(), "Step fail should return Ok(Failure)");
    assert!(result3.is_err(), "Step transient should error");
}

#[tokio::test]
async fn concurrent_e2e_retry_and_non_retry_executed_together() {
    let _guard = state_guard();
    let policy = RetryPolicy::new(3, 10, 2.0).unwrap();

    let (retry_result, direct_result) = tokio::join!(
        execute_step_with_retry(StepId::new("step-flaky".to_string()), 5000, policy.clone()),
        execute_step(StepId::new("step-1".to_string()), 5000)
    );

    assert!(
        matches!(
            retry_result,
            Err(ExecuteNodeError::RetryExhausted { .. })
        ),
        "Flaky step should exhaust retries"
    );
    assert!(direct_result.is_ok(), "Direct step should succeed");
}

#[tokio::test]
async fn concurrent_e2e_many_parallel_executions_all_complete() {
    let _guard = state_guard();

    let step_names = ["step-1", "step-good", "step-fail", "step-transient"];

    let mut handles = vec![];
    for _ in 0..10 {
        for name in step_names {
            let step_id = StepId::new(name.to_string());
            handles.push(tokio::spawn(
                async move { execute_step(step_id, 5000).await },
            ));
        }
    }

    let mut success_count = 0;
    let mut failure_count = 0;
    let mut error_count = 0;

    for handle in handles {
        let result = handle.await;
        match result {
            Ok(Ok(StepResult::Success { .. })) => success_count += 1,
            Ok(Ok(StepResult::Failure { .. })) => failure_count += 1,
            Ok(Err(_)) | Err(_) => error_count += 1,
        }
    }

    assert_eq!(
        success_count, 20,
        "10 iterations × 2 success steps (step-1, step-good) = 20"
    );
    assert_eq!(failure_count, 10, "10 iterations × 1 failure step = 10");
    assert_eq!(
        error_count, 10,
        "10 iterations × 1 transient error step = 10"
    );
}

#[tokio::test]
async fn concurrent_e2e_sequential_then_parallel_mixed_workflow() {
    let _guard = state_guard();

    let result1 = execute_step(StepId::new("step-1".to_string()), 5000).await;
    assert!(result1.is_ok());

    let result2 = execute_step(StepId::new("step-good".to_string()), 5000).await;
    let result3 = execute_step(StepId::new("step-fail".to_string()), 5000).await;

    assert!(result2.is_ok());
    assert!(result3.is_ok());

    let result4 = execute_step(StepId::new("step-1".to_string()), 5000).await;
    assert!(result4.is_ok());

    let status = get_execution_status(&StepId::new("step-1".to_string()));
    assert!(status.is_ready(), "Final status should be Ready");
}
