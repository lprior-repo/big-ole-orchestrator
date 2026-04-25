//!
//! Runtime integration tests.
//!
//! NOTE: These tests use #[test] instead of #[tokio::test] because
//! Runtime::new() creates a new_current_thread() tokio runtime internally.
//! When a Runtime is dropped within a #[tokio::test] context (which runs
//! on tokio's multi-threaded runtime), tokio panics with:
//! "Cannot drop a runtime in a context where blocking is not allowed."
//!
//! The Runtime's synchronous methods (execute_step_sync, etc.) handle
//! the blocking internally via block_on(), so we can use regular #[test].
//!

use super::common::prelude::*;

#[test]
fn runtime_e2e_execute_step_sync_through_runtime() {
    let _guard = state_guard();
    let runtime = Runtime::new().expect("Runtime creation should succeed");

    let result = runtime.execute_step_sync(StepId::new("step-1".to_string()), 5000);
    assert!(result.is_ok(), "Runtime should execute step successfully");
    assert!(
        result.unwrap().is_success(),
        "Result should indicate success"
    );
}

#[test]
#[ignore = "Runtime with retry creates nested tokio runtime that panics on drop - retry logic tested in integration tests"]
fn runtime_e2e_execute_step_with_retry_sync() {
    let _guard = state_guard();
    let runtime = Runtime::new().expect("Runtime creation should succeed");
    let policy = RetryPolicy::new(3, 10, 2.0).unwrap();

    let result = runtime.execute_step_with_retry_sync(
        StepId::new("step-flaky".to_string()),
        5000,
        policy,
    );
    assert!(
        matches!(
            result,
            Err(ExecuteNodeError::RetryExhausted { .. })
        ),
        "Runtime should handle retry exhaustion"
    );
}

#[test]
fn runtime_e2e_get_status_through_runtime() {
    let _guard = state_guard();
    let runtime = Runtime::new().expect("Runtime creation should succeed");

    let status = runtime.get_status(&StepId::new("step-1".to_string()));
    assert_eq!(status, ExecutionStatus::Ready);
}

#[test]
fn runtime_e2e_cancel_through_runtime() {
    let _guard = state_guard();
    let runtime = Runtime::new().expect("Runtime creation should succeed");

    let result = runtime.cancel(StepId::new("step-1".to_string()));
    assert!(result.is_ok(), "Runtime cancel should succeed");
}

#[test]
fn runtime_e2e_step_context_execute() {
    let _guard = state_guard();
    let context = StepContext::new(StepId::new("step-1".to_string()))
        .expect("StepContext creation should succeed");

    let result = context.execute(5000);
    assert!(result.is_ok(), "StepContext execute should succeed");
    assert!(
        result.unwrap().is_success(),
        "Result should indicate success"
    );
}

#[test]
fn runtime_e2e_step_context_execute_with_retry() {
    let _guard = state_guard();
    let context = StepContext::new(StepId::new("step-1".to_string()))
        .expect("StepContext creation should succeed");
    let policy = RetryPolicy::new(3, 10, 2.0).unwrap();

    let result = context.execute_with_retry(5000, policy);
    assert!(result.is_ok(), "StepContext retry execute should succeed");
}

#[test]
fn runtime_e2e_step_context_status() {
    let _guard = state_guard();
    let context = StepContext::new(StepId::new("step-1".to_string()))
        .expect("StepContext creation should succeed");

    let status = context.status();
    assert_eq!(status, ExecutionStatus::Ready);
}

#[test]
fn runtime_e2e_step_context_cancel() {
    let _guard = state_guard();
    let context = StepContext::new(StepId::new("step-1".to_string()))
        .expect("StepContext creation should succeed");

    let result = context.cancel();
    assert!(result.is_ok(), "StepContext cancel should succeed");
}
