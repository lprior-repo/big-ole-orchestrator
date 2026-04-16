//! BLACK-HAT adversarial tests for vo-executor execution layer.
//!
//! Attack surface:
//! - Step timeout bypass: attempts to skip, overflow, or confuse timeout enforcement
//! - Retry exhaustion: pathological retry policies, backoff overflow, resource drain
//! - Concurrent step interference: race conditions on shared DashMap state
//!
//! Each test uses a fresh temp directory to isolate filesystem side-effects.

use std::sync::{Arc, Barrier, LazyLock, Mutex, MutexGuard};
use tempfile::tempdir;

use vo_executor::{
    cancel_execution, execute_step, execute_step_with_retry,
    get_execution_status, get_last_error,
    ExecutionStatus, RetryPolicy, StepId,
};
use vo_executor::errors::ExecuteNodeError;
use vo_executor::state::{
    clear_error, get_error_count, get_state, get_state_count, reset_all_state, set_error,
    set_state, set_executing_state_for_test, StepState,
};

static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn setup() -> MutexGuard<'static, ()> {
    let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset_all_state();
    guard
}

mod timeout_bypass {
    use super::*;

    #[tokio::test]
    async fn zero_timeout_is_rejected() {
        let _guard = setup();
        let _dir = tempdir().unwrap();
        let result = execute_step(StepId::new("step-1".to_string()), 0).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ExecuteNodeError::InvalidTimeout { value: 0, .. }));
    }

    #[tokio::test]
    async fn u64_max_timeout_is_rejected() {
        let _guard = setup();
        let _dir = tempdir().unwrap();
        let result = execute_step(StepId::new("step-1".to_string()), u64::MAX).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ExecuteNodeError::InvalidTimeout { value: u64::MAX, .. }));
    }

    #[tokio::test]
    async fn u64_max_minus_one_is_accepted() {
        let _guard = setup();
        let _dir = tempdir().unwrap();
        let result = execute_step(StepId::new("step-1".to_string()), u64::MAX - 1).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn slow_step_with_one_ms_timeout_fails() {
        let _guard = setup();
        let _dir = tempdir().unwrap();
        let result = execute_step(StepId::new("step-slow".to_string()), 1).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ExecuteNodeError::TimeoutExceeded { .. }));
    }

    #[tokio::test]
    async fn slow_step_with_exact_threshold_succeeds() {
        let _guard = setup();
        let _dir = tempdir().unwrap();
        let result = execute_step(StepId::new("step-slow".to_string()), 3000).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn slow_step_with_2999ms_timeout_fails() {
        let _guard = setup();
        let _dir = tempdir().unwrap();
        let result = execute_step(StepId::new("step-slow".to_string()), 2999).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ExecuteNodeError::TimeoutExceeded { elapsed_ms: 3000, limit_ms: 2999 }));
    }

    #[tokio::test]
    async fn timeout_does_not_orphan_executing_state() {
        let _guard = setup();
        let _dir = tempdir().unwrap();
        let _ = execute_step(StepId::new("step-slow".to_string()), 100).await;
        let state = get_state("step-slow");
        assert!(matches!(state, StepState::Ready), "timeout must restore Ready state, got: {:?}", state);
    }

    #[tokio::test]
    async fn timeout_does_not_leak_error() {
        let _guard = setup();
        let _dir = tempdir().unwrap();
        let _ = execute_step(StepId::new("step-slow".to_string()), 100).await;
        assert!(get_last_error(&StepId::new("step-slow".to_string())).is_none());
        assert_eq!(get_error_count(), 0);
    }

    #[tokio::test]
    async fn repeated_timeouts_do_not_accumulate_state() {
        let _guard = setup();
        let _dir = tempdir().unwrap();
        for _ in 0..50 {
            let _ = execute_step(StepId::new("step-slow".to_string()), 100).await;
        }
        assert_eq!(get_state_count(), 1);
        assert_eq!(get_error_count(), 0);
    }
}

mod retry_exhaustion {
    use super::*;

    #[tokio::test]
    async fn single_attempt_exhausts_immediately() {
        let _guard = setup();
        let _dir = tempdir().unwrap();
        let policy = RetryPolicy::new(1, 10, 2.0).unwrap();
        let result = execute_step_with_retry(StepId::new("step-flaky".to_string()), 5000, policy).await;
        assert!(result.is_err());
        if let ExecuteNodeError::RetryExhausted { attempts, .. } = result.unwrap_err() {
            assert_eq!(attempts, 1);
        } else {
            panic!("expected RetryExhausted");
        }
    }

    #[tokio::test]
    async fn hundred_attempts_still_exhausts() {
        let _guard = setup();
        let _dir = tempdir().unwrap();
        let policy = RetryPolicy::new(100, 1, 1.0).unwrap();
        let result = execute_step_with_retry(StepId::new("step-flaky".to_string()), 5000, policy).await;
        assert!(result.is_err());
        if let ExecuteNodeError::RetryExhausted { attempts, .. } = result.unwrap_err() {
            assert_eq!(attempts, 100);
        } else {
            panic!("expected RetryExhausted with 100 attempts");
        }
    }

    #[tokio::test]
    async fn retry_error_chain_preserves_cause() {
        let _guard = setup();
        let _dir = tempdir().unwrap();
        let policy = RetryPolicy::new(5, 1, 1.0).unwrap();
        let result = execute_step_with_retry(StepId::new("step-flaky".to_string()), 5000, policy).await;
        assert!(result.is_err());
        if let ExecuteNodeError::RetryExhausted { last_error, .. } = result.unwrap_err() {
            assert!(matches!(*last_error, ExecuteNodeError::TransientError { recoverable: true, .. }));
        } else {
            panic!("expected RetryExhausted");
        }
    }

    #[tokio::test]
    async fn backoff_overflow_does_not_panic() {
        let _guard = setup();
        let _dir = tempdir().unwrap();
        let policy = RetryPolicy::with_max_backoff(20, 1, 1000.0, 100).unwrap();
        for attempt in 1..=20 {
            let delay = policy.calculate_backoff_delay(attempt);
            assert!(delay <= 100, "backoff delay {} at attempt {} exceeds max_backoff_ms", delay, attempt);
        }
    }

    #[tokio::test]
    async fn zero_backoff_with_multiplier_stays_zero() {
        let _guard = setup();
        let _dir = tempdir().unwrap();
        let policy = RetryPolicy::new(10, 0, 1000.0).unwrap();
        for attempt in 1..=10 {
            assert_eq!(policy.calculate_backoff_delay(attempt), 0);
        }
    }

    #[tokio::test]
    async fn flaky_with_insufficient_timeout_reports_timeout() {
        let _guard = setup();
        let _dir = tempdir().unwrap();
        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();
        let result = execute_step_with_retry(StepId::new("step-flaky".to_string()), 100, policy).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn success_step_never_retries() {
        let _guard = setup();
        let _dir = tempdir().unwrap();
        let policy = RetryPolicy::new(5, 10, 2.0).unwrap();
        let result = execute_step_with_retry(StepId::new("step-1".to_string()), 5000, policy).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_success());
        let status = get_execution_status(&StepId::new("step-1".to_string()));
        assert!(matches!(status, ExecutionStatus::Ready));
    }

    #[tokio::test]
    async fn retry_exhaustion_sets_retrievable_error() {
        let _guard = setup();
        let _dir = tempdir().unwrap();
        let policy = RetryPolicy::new(3, 1, 1.0).unwrap();
        let _ = execute_step_with_retry(StepId::new("step-flaky".to_string()), 5000, policy).await;
        assert!(get_last_error(&StepId::new("step-flaky".to_string())).is_some());
    }
}

mod concurrent_interference {
    use super::*;

    #[tokio::test]
    async fn concurrent_different_steps_do_not_interfere() {
        let _guard = setup();
        let _dir = tempdir().unwrap();
        let mut handles = vec![];
        for i in 0..20 {
            let step_name = format!("step-{}", i);
            handles.push(tokio::spawn(async move {
                let result = execute_step(StepId::new(step_name.clone()), 5000).await;
                assert!(result.is_ok(), "step {} should succeed", step_name);
            }));
        }
        for handle in handles {
            handle.await.unwrap();
        }
        assert!(get_state_count() >= 20, "expected at least 20 state entries, got {}", get_state_count());
    }

    #[tokio::test]
    async fn concurrent_same_step_rejected() {
        let _guard = setup();
        let _dir = tempdir().unwrap();
        set_executing_state_for_test("step-1");
        let result = execute_step(StepId::new("step-1".to_string()), 5000).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ExecuteNodeError::InvalidTransition { .. }));
        set_state("step-1", StepState::Ready);
    }

    #[tokio::test]
    async fn concurrent_status_reads_under_contention() {
        let _guard = setup();
        let _dir = tempdir().unwrap();
        set_executing_state_for_test("step-1");
        set_state("step-2", StepState::Completed { output: "done".into() });
        set_state("step-3", StepState::Cancelled { reason: "test".into() });
        let barrier = Arc::new(Barrier::new(16));
        let mut handles = vec![];
        for _ in 0..16 {
            let b = barrier.clone();
            handles.push(tokio::spawn(async move {
                b.wait();
                let _ = get_execution_status(&StepId::new("step-1".to_string()));
                let _ = get_execution_status(&StepId::new("step-2".to_string()));
                let _ = get_execution_status(&StepId::new("step-3".to_string()));
            }));
        }
        for handle in handles {
            handle.await.unwrap();
        }
    }

    #[tokio::test]
    async fn concurrent_error_writes_are_durable() {
        let _guard = setup();
        let _dir = tempdir().unwrap();
        let barrier = Arc::new(Barrier::new(20));
        let mut handles = vec![];
        for i in 0..20 {
            let b = barrier.clone();
            handles.push(tokio::spawn(async move {
                b.wait();
                let key = format!("bh-error-{}", i);
                set_error(&key, ExecuteNodeError::TransientError { reason: format!("error-{}", i), recoverable: true });
            }));
        }
        for handle in handles {
            handle.await.unwrap();
        }
        for i in 0..20 {
            let key = format!("bh-error-{}", i);
            assert!(get_last_error(&StepId::new(key)).is_some(), "error {} should be retrievable", i);
        }
    }

    #[tokio::test]
    async fn cancel_execute_race_does_not_corrupt_state() {
        let _guard = setup();
        let _dir = tempdir().unwrap();
        let step = StepId::new("step-1".to_string());
        let step_clone = step.clone();
        let cancel_handle = tokio::spawn(async move { cancel_execution(step_clone).await });
        let step_clone = step.clone();
        let exec_handle = tokio::spawn(async move { execute_step(step_clone, 5000).await });
        let _ = cancel_handle.await;
        let _ = exec_handle.await;
        let state = get_state("step-1");
        match state {
            StepState::Ready | StepState::Completed { .. } | StepState::Cancelled { .. } => {}
            StepState::Executing { .. } => panic!("step stuck in Executing after concurrent cancel+execute"),
        }
    }

    #[tokio::test]
    async fn rapid_error_set_clear_cycle() {
        let _guard = setup();
        let _dir = tempdir().unwrap();
        let key = "bh-rapid-cycle";
        for _ in 0..1000 {
            set_error(key, ExecuteNodeError::TransientError { reason: "flip".into(), recoverable: true });
            clear_error(key);
        }
        assert!(get_last_error(&StepId::new(key.to_string())).is_none(), "error must be cleared after 1000 cycles");
    }

    #[tokio::test]
    async fn concurrent_state_writes_do_not_lose_entries() {
        let _guard = setup();
        let _dir = tempdir().unwrap();
        let barrier = Arc::new(Barrier::new(30));
        let mut handles = vec![];
        for i in 0..30 {
            let b = barrier.clone();
            handles.push(tokio::spawn(async move {
                b.wait();
                let key = format!("bh-state-{}", i);
                set_state(&key, StepState::Completed { output: format!("result-{}", i) });
            }));
        }
        for handle in handles {
            handle.await.unwrap();
        }
        let mut found = 0;
        for i in 0..30 {
            let key = format!("bh-state-{}", i);
            if let StepState::Completed { output } = get_state(&key) {
                if output == format!("result-{}", i) {
                    found += 1;
                }
            }
        }
        assert_eq!(found, 30, "all 30 state entries must be present and correct");
    }
}

mod subprocess_timeout_bypass {
    use vo_executor::{run_subprocess, SubprocessConfig, SubprocessError};

    #[tokio::test]
    async fn slow_subprocess_is_killed_on_timeout() {
        let config = SubprocessConfig::new(
            "/bin/sleep".to_string(),
            vec!["sleep".to_string(), "60".to_string()],
            100,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SubprocessError::Timeout { .. }));
    }

    #[tokio::test]
    async fn nonexistent_binary_fails_gracefully() {
        let config = SubprocessConfig::new(
            "/tmp/blackhat-nonexistent-binary-12345".to_string(),
            vec![],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SubprocessError::SpawnFailed { .. }));
    }

    #[tokio::test]
    async fn zero_timeout_subprocess_times_out() {
        let config = SubprocessConfig::new(
            "/bin/sleep".to_string(),
            vec!["sleep".to_string(), "1".to_string()],
            0,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SubprocessError::Timeout { elapsed_ms: 0, .. }));
    }

    #[tokio::test]
    async fn large_fd3_payload_no_deadlock() {
        let payload: Vec<u8> = (0..204_800).map(|i| (i % 256) as u8).collect();
        let config = SubprocessConfig::new(
            "/bin/cat".to_string(),
            vec!["cat".to_string()],
            5000,
            payload,
        );
        let result = run_subprocess(config).await;
        match result {
            Ok(output) => { assert!(output.exit_code.is_some()); }
            Err(SubprocessError::Timeout { .. }) => {
                panic!("200KB FD3 payload caused deadlock -- ADR-018 violation");
            }
            Err(e) => {
                assert!(
                    matches!(e, SubprocessError::SpawnFailed { .. } | SubprocessError::Fd4ReadFailed { .. }),
                    "unexpected error: {}", e
                );
            }
        }
    }
}
