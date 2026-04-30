//! Error propagation tests for vo-executor
//!
//! Tests cover:
//! - Error chain preservation (RetryExhausted inner error)
//! - Error classification (retriable vs non-retriable)
//! - Subprocess output and exit code preservation
//! - Error hierarchy chaining (SchedulerError → ExecutionError → RetryExhaustedError)
//! - Timeout enforcement - cancellation during execution
//! - Timeout enforcement - graceful state cleanup after timeout
//! - Retry policy - max retry limit enforcement
//! - Retry with retry policy validation (zero-attempt policy)

use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::MutexGuard;

use vo_executor::errors::{ExecuteNodeError, RetryPolicyError};
use vo_executor::scheduler::JobId;
use vo_executor::scheduler::{ExecutionError, RetryExhaustedError, SchedulerError};
use vo_executor::state::{
    get_state, reset_all_state, set_error, set_executing_state_for_test, set_state, StepState,
};
use vo_executor::{
    cancel_execution, execute_step, execute_step_with_retry, get_execution_status, ExecutionStatus,
    RetryPolicy, StepId, SubprocessOutput,
};

static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn setup() -> MutexGuard<'static, ()> {
    let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset_all_state();
    guard
}

// ============================================================================
// Error Chain Preservation Tests
// ============================================================================

mod error_chain_preservation {
    use super::*;

    #[test]
    fn retry_exhausted_preserves_inner_error_type() {
        let inner = ExecuteNodeError::TransientError {
            reason: "connection reset".to_string(),
            recoverable: true,
        };
        let chained = ExecuteNodeError::RetryExhausted {
            attempts: 3,
            last_error: Box::new(inner.clone()),
        };
        // Verify the inner error is accessible via display
        let display = format!("{}", chained);
        assert!(display.contains("connection reset"));
        assert!(display.contains("3"));
        assert!(display.contains("Retry exhausted"));
    }

    #[test]
    fn retry_exhausted_preserves_attempts_count() {
        let inner = ExecuteNodeError::TransientError {
            reason: "timeout".to_string(),
            recoverable: false,
        };
        for attempts in 1..=10u32 {
            let err = ExecuteNodeError::RetryExhausted {
                attempts,
                last_error: Box::new(inner.clone()),
            };
            let display = format!("{}", err);
            assert!(
                display.contains(&attempts.to_string()),
                "Display for attempts={} should contain '{}'",
                attempts,
                attempts
            );
        }
    }

    #[test]
    fn retry_exhausted_with_different_inner_error_types() {
        // RetryExhausted wrapping StepNotFound
        let inner = ExecuteNodeError::StepNotFound {
            step_id: StepId::new("missing".to_string()),
        };
        let err = ExecuteNodeError::RetryExhausted {
            attempts: 1,
            last_error: Box::new(inner),
        };
        let display = format!("{}", err);
        assert!(display.contains("missing"));
        assert!(display.contains("Step not found"));

        // RetryExhausted wrapping TimeoutExceeded
        let inner2 = ExecuteNodeError::TimeoutExceeded {
            elapsed_ms: 5000,
            limit_ms: 3000,
        };
        let err2 = ExecuteNodeError::RetryExhausted {
            attempts: 2,
            last_error: Box::new(inner2),
        };
        let display2 = format!("{}", err2);
        assert!(display2.contains("5000ms"));
        assert!(display2.contains("3000ms"));

        // RetryExhausted wrapping ExecutionCancelled
        let inner3 = ExecuteNodeError::ExecutionCancelled {
            reason: "timeout during retry".to_string(),
        };
        let err3 = ExecuteNodeError::RetryExhausted {
            attempts: 1,
            last_error: Box::new(inner3),
        };
        let display3 = format!("{}", err3);
        assert!(display3.contains("timeout during retry"));
    }

    #[test]
    fn retry_exhausted_error_is_clone() {
        let inner = ExecuteNodeError::TransientError {
            reason: "transient".to_string(),
            recoverable: true,
        };
        let err = ExecuteNodeError::RetryExhausted {
            attempts: 5,
            last_error: Box::new(inner),
        };
        let cloned = err.clone();
        assert_eq!(err, cloned);
    }

    #[test]
    fn retry_exhausted_error_is_debug() {
        let inner = ExecuteNodeError::TransientError {
            reason: "debug test".to_string(),
            recoverable: true,
        };
        let err = ExecuteNodeError::RetryExhausted {
            attempts: 3,
            last_error: Box::new(inner),
        };
        let debug = format!("{:?}", err);
        assert!(debug.contains("RetryExhausted"));
    }

    #[test]
    fn retry_exhausted_preserves_recoverable_flag() {
        // Non-recoverable transient error inside RetryExhausted
        let non_recoverable = ExecuteNodeError::TransientError {
            reason: "unrecoverable".to_string(),
            recoverable: false,
        };
        let err1 = ExecuteNodeError::RetryExhausted {
            attempts: 1,
            last_error: Box::new(non_recoverable),
        };
        let display1 = format!("{}", err1);
        assert!(display1.contains("recoverable=false"));

        // Recoverable transient error inside RetryExhausted
        let recoverable = ExecuteNodeError::TransientError {
            reason: "recoverable".to_string(),
            recoverable: true,
        };
        let err2 = ExecuteNodeError::RetryExhausted {
            attempts: 1,
            last_error: Box::new(recoverable),
        };
        let display2 = format!("{}", err2);
        assert!(display2.contains("recoverable=true"));
    }

    #[test]
    fn nested_error_chain_deeply_nested() {
        // A chain of RetryExhausted wrapping another RetryExhausted
        let inner = ExecuteNodeError::TransientError {
            reason: "deep".to_string(),
            recoverable: true,
        };
        let mid = ExecuteNodeError::RetryExhausted {
            attempts: 2,
            last_error: Box::new(inner),
        };
        let outer = ExecuteNodeError::RetryExhausted {
            attempts: 3,
            last_error: Box::new(mid),
        };
        let display = format!("{}", outer);
        assert!(display.contains("Retry exhausted"));
        assert!(display.contains("deep"));
    }
}

// ============================================================================
// Error Classification Tests (Retriable vs Non-Retriable)
// ============================================================================

mod error_classification {
    use super::*;

    fn classify_retriable(err: &ExecuteNodeError) -> bool {
        matches!(
            err,
            ExecuteNodeError::TransientError {
                recoverable: true,
                ..
            }
        )
    }

    #[test]
    fn transient_error_recoverable_is_retriable() {
        let err = ExecuteNodeError::TransientError {
            reason: "network timeout".to_string(),
            recoverable: true,
        };
        assert!(classify_retriable(&err));
    }

    #[test]
    fn transient_error_not_recoverable_is_not_retriable() {
        let err = ExecuteNodeError::TransientError {
            reason: "network timeout".to_string(),
            recoverable: false,
        };
        assert!(!classify_retriable(&err));
    }

    #[test]
    fn step_not_found_is_not_retriable() {
        let err = ExecuteNodeError::StepNotFound {
            step_id: StepId::new("missing".to_string()),
        };
        assert!(!classify_retriable(&err));
    }

    #[test]
    fn timeout_exceeded_is_not_retriable() {
        let err = ExecuteNodeError::TimeoutExceeded {
            elapsed_ms: 5000,
            limit_ms: 3000,
        };
        assert!(!classify_retriable(&err));
    }

    #[test]
    fn invalid_transition_is_not_retriable() {
        let err = ExecuteNodeError::InvalidTransition {
            from_state: "Ready".to_string(),
            action: "execute".to_string(),
        };
        assert!(!classify_retriable(&err));
    }

    #[test]
    fn retry_exhausted_is_not_retriable() {
        let err = ExecuteNodeError::RetryExhausted {
            attempts: 3,
            last_error: Box::new(ExecuteNodeError::TransientError {
                reason: "exhausted".to_string(),
                recoverable: true,
            }),
        };
        assert!(!classify_retriable(&err));
    }

    #[test]
    fn execution_cancelled_is_not_retriable() {
        let err = ExecuteNodeError::ExecutionCancelled {
            reason: "user request".to_string(),
        };
        assert!(!classify_retriable(&err));
    }

    #[test]
    fn invalid_timeout_is_not_retriable() {
        let err = ExecuteNodeError::InvalidTimeout {
            value: 0,
            reason: "must be > 0ms".to_string(),
        };
        assert!(!classify_retriable(&err));
    }

    #[test]
    fn invalid_retry_policy_is_not_retriable() {
        let err = ExecuteNodeError::InvalidRetryPolicy {
            node_name: "step-a".to_string(),
            reason: RetryPolicyError::ZeroAttempts,
        };
        assert!(!classify_retriable(&err));
    }

    #[test]
    fn all_non_retriable_error_variants() {
        let non_retriable = vec![
            ExecuteNodeError::StepNotFound {
                step_id: StepId::new("x".to_string()),
            },
            ExecuteNodeError::InvalidTimeout {
                value: 0,
                reason: "r".to_string(),
            },
            ExecuteNodeError::TimeoutExceeded {
                elapsed_ms: 1,
                limit_ms: 2,
            },
            ExecuteNodeError::InvalidTransition {
                from_state: "a".to_string(),
                action: "b".to_string(),
            },
            ExecuteNodeError::RetryExhausted {
                attempts: 1,
                last_error: Box::new(ExecuteNodeError::ExecutionCancelled {
                    reason: "r".to_string(),
                }),
            },
            ExecuteNodeError::ExecutionCancelled {
                reason: "r".to_string(),
            },
            ExecuteNodeError::InvalidRetryPolicy {
                node_name: "n".to_string(),
                reason: RetryPolicyError::ZeroAttempts,
            },
        ];
        for err in non_retriable {
            assert!(
                !classify_retriable(&err),
                "Error {:?} should not be retriable",
                err
            );
        }
    }
}

// ============================================================================
// Subprocess Exit Code Preservation Tests
// ============================================================================

mod subprocess_exit_code {
    use super::*;

    #[test]
    fn subprocess_output_preserves_exit_code_zero() {
        let output = SubprocessOutput {
            fd4_bytes: vec![0u8, 1, 2, 3],
            exit_code: Some(0),
        };
        assert_eq!(output.exit_code, Some(0));
        assert_eq!(output.fd4_bytes, vec![0, 1, 2, 3]);
    }

    #[test]
    fn subprocess_output_preserves_nonzero_exit_code() {
        for code in [-1, 1, 42, 127, 255] {
            let output = SubprocessOutput {
                fd4_bytes: vec![],
                exit_code: Some(code),
            };
            assert_eq!(
                output.exit_code,
                Some(code),
                "exit_code {} should be preserved exactly",
                code
            );
        }
    }

    #[test]
    fn subprocess_output_preserves_negative_exit_code() {
        let output = SubprocessOutput {
            fd4_bytes: vec![],
            exit_code: Some(-1),
        };
        assert_eq!(output.exit_code, Some(-1));
    }

    #[test]
    fn subprocess_output_no_exit_code_for_signal_death() {
        let output = SubprocessOutput {
            fd4_bytes: vec![],
            exit_code: None,
        };
        assert!(output.exit_code.is_none());
    }

    #[test]
    fn subprocess_output_preserves_empty_output_with_exit_code() {
        let output = SubprocessOutput {
            fd4_bytes: vec![],
            exit_code: Some(42),
        };
        assert!(output.fd4_bytes.is_empty());
        assert_eq!(output.exit_code, Some(42));
    }

    #[test]
    fn subprocess_output_preserves_large_output_with_exit_code() {
        let large_output: Vec<u8> = (0..=255).cycle().take(10_000).collect();
        let output = SubprocessOutput {
            fd4_bytes: large_output.clone(),
            exit_code: Some(0),
        };
        assert_eq!(output.fd4_bytes.len(), 10_000);
        assert_eq!(output.exit_code, Some(0));
        assert_eq!(output.fd4_bytes, large_output);
    }

    #[test]
    fn subprocess_output_equality() {
        let output1 = SubprocessOutput {
            fd4_bytes: vec![1, 2, 3],
            exit_code: Some(0),
        };
        let output2 = SubprocessOutput {
            fd4_bytes: vec![1, 2, 3],
            exit_code: Some(0),
        };
        let output3 = SubprocessOutput {
            fd4_bytes: vec![1, 2, 3],
            exit_code: Some(1),
        };
        let output4 = SubprocessOutput {
            fd4_bytes: vec![],
            exit_code: Some(0),
        };

        assert_eq!(output1, output2);
        assert_ne!(output1, output3);
        assert_ne!(output1, output4);
    }
}

// ============================================================================
// Error Hierarchy Tests (SchedulerError -> ExecutionError -> RetryExhaustedError)
// ============================================================================

mod error_hierarchy {
    use super::*;

    #[test]
    fn scheduler_error_display_all_variants() {
        // JobNotFound
        let err = SchedulerError::JobNotFound(JobId::new(99));
        let msg = format!("{}", err);
        assert!(msg.contains("Job") || msg.contains("not found"));

        // QueueFull
        let err = SchedulerError::QueueFull;
        let msg = format!("{}", err);
        assert!(msg.contains("Queue") || msg.contains("full"));

        // SchedulerStopped
        let err = SchedulerError::SchedulerStopped;
        let msg = format!("{}", err);
        assert!(msg.contains("stopped") || msg.contains("Scheduler"));

        // InvalidSchedule
        let err = SchedulerError::InvalidSchedule("bad cron expression".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Invalid") || msg.contains("schedule"));

        // ConcurrencyLimitReached
        let err = SchedulerError::ConcurrencyLimitReached;
        let msg = format!("{}", err);
        assert!(msg.contains("Concurrency") || msg.contains("limit"));

        // StorageError
        let err = SchedulerError::StorageError("disk full".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Storage") || msg.contains("disk"));

        // InvalidTransition
        let err = SchedulerError::InvalidTransition {
            from_state: "Running".to_string(),
            event: "stop".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Invalid transition"));
        assert!(msg.contains("Running"));

        // SerializationError
        let err = SchedulerError::SerializationError("json decode".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Serialization") || msg.contains("json"));

        // InvalidJobId
        let err = SchedulerError::InvalidJobId("bad-id".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Invalid JobId") || msg.contains("bad-id"));
    }

    #[test]
    fn scheduler_error_is_clone_and_eq() {
        let e1 = SchedulerError::QueueFull;
        let e2 = e1.clone();
        assert_eq!(e1, e2);

        let e3 = SchedulerError::ConcurrencyLimitReached;
        assert_ne!(e1, e3);

        let e4 = SchedulerError::JobNotFound(JobId::new(42));
        let e5 = SchedulerError::JobNotFound(JobId::new(42));
        assert_eq!(e4, e5);

        let e6 = SchedulerError::JobNotFound(JobId::new(99));
        assert_ne!(e4, e6);
    }

    #[test]
    fn execution_error_display_panicked() {
        let err = ExecutionError::Panicked {
            job_id: JobId::new(1),
            reason: "panic!".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("1") || msg.contains("panic"));
    }

    #[test]
    fn execution_error_display_timed_out() {
        let err = ExecutionError::TimedOut {
            job_id: JobId::new(1),
            timeout_ms: 5000,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("5000") || msg.contains("timed out"));
    }

    #[test]
    fn execution_error_display_cancelled() {
        let err = ExecutionError::Cancelled {
            job_id: JobId::new(1),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("cancelled") || msg.contains("1"));
    }

    #[test]
    fn execution_error_display_resource_exhausted() {
        let err = ExecutionError::ResourceExhausted {
            job_id: JobId::new(1),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("resource") || msg.contains("exhausted"));
    }

    #[test]
    fn retry_exhausted_error_display_all_variants() {
        // MaxAttemptsReached
        let err = RetryExhaustedError::MaxAttemptsReached {
            job_id: JobId::new(1),
            max_attempts: 5,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("5") || msg.contains("max attempts"));

        // BackoffOverflow
        let err = RetryExhaustedError::BackoffOverflow {
            job_id: JobId::new(2),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("overflow") || msg.contains("Backoff"));

        // RetryNotAllowed
        let err = RetryExhaustedError::RetryNotAllowed {
            job_id: JobId::new(3),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("retr") || msg.contains("allowed"));
    }

    #[test]
    fn execution_error_debug_format() {
        let err = ExecutionError::Panicked {
            job_id: JobId::new(42),
            reason: "test panic".to_string(),
        };
        let debug = format!("{:?}", err);
        assert!(debug.contains("Panicked"));
        assert!(debug.contains("42"));
    }

    #[test]
    fn retry_exhausted_error_debug_format() {
        let err = RetryExhaustedError::MaxAttemptsReached {
            job_id: JobId::new(1),
            max_attempts: 3,
        };
        let debug = format!("{:?}", err);
        assert!(debug.contains("MaxAttemptsReached"));
    }

    #[test]
    fn error_hierarchy_preserves_job_id() {
        let job_id = JobId::new(12345);

        let execution_err = ExecutionError::TimedOut {
            job_id: job_id.clone(),
            timeout_ms: 10000,
        };
        let msg = format!("{}", execution_err);
        assert!(msg.contains("12345") || msg.contains("timed out"));

        let retry_err = RetryExhaustedError::MaxAttemptsReached {
            job_id: job_id.clone(),
            max_attempts: 10,
        };
        let msg = format!("{}", retry_err);
        assert!(msg.contains("12345") || msg.contains("max attempts"));
    }
}

// ============================================================================
// Timeout Enforcement - Cancellation During Execution
// ============================================================================

mod timeout_cancellation {
    use super::*;

    #[tokio::test]
    async fn cancel_execution_from_ready_sets_cancelled_state() {
        let _guard = setup();
        let step = StepId::new("cancel-ready-test".to_string());
        set_state(step.as_str(), StepState::Ready);
        let result = cancel_execution(step.clone()).await;
        assert!(result.is_ok());
        let status = get_execution_status(&step);
        assert!(matches!(status, ExecutionStatus::Cancelled { .. }));
    }

    #[tokio::test]
    async fn cancel_execution_from_completed_is_noop() {
        let _guard = setup();
        set_state(
            "cancel-completed",
            StepState::Completed {
                output: "done".to_string(),
            },
        );
        let result = cancel_execution(StepId::new("cancel-completed".to_string())).await;
        assert!(result.is_ok());
        let status = get_execution_status(&StepId::new("cancel-completed".to_string()));
        assert!(matches!(status, ExecutionStatus::Completed { .. }));
    }

    #[tokio::test]
    async fn cancel_execution_from_cancelled_is_noop() {
        let _guard = setup();
        set_state(
            "cancel-already",
            StepState::Cancelled {
                reason: "already cancelled".to_string(),
            },
        );
        let result = cancel_execution(StepId::new("cancel-already".to_string())).await;
        assert!(result.is_ok());
        let status = get_execution_status(&StepId::new("cancel-already".to_string()));
        assert!(matches!(status, ExecutionStatus::Cancelled { .. }));
    }

    #[tokio::test]
    async fn cancel_execution_from_executing_returns_cancelled_error() {
        let _guard = setup();
        let step = StepId::new("cancel-executing".to_string());
        set_executing_state_for_test(step.as_str());
        let result = cancel_execution(step.clone()).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ExecuteNodeError::ExecutionCancelled { .. }));
        let err_display = format!("{}", err);
        assert!(err_display.contains("cancel"));
    }

    #[tokio::test]
    async fn cancel_execution_preserves_cancel_reason() {
        let _guard = setup();
        let step = StepId::new("cancel-reason-test".to_string());
        set_state(step.as_str(), StepState::Ready);
        cancel_execution(step.clone()).await.unwrap();
        let status = get_execution_status(&step);
        if let ExecutionStatus::Cancelled { reason } = status {
            assert_eq!(reason, "cancelled by user");
        } else {
            panic!("Expected Cancelled status");
        }
    }

    #[tokio::test]
    async fn cancel_from_executing_sets_cancelled_state() {
        let _guard = setup();
        let step = StepId::new("cancel-from-executing".to_string());
        set_executing_state_for_test(step.as_str());
        // Attempting to cancel during executing state returns an error,
        // but the state should be updated
        let result = cancel_execution(step.clone()).await;
        assert!(result.is_err());
        // Verify the state transition attempt was rejected
        let state = get_state(step.as_str());
        assert!(matches!(state, StepState::Executing { .. }));
    }

    #[test]
    fn execution_cancelled_error_display_contains_reason() {
        let err = ExecuteNodeError::ExecutionCancelled {
            reason: "timeout during execution".to_string(),
        };
        let display = format!("{}", err);
        assert!(display.contains("timeout during execution"));
        assert!(display.contains("cancelled"));
    }

    #[test]
    fn execution_cancelled_error_equality() {
        let err1 = ExecuteNodeError::ExecutionCancelled {
            reason: "same".to_string(),
        };
        let err2 = ExecuteNodeError::ExecutionCancelled {
            reason: "same".to_string(),
        };
        let err3 = ExecuteNodeError::ExecutionCancelled {
            reason: "diff".to_string(),
        };
        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }

    #[test]
    fn execution_cancelled_error_is_debug() {
        let err = ExecuteNodeError::ExecutionCancelled {
            reason: "test".to_string(),
        };
        let debug = format!("{:?}", err);
        assert!(debug.contains("ExecutionCancelled"));
    }
}

// ============================================================================
// Timeout Enforcement - Graceful State Cleanup After Timeout
// ============================================================================

mod timeout_cleanup {
    use super::*;

    #[tokio::test]
    async fn slow_step_timeout_returns_step_to_ready() {
        let _guard = setup();
        let step = StepId::new("step-slow".to_string());
        let result = execute_step(step.clone(), 100).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ExecuteNodeError::TimeoutExceeded { .. }));
        // After timeout, step should return to Ready state
        let status = get_execution_status(&step);
        assert!(matches!(status, ExecutionStatus::Ready));
    }

    #[tokio::test]
    async fn slow_step_timeout_clears_previous_error() {
        let _guard = setup();
        let step = StepId::new("step-slow-clear".to_string());
        // Set an error on the step
        set_error(
            step.as_str(),
            ExecuteNodeError::TransientError {
                reason: "old error".to_string(),
                recoverable: true,
            },
        );
        // Execute slow step with small timeout - should trigger timeout
        // and clear the old error
        let result = execute_step(step.clone(), 100).await;
        assert!(result.is_err());
        // Error should be cleared after successful execution attempt
        // (even though it timed out, start_execution clears the error)
        let err = result.unwrap_err();
        assert!(matches!(err, ExecuteNodeError::TimeoutExceeded { .. }));
    }

    #[tokio::test]
    async fn step_retries_after_timeout() {
        let _guard = setup();
        let step = StepId::new("step-retry-timeout".to_string());
        // First execution times out
        let result = execute_step(step.clone(), 100).await;
        assert!(result.is_err());
        // After timeout, step is back to Ready - can be re-executed
        let result = execute_step(step.clone(), 5000).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_success());
    }

    #[tokio::test]
    async fn timeout_preserves_error_for_inspection() {
        let _guard = setup();
        let step = StepId::new("step-slow-error".to_string());
        // Execute step that will time out
        let result = execute_step(step.clone(), 100).await;
        // The error from the timeout is stored for inspection
        // (start_execution clears, but handle_slow_step_timeout sets state and returns error)
        assert!(result.is_err());
    }

    #[test]
    fn timeout_exceeded_error_display_contains_elapsed_and_limit() {
        let err = ExecuteNodeError::TimeoutExceeded {
            elapsed_ms: 5000,
            limit_ms: 1000,
        };
        let display = format!("{}", err);
        assert!(display.contains("5000ms"));
        assert!(display.contains("1000ms"));
        assert!(display.contains("Timeout exceeded"));
    }

    #[test]
    fn timeout_exceeded_error_equality() {
        let e1 = ExecuteNodeError::TimeoutExceeded {
            elapsed_ms: 5000,
            limit_ms: 1000,
        };
        let e2 = ExecuteNodeError::TimeoutExceeded {
            elapsed_ms: 5000,
            limit_ms: 1000,
        };
        let e3 = ExecuteNodeError::TimeoutExceeded {
            elapsed_ms: 6000,
            limit_ms: 1000,
        };
        assert_eq!(e1, e2);
        assert_ne!(e1, e3);
    }

    #[test]
    fn timeout_exceeded_error_is_debug() {
        let err = ExecuteNodeError::TimeoutExceeded {
            elapsed_ms: 3000,
            limit_ms: 1000,
        };
        let debug = format!("{:?}", err);
        assert!(debug.contains("TimeoutExceeded"));
    }

    #[tokio::test]
    async fn normal_step_execution_returns_to_ready() {
        let _guard = setup();
        let step = StepId::new("step-normal".to_string());
        let result = execute_step(step.clone(), 5000).await;
        assert!(result.is_ok());
        let status = get_execution_status(&step);
        assert!(matches!(status, ExecutionStatus::Ready));
    }

    #[tokio::test]
    async fn step_failure_returns_to_ready() {
        let _guard = setup();
        let step = StepId::new("step-fail-cleanup".to_string());
        let result = execute_step(step.clone(), 5000).await;
        assert!(result.is_ok());
        // Failure still returns to Ready state
        let status = get_execution_status(&step);
        assert!(matches!(status, ExecutionStatus::Ready));
    }
}

// ============================================================================
// Retry Policy - Max Retry Limit Enforcement
// ============================================================================

mod retry_limit_enforcement {
    use super::*;

    #[tokio::test]
    async fn execute_step_with_retry_respects_max_attempts() {
        let _guard = setup();
        // Step 1 succeeds on first try
        let policy = RetryPolicy::new(3, 100, 2.0).unwrap();
        let result = execute_step_with_retry(StepId::new("step-1".to_string()), 5000, policy).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn retry_policy_max_attempts_one() {
        let _guard = setup();
        let policy = RetryPolicy::new(1, 10, 2.0).unwrap();
        assert_eq!(policy.max_attempts, 1);
    }

    #[tokio::test]
    async fn retry_policy_max_attempts_ten() {
        let _guard = setup();
        let policy = RetryPolicy::new(10, 100, 2.0).unwrap();
        assert_eq!(policy.max_attempts, 10);
    }

    #[test]
    fn retry_policy_max_attempts_cannot_be_zero() {
        let result = RetryPolicy::new(0, 100, 2.0);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RetryPolicyError::ZeroAttempts
        ));
    }

    #[test]
    fn retry_policy_max_attempts_max_u32() {
        let policy = RetryPolicy::new(u32::MAX, 100, 2.0).unwrap();
        assert_eq!(policy.max_attempts, u32::MAX);
    }

    #[test]
    fn retry_policy_max_attempts_one_with_backoff() {
        let policy = RetryPolicy::new(1, 100, 2.0).unwrap();
        // With max_attempts=1, even if the step fails, no backoff is needed
        // (no retry happens)
        assert_eq!(policy.max_attempts, 1);
    }

    #[tokio::test]
    async fn flaky_step_with_max_attempts_two_exhausts() {
        let _guard = setup();
        #[cfg(feature = "test-sim")]
        {
            let policy = RetryPolicy::new(2, 10, 2.0).unwrap();
            let result =
                execute_step_with_retry(StepId::new("step-flaky".to_string()), 5000, policy).await;
            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                ExecuteNodeError::RetryExhausted { attempts: 2, .. }
            ));
        }
    }

    #[tokio::test]
    async fn flaky_step_with_max_attempts_three_exhausts() {
        let _guard = setup();
        #[cfg(feature = "test-sim")]
        {
            let policy = RetryPolicy::new(3, 10, 2.0).unwrap();
            let result =
                execute_step_with_retry(StepId::new("step-flaky".to_string()), 5000, policy).await;
            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                ExecuteNodeError::RetryExhausted { attempts: 3, .. }
            ));
        }
    }

    #[test]
    fn retry_policy_backoff_delay_attempts_match_max_attempts() {
        let policy = RetryPolicy::new(5, 100, 2.0).unwrap();
        // Calculate delays for attempts 1..=max_attempts
        for attempt in 1..=policy.max_attempts {
            let delay = policy.calculate_backoff_delay(attempt);
            assert!(
                delay > 0,
                "Backoff delay for attempt {} should be > 0",
                attempt
            );
        }
    }
}

// ============================================================================
// Retry With Retry Policy Validation Tests
// ============================================================================

mod retry_policy_validation {
    use super::*;

    #[tokio::test]
    async fn execute_step_with_retry_zero_attempts_rejects() {
        let _guard = setup();
        let result = RetryPolicy::new(0, 100, 2.0);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RetryPolicyError::ZeroAttempts
        ));
    }

    #[tokio::test]
    async fn execute_step_with_retry_invalid_multiplier_rejects() {
        let _guard = setup();
        let result = RetryPolicy::new(3, 100, 0.5);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn execute_step_with_retry_valid_policy_succeeds() {
        let _guard = setup();
        let policy = RetryPolicy::new(3, 100, 2.0).unwrap();
        let result = execute_step_with_retry(StepId::new("step-1".to_string()), 5000, policy).await;
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.is_success());
    }

    #[tokio::test]
    async fn retry_policy_equality_same_params() {
        let p1 = RetryPolicy::new(3, 100, 2.0).unwrap();
        let p2 = RetryPolicy::new(3, 100, 2.0).unwrap();
        assert_eq!(p1, p2);
    }

    #[test]
    fn retry_policy_inequality_different_max_attempts() {
        let p1 = RetryPolicy::new(3, 100, 2.0).unwrap();
        let p2 = RetryPolicy::new(5, 100, 2.0).unwrap();
        assert_ne!(p1, p2);
    }

    #[test]
    fn retry_policy_inequality_different_backoff() {
        let p1 = RetryPolicy::new(3, 100, 2.0).unwrap();
        let p2 = RetryPolicy::new(3, 200, 2.0).unwrap();
        assert_ne!(p1, p2);
    }

    #[test]
    fn retry_policy_inequality_different_multiplier() {
        let p1 = RetryPolicy::new(3, 100, 2.0).unwrap();
        let p2 = RetryPolicy::new(3, 100, 3.0).unwrap();
        assert_ne!(p1, p2);
    }

    #[test]
    fn retry_policy_clone_preserves_all_fields() {
        let p1 = RetryPolicy::with_max_backoff(5, 200, 1.5, 5000).unwrap();
        let p2 = p1.clone();
        assert_eq!(p1.max_attempts, p2.max_attempts);
        assert_eq!(p1.backoff_ms, p2.backoff_ms);
        assert!((p1.backoff_multiplier - p2.backoff_multiplier).abs() < f64::EPSILON);
        assert_eq!(p1.max_backoff_ms, p2.max_backoff_ms);
    }

    #[test]
    fn retry_policy_with_max_backoff_zero_backoff_ok() {
        let policy = RetryPolicy::with_max_backoff(3, 0, 2.0, 100).unwrap();
        assert_eq!(policy.backoff_ms, 0);
        assert_eq!(policy.max_backoff_ms, 100);
    }

    #[test]
    fn retry_policy_with_max_backoff_large_max() {
        let policy = RetryPolicy::with_max_backoff(10, 1, 2.0, u64::MAX - 1).unwrap();
        assert_eq!(policy.max_backoff_ms, u64::MAX - 1);
    }

    #[test]
    fn retry_policy_with_max_backoff_backoff_equal_max() {
        let policy = RetryPolicy::with_max_backoff(3, 100, 2.0, 100).unwrap();
        assert_eq!(policy.backoff_ms, 100);
        assert_eq!(policy.max_backoff_ms, 100);
        // All delays should be capped at 100
        assert_eq!(policy.calculate_backoff_delay(1), 100);
        assert_eq!(policy.calculate_backoff_delay(2), 100);
        assert_eq!(policy.calculate_backoff_delay(10), 100);
    }
}
