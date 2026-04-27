use super::common::prelude::*;

#[tokio::test]
async fn state_transitions_ready_to_executing_to_ready() {
    let _guard = state_guard();
    let step_id = StepId::new("step-1".to_string());

    let initial = get_execution_status(&step_id);
    assert!(initial.is_ready(), "Should start in Ready state");

    let result = execute_step(step_id.clone(), 5000).await;
    assert!(result.is_ok());

    let final_status = get_execution_status(&step_id);
    assert!(
        final_status.is_ready(),
        "Should return to Ready after execution"
    );
}

#[tokio::test]
async fn state_transitions_ready_to_cancelled() {
    let _guard = state_guard();
    let step_id = StepId::new("step-1".to_string());

    let result = cancel_execution(step_id.clone()).await;
    assert!(result.is_ok(), "Cancel on Ready should succeed");

    let status = get_execution_status(&step_id);
    match status {
        ExecutionStatus::Cancelled { reason } => {
            assert!(reason.contains("cancelled"));
        }
        other => panic!("Expected Cancelled, got {:?}", other),
    }
}

#[tokio::test]
async fn state_transitions_cancelled_to_ready_on_next_execution() {
    let _guard = state_guard();
    let step_id = StepId::new("step-1".to_string());

    cancel_execution(step_id.clone())
        .await
        .expect("Cancel should succeed");
    let cancelled_status = get_execution_status(&step_id);
    assert!(
        matches!(
            cancelled_status,
            ExecutionStatus::Cancelled { .. }
        ),
        "Should be Cancelled"
    );

    let result = execute_step(step_id.clone(), 5000).await;
    assert!(result.is_ok());

    let ready_status = get_execution_status(&step_id);
    assert!(
        ready_status.is_ready(),
        "Should return to Ready after execution"
    );
}

#[tokio::test]
async fn state_transitions_double_cancel_is_noop() {
    let _guard = state_guard();
    let step_id = StepId::new("step-1".to_string());

    cancel_execution(step_id.clone())
        .await
        .expect("First cancel should succeed");
    let result2 = cancel_execution(step_id.clone()).await;
    assert!(result2.is_ok(), "Second cancel should be no-op and succeed");
}
