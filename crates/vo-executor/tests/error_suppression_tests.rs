//! BLACKHAT bh-012: Error suppression attack tests
//!
//! Adversarial tests verifying that errors CANNOT be suppressed or swallowed.
//! Every error path must propagate to the caller. No silent success when failure occurred.

use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::MutexGuard;

use vo_executor::{cancel_execution, execute_step, execute_step_with_retry, get_last_error, reset_all_state, set_error, ExecuteNodeError, RetryPolicy, RetryPolicyError, StepId, StepResult};

static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn setup() -> MutexGuard<'static, ()> {
    let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset_all_state();
    guard
}

/// ATTACK 1: StepResult::Failure must never be confused with success.
/// If a step fails (non-zero exit), the caller MUST be able to distinguish
/// Failure from Success — the error must not be suppressed into Ok(Success).
#[tokio::test]
async fn attack_failure_result_is_not_suppressed_into_success() {
    let _guard = setup();
    let result = execute_step(StepId::new("step-fail".to_string()), 5000).await;
    assert!(
        result.is_ok(),
        "step-fail should return Ok(StepResult::Failure), not Err"
    );
    let step_result = result.unwrap();
    assert!(
        !step_result.is_success(),
        "Failure variant must NOT report is_success() — error was suppressed"
    );
    assert!(
        matches!(step_result, StepResult::Failure { .. }),
        "Expected StepResult::Failure, got {:?}",
        step_result
    );
}

/// ATTACK 2: Nonexistent step MUST return Err — the error cannot be suppressed.
#[tokio::test]
async fn attack_step_not_found_returns_err_not_ok() {
    let _guard = setup();
    let result = execute_step(StepId::new("does-not-exist".to_string()), 5000).await;
    assert!(result.is_err(), "Nonexistent step MUST return Err, got Ok");
    assert!(
        matches!(result.unwrap_err(), ExecuteNodeError::StepNotFound { .. }),
        "Expected StepNotFound error"
    );
}

/// ATTACK 3: Transient errors must propagate as Err AND be stored in get_last_error.
#[tokio::test]
async fn attack_transient_error_propagates_and_stored() {
    let _guard = setup();
    let step = StepId::new("step-transient".to_string());
    let result = execute_step(step.clone(), 5000).await;
    assert!(result.is_err(), "Transient step MUST return Err");
    assert!(
        matches!(result.unwrap_err(), ExecuteNodeError::TransientError { .. }),
        "Expected TransientError"
    );
    let stored = get_last_error(&step);
    assert!(stored.is_some(), "Transient error MUST be stored — suppressed if None");
}

/// ATTACK 4: Zero timeout must return Err, not be silently accepted.
#[tokio::test]
async fn attack_zero_timeout_returns_err() {
    let _guard = setup();
    let result = execute_step(StepId::new("step-1".to_string()), 0).await;
    assert!(result.is_err(), "timeout=0 MUST return Err");
    assert!(matches!(result.unwrap_err(), ExecuteNodeError::InvalidTimeout { value: 0, .. }));
}

/// ATTACK 5: u64::MAX timeout must return Err — effectively infinite timeout masks failures.
#[tokio::test]
async fn attack_max_timeout_returns_err() {
    let _guard = setup();
    let result = execute_step(StepId::new("step-1".to_string()), u64::MAX).await;
    assert!(result.is_err(), "timeout=u64::MAX MUST return Err");
    assert!(matches!(result.unwrap_err(), ExecuteNodeError::InvalidTimeout { value: u64::MAX, .. }));
}

/// ATTACK 6: Slow step with small timeout must return TimeoutExceeded, not silently succeed.
#[tokio::test]
async fn attack_slow_step_timeout_not_suppressed() {
    let _guard = setup();
    let result = execute_step(StepId::new("step-slow".to_string()), 100).await;
    assert!(result.is_err(), "Slow step with tiny timeout MUST return Err");
    assert!(matches!(result.unwrap_err(), ExecuteNodeError::TimeoutExceeded { .. }));
}

/// ATTACK 7: RetryExhausted must preserve the original inner error, not swallow it.
#[tokio::test]
async fn attack_retry_exhausted_preserves_last_error() {
    let _guard = setup();
    let policy = RetryPolicy::new(3, 1, 1.0).unwrap();
    let result = execute_step_with_retry(StepId::new("step-flaky".to_string()), 5000, policy).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ExecuteNodeError::RetryExhausted { attempts, last_error } => {
            assert_eq!(attempts, 3, "Must report correct attempt count");
            assert!(
                matches!(*last_error, ExecuteNodeError::TransientError { .. }),
                "Inner error must be the original TransientError"
            );
        }
        other => panic!("Expected RetryExhausted, got {:?}", other),
    }
}

/// ATTACK 8: Re-executing a step in Executing state must return InvalidTransition.
#[tokio::test]
async fn attack_double_execution_returns_invalid_transition() {
    let _guard = setup();
    vo_executor::set_executing_state_for_test("step-1");
    let result = execute_step(StepId::new("step-1".to_string()), 5000).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ExecuteNodeError::InvalidTransition { .. }));
}

/// ATTACK 9: Cancel during Executing must return ExecutionCancelled, not silently succeed.
#[tokio::test]
async fn attack_cancel_during_executing_returns_err() {
    let _guard = setup();
    vo_executor::set_executing_state_for_test("cancel-target");
    let result = cancel_execution(StepId::new("cancel-target".to_string())).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ExecuteNodeError::ExecutionCancelled { .. }));
}

/// ATTACK 10: Zero retry attempts must be rejected — cannot bypass retry validation.
#[test]
fn attack_zero_retry_attempts_rejected() {
    let result = RetryPolicy::new(0, 100, 2.0);
    assert!(result.is_err());
}

/// ATTACK 11: NaN multiplier must be rejected — could bypass retry delay calculations.
#[test]
fn attack_nan_multiplier_rejected() {
    let result = RetryPolicy::new(3, 100, f64::NAN);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), RetryPolicyError::InvalidMultiplier { .. }));
}

/// ATTACK 12: Concurrent error-producing operations must all propagate — none swallowed.
#[tokio::test]
async fn attack_concurrent_errors_all_propagate() {
    let _guard = setup();
    let mut handles = Vec::new();
    for i in 0..10 {
        handles.push(tokio::spawn(async move {
            let step = StepId::new(format!("nonexistent-{}", i));
            execute_step(step, 5000).await
        }));
    }
    for (i, handle) in handles.into_iter().enumerate() {
        let result = handle.await.unwrap();
        assert!(result.is_err(), "Concurrent error {} was suppressed", i);
    }
}

/// ATTACK 13: Cancel from Ready must not clear a previously stored error.
#[tokio::test]
async fn attack_cancel_does_not_suppress_stored_error() {
    let _guard = setup();
    set_error("cancel-idem", ExecuteNodeError::TransientError {
        reason: "test-error".to_string(),
        recoverable: false,
    });
    let _ = cancel_execution(StepId::new("cancel-idem".to_string())).await;
    let stored = get_last_error(&StepId::new("cancel-idem".to_string()));
    assert!(stored.is_some(), "Cancel must not suppress stored error");
}

/// ATTACK 14: StepId::parse must reject special characters with error, not panic.
#[test]
fn attack_step_id_parse_rejects_special_chars() {
    for input in ["has space", "dot.value", "a@b", "a!b", "step/slash"] {
        assert!(StepId::parse(input).is_err(), "StepId::parse({:?}) must reject", input);
    }
}

/// ATTACK 15: Runtime::execute_step_sync must propagate StepNotFound.
#[test]
fn attack_runtime_propagates_step_not_found() {
    let _guard = setup();
    let runtime = vo_executor::Runtime::new().unwrap();
    let result = runtime.execute_step_sync(StepId::new("nonexistent-rt".to_string()), 5000);
    assert!(result.is_err(), "Runtime MUST propagate StepNotFound");
}

/// ATTACK 16: SubprocessError display must include diagnostic info — no empty messages that hide failures.
#[test]
fn attack_subprocess_error_display_carries_info() {
    use vo_executor::SubprocessError;
    assert!(SubprocessError::Timeout { elapsed_ms: 5000 }.to_string().contains("5000"));
    assert!(SubprocessError::SpawnFailed("permission denied".into()).to_string().contains("permission denied"));
    assert!(SubprocessError::ProcessFailed { exit_code: 1 }.to_string().contains("1"));
}
