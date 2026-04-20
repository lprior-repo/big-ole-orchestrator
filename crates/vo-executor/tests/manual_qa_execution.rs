use std::time::Instant;

use vo_executor::{
    cancel_execution, execute_step, execute_step_with_retry, get_execution_status,
    get_last_error, reset_all_state, ExecutionStatus, ExecuteNodeError, RetryPolicy, StepId,
};

// --- Happy Paths ---

#[tokio::test]
async fn qam_task_completes_successfully_step1() {
    reset_all_state();
    let result = execute_step(StepId::new("step-1".into()), 5000).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_success());
}

#[tokio::test]
async fn qam_task_failure_returns_stepresult_failure() {
    reset_all_state();
    let result = execute_step(StepId::new("step-fail".into()), 5000).await;
    assert!(result.is_ok());
    assert!(!result.unwrap().is_success());
}

#[tokio::test]
async fn qam_slow_step_succeeds_with_large_timeout() {
    reset_all_state();
    let result = execute_step(StepId::new("step-slow".into()), 5000).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_success());
}

#[tokio::test]
async fn qam_workflow_step_executes() {
    reset_all_state();
    let result = execute_step(StepId::new("workflow-step-1".into()), 5000).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_success());
}

#[tokio::test]
async fn qam_step_good_executes_successfully() {
    reset_all_state();
    let result = execute_step(StepId::new("step-good".into()), 5000).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_success());
}

#[tokio::test]
async fn qam_step_valid_executes_successfully() {
    reset_all_state();
    let result = execute_step(StepId::new("step-valid".into()), 5000).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_success());
}

// --- State Transitions / Completion ---

#[tokio::test]
async fn qam_state_returns_to_ready_after_success() {
    reset_all_state();
    let _ = execute_step(StepId::new("step-1".into()), 5000).await;
    assert_eq!(
        get_execution_status(&StepId::new("step-1".into())),
        ExecutionStatus::Ready
    );
}

#[tokio::test]
async fn qam_state_returns_to_ready_after_failure() {
    reset_all_state();
    let _ = execute_step(StepId::new("step-fail".into()), 5000).await;
    assert_eq!(
        get_execution_status(&StepId::new("step-fail".into())),
        ExecutionStatus::Ready
    );
}

#[tokio::test]
async fn qam_get_execution_status_reflects_executing() {
    reset_all_state();
    vo_executor::state::set_state(
        "qa-step",
        vo_executor::state::StepState::Executing {
            step_id: StepId::new("qa-step".into()),
            start_time: Instant::now(),
        },
    );
    let status = get_execution_status(&StepId::new("qa-step".into()));
    assert!(matches!(status, ExecutionStatus::Executing { .. }));
}

#[tokio::test]
async fn qam_get_execution_status_reflects_completed() {
    reset_all_state();
    vo_executor::state::set_state(
        "qa-step",
        vo_executor::state::StepState::Completed {
            output: "result-data".into(),
        },
    );
    let status = get_execution_status(&StepId::new("qa-step".into()));
    assert_eq!(
        status,
        ExecutionStatus::Completed {
            output: "result-data".into()
        }
    );
}

// --- Cancellation ---

#[tokio::test]
async fn qam_cancel_from_ready_transitions_to_cancelled() {
    reset_all_state();
    vo_executor::state::set_state("qa-step", vo_executor::state::StepState::Ready);
    let result = cancel_execution(StepId::new("qa-step".into())).await;
    assert!(result.is_ok());
    assert!(matches!(
        get_execution_status(&StepId::new("qa-step".into())),
        ExecutionStatus::Cancelled { .. }
    ));
}

#[tokio::test]
async fn qam_cancel_from_executing_returns_error() {
    reset_all_state();
    vo_executor::state::set_state(
        "qa-step",
        vo_executor::state::StepState::Executing {
            step_id: StepId::new("qa-step".into()),
            start_time: Instant::now(),
        },
    );
    let result = cancel_execution(StepId::new("qa-step".into())).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn qam_cancel_from_cancelled_is_noop() {
    reset_all_state();
    vo_executor::state::set_state(
        "qa-step",
        vo_executor::state::StepState::Cancelled {
            reason: "first".into(),
        },
    );
    let result = cancel_execution(StepId::new("qa-step".into())).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn qam_cancel_from_completed_is_noop() {
    reset_all_state();
    vo_executor::state::set_state(
        "qa-step",
        vo_executor::state::StepState::Completed {
            output: "done".into(),
        },
    );
    let result = cancel_execution(StepId::new("qa-step".into())).await;
    assert!(result.is_ok());
}

// --- Zombie Task Detection ---

#[tokio::test]
async fn qam_no_zombie_tasks_after_success() {
    reset_all_state();
    let start_count = vo_executor::state::get_state_count();
    let _ = execute_step(StepId::new("step-1".into()), 5000).await;
    let end_count = vo_executor::state::get_state_count();
    assert_eq!(end_count, start_count + 1);
}

#[tokio::test]
async fn qam_no_zombie_tasks_after_failure() {
    reset_all_state();
    let _ = execute_step(StepId::new("step-fail".into()), 5000).await;
    assert_eq!(
        get_execution_status(&StepId::new("step-fail".into())),
        ExecutionStatus::Ready
    );
}

#[tokio::test]
async fn qam_no_zombie_tasks_after_transient_error() {
    reset_all_state();
    let _ = execute_step(StepId::new("step-transient".into()), 5000).await;
    assert_eq!(
        get_execution_status(&StepId::new("step-transient".into())),
        ExecutionStatus::Ready
    );
}

#[tokio::test]
async fn qam_no_stale_errors_after_successful_execution() {
    reset_all_state();
    let _ = execute_step(StepId::new("step-1".into()), 5000).await;
    assert!(get_last_error(&StepId::new("step-1".into())).is_none());
}

#[tokio::test]
async fn qam_error_stored_after_transient_failure() {
    reset_all_state();
    let _ = execute_step(StepId::new("step-transient".into()), 5000).await;
    assert!(get_last_error(&StepId::new("step-transient".into())).is_some());
}

// --- Error Paths ---

#[tokio::test]
async fn qam_nonexistent_step_returns_not_found() {
    reset_all_state();
    let result = execute_step(StepId::new("nonexistent".into()), 5000).await;
    assert!(matches!(result, Err(ExecuteNodeError::StepNotFound { .. })));
}

#[tokio::test]
async fn qam_timeout_zero_rejects() {
    reset_all_state();
    let result = execute_step(StepId::new("step-1".into()), 0).await;
    assert!(matches!(
        result,
        Err(ExecuteNodeError::InvalidTimeout { value: 0, .. })
    ));
}

#[tokio::test]
async fn qam_timeout_max_rejects() {
    reset_all_state();
    let result = execute_step(StepId::new("step-1".into()), u64::MAX).await;
    assert!(matches!(
        result,
        Err(ExecuteNodeError::InvalidTimeout { value, .. }) if value == u64::MAX
    ));
}

#[tokio::test]
async fn qam_slow_step_small_timeout_exceeds() {
    reset_all_state();
    let result = execute_step(StepId::new("step-slow".into()), 100).await;
    assert!(matches!(result, Err(ExecuteNodeError::TimeoutExceeded { .. })));
}

#[tokio::test]
async fn qam_double_execute_guard() {
    reset_all_state();
    vo_executor::state::set_state(
        "step-1",
        vo_executor::state::StepState::Executing {
            step_id: StepId::new("step-1".into()),
            start_time: Instant::now(),
        },
    );
    let result = execute_step(StepId::new("step-1".into()), 5000).await;
    assert!(matches!(result, Err(ExecuteNodeError::InvalidTransition { .. })));
}

// --- Retry ---

#[tokio::test]
async fn qam_retry_success_step_succeeds() {
    reset_all_state();
    let policy = RetryPolicy::new(3, 100, 2.0).unwrap();
    let result =
        execute_step_with_retry(StepId::new("step-1".into()), 5000, policy).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_success());
}

#[tokio::test]
async fn qam_retry_flaky_exhausts_all() {
    reset_all_state();
    let policy = RetryPolicy::new(3, 10, 2.0).unwrap();
    let result =
        execute_step_with_retry(StepId::new("step-flaky".into()), 5000, policy).await;
    assert!(matches!(
        result,
        Err(ExecuteNodeError::RetryExhausted { attempts: 3, .. })
    ));
}

#[tokio::test]
async fn qam_retry_flaky_single_attempt() {
    reset_all_state();
    let policy = RetryPolicy::new(1, 10, 2.0).unwrap();
    let result =
        execute_step_with_retry(StepId::new("step-flaky".into()), 5000, policy).await;
    assert!(matches!(
        result,
        Err(ExecuteNodeError::RetryExhausted { attempts: 1, .. })
    ));
}

#[tokio::test]
async fn qam_retry_zero_attempts_rejected() {
    let policy = RetryPolicy::new(0, 100, 2.0);
    assert!(policy.is_err());
}

// --- Edge Cases ---

#[tokio::test]
async fn qam_empty_step_returns_not_found() {
    reset_all_state();
    let result = execute_step(StepId::new(String::new()), 5000).await;
    assert!(matches!(result, Err(ExecuteNodeError::StepNotFound { .. })));
}

#[tokio::test]
async fn qam_numeric_step_names_work() {
    reset_all_state();
    let result = execute_step(StepId::new("step-999".into()), 5000).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_success());
}

#[tokio::test]
async fn qam_transient_error_stored_recoverable() {
    reset_all_state();
    let _ = execute_step(StepId::new("step-transient".into()), 5000).await;
    let err_val = get_last_error(&StepId::new("step-transient".into()));
    assert!(matches!(
        err_val,
        Some(ExecuteNodeError::TransientError {
            recoverable: true,
            ..
        })
    ));
}

#[tokio::test]
async fn qam_reset_clears_zombies() {
    reset_all_state();
    let _ = execute_step(StepId::new("step-1".into()), 5000).await;
    let _ = execute_step(StepId::new("step-transient".into()), 5000).await;
    reset_all_state();
    assert_eq!(vo_executor::state::get_state_count(), 0);
    assert_eq!(vo_executor::state::get_error_count(), 0);
}

#[tokio::test]
async fn qam_last_error_none_for_unknown() {
    reset_all_state();
    assert!(get_last_error(&StepId::new("unknown-qa-step".into())).is_none());
}
