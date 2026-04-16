//! QA tests for step execution lifecycle, timeouts, and retries.
//!
//! Validates the execution surface of `vo-executor` against production contracts.

use std::sync::LazyLock;
use std::sync::Mutex;

use tempfile::TempDir;
use vo_executor::{
    cancel_execution, execute_step, execute_step_with_retry, get_execution_status, get_last_error,
    reset_all_state, set_executing_state_for_test, ExecuteNodeError, ExecutionStatus, RetryPolicy,
    RetryPolicyError, Runtime, StepContext, StepId, StepResult,
};

static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn lock_and_reset() -> std::sync::MutexGuard<'static, ()> {
    let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset_all_state();
    guard
}

// ── Lifecycle ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn qa_lifecycle_success_returns_to_ready() {
    let _guard = lock_and_reset();
    let step = StepId::new("step-1".to_string());
    let result = execute_step(step.clone(), 5000).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_success());
    assert_eq!(get_execution_status(&step), ExecutionStatus::Ready);
}

#[tokio::test]
async fn qa_lifecycle_failure_and_cancel() {
    let _guard = lock_and_reset();

    let step = StepId::new("step-fail".to_string());
    let result = execute_step(step.clone(), 5000).await;
    assert!(result.is_ok());
    assert!(!result.unwrap().is_success());
    assert_eq!(get_execution_status(&step), ExecutionStatus::Ready);

    let c_step = StepId::new("step-1".to_string());
    assert!(cancel_execution(c_step.clone()).await.is_ok());
    assert!(matches!(get_execution_status(&c_step), ExecutionStatus::Cancelled { .. }));
    assert!(cancel_execution(c_step).await.is_ok());
}

#[tokio::test]
async fn qa_lifecycle_transient_stores_error() {
    let _guard = lock_and_reset();
    let step = StepId::new("step-transient".to_string());
    let res = execute_step(step.clone(), 5000).await;
    assert!(matches!(res, Err(ExecuteNodeError::TransientError { .. })));
    assert_eq!(get_execution_status(&step), ExecutionStatus::Ready);
    assert!(get_last_error(&step).is_some());
}

#[tokio::test]
async fn qa_lifecycle_cancel_during_executing_errors() {
    let _guard = lock_and_reset();
    let step = StepId::new("step-1".to_string());
    set_executing_state_for_test(step.as_str());
    let res = cancel_execution(step).await;
    assert!(matches!(res, Err(ExecuteNodeError::ExecutionCancelled { .. })));
}

#[tokio::test]
async fn qa_lifecycle_unknown_step_not_found() {
    let _guard = lock_and_reset();
    let res = execute_step(StepId::new("no-such-step-ever".to_string()), 5000).await;
    assert!(matches!(res, Err(ExecuteNodeError::StepNotFound { .. })));
}

// ── Timeouts ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn qa_timeout_invalid_values_rejected() {
    let _guard = lock_and_reset();

    let r0 = execute_step(StepId::new("step-1".to_string()), 0).await;
    assert!(matches!(r0, Err(ExecuteNodeError::InvalidTimeout { value: 0, .. })));

    let rm = execute_step(StepId::new("step-1".to_string()), u64::MAX).await;
    assert!(matches!(rm, Err(ExecuteNodeError::InvalidTimeout { value: v, .. }) if v == u64::MAX));
}

#[tokio::test]
async fn qa_timeout_slow_step_boundary_conditions() {
    let _guard = lock_and_reset();

    let res = execute_step(StepId::new("step-slow".to_string()), 100).await;
    assert!(matches!(res, Err(ExecuteNodeError::TimeoutExceeded { .. })));

    let res = execute_step(StepId::new("step-slow".to_string()), 3000).await;
    assert!(res.is_ok());

    let res = execute_step(StepId::new("step-slow".to_string()), 2999).await;
    assert!(matches!(res, Err(ExecuteNodeError::TimeoutExceeded { .. })));

    let res = execute_step(StepId::new("step-slow".to_string()), 5000).await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn qa_timeout_fast_step_minimal_timeout() {
    let _guard = lock_and_reset();
    let res = execute_step(StepId::new("step-1".to_string()), 1).await;
    assert!(res.is_ok());
}

// ── Retry policy construction ──────────────────────────────────────────────

#[test]
fn qa_retry_policy_construction() {
    let p = RetryPolicy::new(3, 100, 2.0).unwrap();
    assert_eq!((p.max_attempts, p.backoff_ms, p.max_backoff_ms), (3, 100, u64::MAX));
    assert!(RetryPolicy::with_max_backoff(5, 100, 2.0, 1000).is_ok());
    assert!(RetryPolicy::with_max_backoff(3, 100, 2.0, 100).is_ok());
    assert!(RetryPolicy::new(1, 0, 1.0).is_ok());

    assert_eq!(RetryPolicy::new(0, 100, 2.0).unwrap_err(), RetryPolicyError::ZeroAttempts);
    assert!(matches!(RetryPolicy::new(3, 100, f64::NAN), Err(RetryPolicyError::InvalidMultiplier { .. })));
    assert!(matches!(RetryPolicy::new(3, 100, 0.5), Err(RetryPolicyError::InvalidMultiplier { .. })));
    assert!(matches!(
        RetryPolicy::with_max_backoff(3, 100, 2.0, 50),
        Err(RetryPolicyError::MaxBackoffTooSmall { max: 50, ms: 100 })
    ));
}

// ── Backoff calculation ────────────────────────────────────────────────────

#[test]
fn qa_backoff_calculation() {
    let p = RetryPolicy::new(10, 100, 2.0).unwrap();
    assert_eq!(p.calculate_backoff_delay(0), 0);
    assert_eq!(p.calculate_backoff_delay(1), 100);
    assert_eq!(p.calculate_backoff_delay(2), 200);
    assert_eq!(p.calculate_backoff_delay(3), 400);
    assert_eq!(p.calculate_backoff_delay(5), 1600);

    let pz = RetryPolicy::new(5, 0, 2.0).unwrap();
    assert_eq!(pz.calculate_backoff_delay(1), 0);

    let pc = RetryPolicy::with_max_backoff(10, 100, 10.0, 500).unwrap();
    assert_eq!(pc.calculate_backoff_delay(1), 100);
    assert_eq!(pc.calculate_backoff_delay(2), 500);
    assert_eq!(pc.calculate_backoff_delay(3), 500);
}

// ── Retry execution ────────────────────────────────────────────────────────

#[tokio::test]
async fn qa_retry_success_step_passes_immediately() {
    let _guard = lock_and_reset();
    let pol = RetryPolicy::new(3, 10, 2.0).unwrap();
    let r = execute_step_with_retry(StepId::new("step-1".to_string()), 5000, pol).await;
    assert!(r.is_ok());
}

#[tokio::test]
async fn qa_retry_flaky_exhausts_all_attempts() {
    let _guard = lock_and_reset();

    let r2 = execute_step_with_retry(
        StepId::new("step-flaky".to_string()), 5000,
        RetryPolicy::new(3, 10, 2.0).unwrap(),
    ).await;
    assert!(matches!(r2, Err(ExecuteNodeError::RetryExhausted { attempts: 3, .. })));

    reset_all_state();

    let r3 = execute_step_with_retry(
        StepId::new("step-flaky".to_string()), 5000,
        RetryPolicy::new(1, 10, 2.0).unwrap(),
    ).await;
    assert!(matches!(r3, Err(ExecuteNodeError::RetryExhausted { attempts: 1, .. })));

    reset_all_state();

    let r4 = execute_step_with_retry(
        StepId::new("nope".to_string()), 5000,
        RetryPolicy::new(3, 10, 2.0).unwrap(),
    ).await;
    assert!(matches!(r4, Err(ExecuteNodeError::StepNotFound { .. })));
}

#[tokio::test]
async fn qa_retry_with_invalid_timeout_rejects() {
    let _guard = lock_and_reset();
    let pol = RetryPolicy::new(3, 10, 2.0).unwrap();
    let res = execute_step_with_retry(StepId::new("step-1".to_string()), 0, pol).await;
    assert!(matches!(res, Err(ExecuteNodeError::InvalidTimeout { .. })));
}

// ── Runtime (sync wrapper) ─────────────────────────────────────────────────

#[test]
fn qa_runtime_api_surface() {
    let _guard = lock_and_reset();
    let rt = Runtime::new().unwrap();

    assert!(rt.execute_step_sync(StepId::new("step-1".to_string()), 5000).is_ok());
    reset_all_state();
    assert!(!rt.execute_step_sync(StepId::new("step-fail".to_string()), 5000).unwrap().is_success());
    assert!(matches!(
        rt.execute_step_sync(StepId::new("nope".to_string()), 5000),
        Err(ExecuteNodeError::StepNotFound { .. })
    ));
    assert!(matches!(
        rt.execute_step_sync(StepId::new("step-1".to_string()), 0),
        Err(ExecuteNodeError::InvalidTimeout { .. })
    ));
    reset_all_state();
    assert!(rt.execute_step_with_retry_sync(
        StepId::new("step-1".to_string()), 5000,
        RetryPolicy::new(3, 10, 2.0).unwrap(),
    ).is_ok());
    assert_eq!(rt.get_status(&StepId::new("step-1".to_string())), ExecutionStatus::Ready);
    assert!(rt.cancel(StepId::new("step-1".to_string())).is_ok());
}

// ── Step context ───────────────────────────────────────────────────────────

#[test]
fn qa_step_context_api_surface() {
    let _guard = lock_and_reset();
    let ctx = StepContext::new(StepId::new("step-1".to_string())).unwrap();
    assert!(ctx.execute(5000).is_ok());
    assert_eq!(ctx.status(), ExecutionStatus::Ready);

    reset_all_state();
    let ctx_f = StepContext::new(StepId::new("step-fail".to_string())).unwrap();
    assert!(!ctx_f.execute(5000).unwrap().is_success());

    reset_all_state();
    let ctx_r = StepContext::new(StepId::new("step-1".to_string())).unwrap();
    assert!(ctx_r.execute_with_retry(5000, RetryPolicy::new(3, 10, 2.0).unwrap()).is_ok());

    reset_all_state();
    let ctx_c = StepContext::new(StepId::new("step-1".to_string())).unwrap();
    assert!(ctx_c.cancel().is_ok());
    assert!(ctx_c.last_error().is_none());
}

// ── Concurrent execution ───────────────────────────────────────────────────

#[tokio::test]
async fn qa_concurrent_state_machine_contract() {
    let _guard = lock_and_reset();

    set_executing_state_for_test("step-1");
    let r = execute_step(StepId::new("step-1".to_string()), 5000).await;
    assert!(matches!(r, Err(ExecuteNodeError::InvalidTransition { .. })));

    reset_all_state();

    let step = StepId::new("step-1".to_string());
    assert!(execute_step(step.clone(), 5000).await.is_ok());
    assert!(execute_step(step, 5000).await.is_ok());
}

// ── Error semantics ────────────────────────────────────────────────────────

#[test]
fn qa_error_types_and_display() {
    let err = ExecuteNodeError::StepNotFound { step_id: StepId::new("x".into()) };
    assert_eq!(err, err.clone());

    let nested = ExecuteNodeError::RetryExhausted {
        attempts: 5,
        last_error: Box::new(ExecuteNodeError::TransientError {
            reason: "conn reset".into(), recoverable: true,
        }),
    };
    let msg = nested.to_string();
    assert!(msg.contains("5") && msg.contains("conn reset"));

    assert_eq!(RetryPolicyError::ZeroAttempts.to_string(), "Zero attempts not allowed");
    assert_ne!(
        RetryPolicyError::InvalidMultiplier { got: 1.5 },
        RetryPolicyError::InvalidMultiplier { got: 2.0 }
    );
}

// ── Step ID and result types ───────────────────────────────────────────────

#[test]
fn qa_step_id_and_result_types() {
    for case in ["step-1", "step_2", "abc123", "UPPER", "x"] {
        assert!(StepId::parse(case).is_ok());
    }
    for case in ["", "has space", "a@b"] {
        assert!(StepId::parse(case).is_err());
    }
    assert_eq!(format!("{}", StepId::new("my".into())), "my");
    let s: String = StepId::new("c".into()).into();
    assert_eq!(s, "c");

    let r = StepResult::Success { output: "data".into() };
    let json = serde_json::to_string(&r).unwrap();
    assert_eq!(serde_json::from_str::<StepResult>(&json).unwrap(), r);
    assert!(!StepResult::Failure { output: "e".into() }.is_success());
}

// ── Temp directory isolation ───────────────────────────────────────────────

#[tokio::test]
async fn qa_temp_dir_isolation() {
    let _guard = lock_and_reset();
    let _tmp = TempDir::new().unwrap();
    assert!(execute_step(StepId::new("step-1".to_string()), 5000).await.is_ok());
}

// ── State reset ────────────────────────────────────────────────────────────

#[tokio::test]
async fn qa_state_reset_clears_all() {
    let _guard = lock_and_reset();
    let _ = execute_step(StepId::new("step-transient".to_string()), 5000).await;
    reset_all_state();
    assert!(get_last_error(&StepId::new("step-transient".to_string())).is_none());
    assert_eq!(get_execution_status(&StepId::new("step-1".to_string())), ExecutionStatus::Ready);
}
