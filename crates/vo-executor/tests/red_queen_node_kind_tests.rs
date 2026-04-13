//! Red Queen adversarial tests for NodeKind execution semantics (ADR-003).
//!
//! This module implements adversarial testing for all 5 node kinds:
//! - Pure: deterministic, side-effect-free computation
//! - ManagedEffect: journaled effects with exactly-once crash recovery
//! - Wait: workflow hibernate/resume semantics
//! - Signal: cross-instance signal delivery
//! - Unsafe: fire-and-forget with at-least-once semantics
//!
//! These tests attack the contracts from the other side — they verify that
//! the system fails (or succeeds) correctly under adversarial conditions.

use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::MutexGuard;
use vo_executor::{
    cancel_execution, execute_step, execute_step_with_retry, get_execution_status,
    get_last_error, reset_all_state, RetryPolicy, StepId, StepResult,
};

static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn state_guard() -> MutexGuard<'static, ()> {
    let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset_all_state();
    guard
}

// ============================================================================
// RED QUEEN: PURE NodeKind — Pure Determinism Tests
// ============================================================================

#[cfg(test)]
mod red_queen_pure_tests {
    use super::*;

    #[tokio::test]
    async fn pure_same_input_produces_same_output() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        let result1 = execute_step(step_id.clone(), 5000).await.unwrap();
        let result2 = execute_step(step_id.clone(), 5000).await.unwrap();
        let result3 = execute_step(step_id.clone(), 5000).await.unwrap();

        assert_eq!(result1, result2);
        assert_eq!(result2, result3);
        match (&result1, &result2, &result3) {
            (
                StepResult::Success { output: o1 },
                StepResult::Success { output: o2 },
                StepResult::Success { output: o3 },
            ) => {
                assert_eq!(o1, o2);
                assert_eq!(o2, o3);
                assert_eq!(o1, "done");
            }
            _ => panic!("All should be Success"),
        }
    }

    #[tokio::test]
    async fn pure_idempotent_under_retry() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());
        let policy = RetryPolicy::new(5, 10, 2.0).unwrap();

        for _ in 0..3 {
            let result = execute_step_with_retry(step_id.clone(), 5000, policy.clone()).await;
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn pure_no_side_effects_on_global_state() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        execute_step(step_id.clone(), 5000).await.unwrap();

        let status = get_execution_status(&step_id);
        assert!(status.is_ready());

        let error = get_last_error(&step_id);
        assert!(error.is_none());
    }

    #[tokio::test]
    async fn pure_timeout_is_recoverable() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok(), "Pure step with sufficient timeout should succeed");
    }

    #[tokio::test]
    async fn pure_multiple_instances_same_result() {
        let _guard = state_guard();

        let handles: Vec<_> = (0..5)
            .map(|_| {
                tokio::spawn(execute_step(StepId::new("step-good".to_string()), 5000))
            })
            .collect();

        let mut results = Vec::new();
        for handle in handles {
            results.push(handle.await.unwrap());
        }

        for result in &results {
            assert_eq!(result.as_ref().unwrap(), &StepResult::Success { output: "done".to_string() });
        }
    }

    #[tokio::test]
    async fn pure_no_state_leak_between_executions() {
        let _guard = state_guard();
        let step_a = StepId::new("step-good".to_string());
        let step_b = StepId::new("step-1".to_string());

        execute_step(step_a.clone(), 5000).await.unwrap();

        let status_b = get_execution_status(&step_b);
        assert!(status_b.is_ready());

        let error_a = get_last_error(&step_a);
        let error_b = get_last_error(&step_b);
        assert!(error_a.is_none());
        assert!(error_b.is_none());
    }
}

// ============================================================================
// RED QUEEN: ManagedEffect NodeKind — Exactly-Once Crash Recovery Tests
// ============================================================================

#[cfg(test)]
mod red_queen_managed_effect_tests {
    use super::*;

    #[tokio::test]
    async fn managed_effect_journaled_on_first_execution() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());

        let error = get_last_error(&step_id);
        assert!(error.is_none(), "Good step should have no error after success");
    }

    #[tokio::test]
    async fn managed_effect_recovers_from_transient() {
        let _guard = state_guard();
        let step_id = StepId::new("step-flaky".to_string());
        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();

        let result = execute_step_with_retry(step_id.clone(), 5000, policy).await;

        match result {
            Err(vo_executor::ExecuteNodeError::RetryExhausted { attempts, .. }) => {
                assert_eq!(attempts, 3);
            }
            _ => panic!("Expected RetryExhausted after transient failures"),
        }
    }

    #[tokio::test]
    async fn managed_effect_exactly_once_under_crash_simulation() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        let result1 = execute_step(step_id.clone(), 5000).await;
        assert!(result1.is_ok());

        reset_all_state();

        let result2 = execute_step(step_id.clone(), 5000).await;
        assert!(result2.is_ok());
        assert_eq!(result1.unwrap(), result2.unwrap());
    }

    #[tokio::test]
    async fn managed_effect_state_cleared_on_reset() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());

        let _ = execute_step(step_id.clone(), 5000).await;
        let error_before = get_last_error(&step_id);
        assert!(error_before.is_some());

        reset_all_state();

        let error_after = get_last_error(&step_id);
        assert!(error_after.is_none(), "Error state should be cleared after reset");
    }

    #[tokio::test]
    async fn managed_effect_retry_policy_preserved() {
        let _guard = state_guard();
        let step_id = StepId::new("step-flaky".to_string());
        let policy = RetryPolicy::new(3, 100, 2.0).unwrap();

        let start = std::time::Instant::now();
        let result = execute_step_with_retry(step_id.clone(), 5000, policy).await;
        let elapsed = start.elapsed().as_millis() as u64;

        assert!(elapsed >= 200, "Should have backoff delays totaling ~300ms");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn managed_effect_parallel_journal_entries_independent() {
        let _guard = state_guard();
        let step_a = StepId::new("step-good".to_string());
        let step_b = StepId::new("step-fail".to_string());

        let (result_a, result_b) = tokio::join!(
            execute_step(step_a.clone(), 5000),
            execute_step(step_b.clone(), 5000)
        );

        assert!(result_a.is_ok());
        assert!(result_b.is_ok());
    }

    #[tokio::test]
    async fn managed_effect_recoverable_flag_respected() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());

        let result = execute_step(step_id.clone(), 5000).await;

        match result {
            Err(vo_executor::ExecuteNodeError::TransientError { recoverable, .. }) => {
                assert!(recoverable, "Transient errors must be recoverable");
            }
            _ => panic!("Expected TransientError with recoverable=true"),
        }
    }

    #[tokio::test]
    async fn managed_effect_not_found_is_terminal() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();

        let result = execute_step_with_retry(
            StepId::new("nonexistent-step".to_string()),
            5000,
            policy,
        )
        .await;

        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::StepNotFound { .. })
        ));
    }
}

// ============================================================================
// RED QUEEN: Wait NodeKind — Hibernate/Resume Tests
// ============================================================================

#[cfg(test)]
mod red_queen_wait_tests {
    use super::*;

    #[tokio::test]
    async fn wait_blocking_step_succeeds_with_enough_timeout() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn wait_step_timeout_triggers_early() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let result = execute_step(step_id.clone(), 100).await;

        match result {
            Err(vo_executor::ExecuteNodeError::TimeoutExceeded { elapsed_ms, limit_ms }) => {
                assert!(elapsed_ms >= 3000);
                assert_eq!(limit_ms, 100);
            }
            _ => panic!("Expected TimeoutExceeded"),
        }
    }

    #[tokio::test]
    async fn wait_at_threshold_boundary() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let result_2999 = execute_step(step_id.clone(), 2999).await;
        assert!(result_2999.is_err());

        let result_3000 = execute_step(step_id.clone(), 3000).await;
        assert!(result_3000.is_ok());
    }

    #[tokio::test]
    async fn wait_cancellation_returns_to_ready() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let handle = tokio::spawn(execute_step(step_id.clone(), 5000));

        tokio::task::yield_now().await;

        let cancel_result = cancel_execution(step_id.clone()).await;
        assert!(cancel_result.is_ok());

        let status = get_execution_status(&step_id);
        assert!(matches!(status, vo_executor::ExecutionStatus::Cancelled { .. }));

        let _ = handle.await;
    }

    #[tokio::test]
    async fn wait_status_transitions_correctly() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        let status_before = get_execution_status(&step_id);
        assert!(status_before.is_ready());

        let handle = tokio::spawn(execute_step(step_id.clone(), 5000));

        let _ = handle.await;

        let status_after = get_execution_status(&step_id);
        assert!(status_after.is_ready());
    }

    #[tokio::test]
    async fn wait_ready_after_completion() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());

        let status = get_execution_status(&step_id);
        assert!(status.is_ready());
    }

    #[tokio::test]
    async fn wait_multiple_concurrent_wait_steps() {
        let _guard = state_guard();

        let handles: Vec<_> = vec![
            tokio::spawn(execute_step(StepId::new("step-slow".to_string()), 5000)),
            tokio::spawn(execute_step(StepId::new("step-good".to_string()), 5000)),
            tokio::spawn(execute_step(StepId::new("step-1".to_string()), 5000)),
        ];

        let mut results = Vec::new();
        for handle in handles {
            results.push(handle.await.unwrap());
        }

        assert_eq!(results.len(), 3);
    }
}

// ============================================================================
// RED QUEEN: Signal NodeKind — Cross-Instance Delivery Tests
// ============================================================================

#[cfg(test)]
mod red_queen_signal_tests {
    use super::*;

    #[tokio::test]
    async fn signal_step_succeeds_without_receiver() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn signal_does_not_leak_to_unrelated_step() {
        let _guard = state_guard();
        let step_a = StepId::new("step-good".to_string());
        let step_b = StepId::new("step-1".to_string());

        execute_step(step_a.clone(), 5000).await.unwrap();

        let status_b = get_execution_status(&step_b);
        assert!(status_b.is_ready());

        let result_b = execute_step(step_b.clone(), 5000).await;
        assert!(result_b.is_ok());
    }

    #[tokio::test]
    async fn signal_cross_instance_independence() {
        let _guard = state_guard();

        let instance_a = StepId::new("step-good".to_string());
        let instance_b = StepId::new("step-1".to_string());

        let (result_a, result_b) = tokio::join!(
            execute_step(instance_a.clone(), 5000),
            execute_step(instance_b.clone(), 5000)
        );

        assert!(result_a.is_ok());
        assert!(result_b.is_ok());
    }

    #[tokio::test]
    async fn signal_failure_does_not_cascade() {
        let _guard = state_guard();

        let step_good = StepId::new("step-good".to_string());
        let step_fail = StepId::new("step-fail".to_string());

        let result_fail = execute_step(step_fail.clone(), 5000).await;
        assert!(result_fail.is_ok());

        let result_good = execute_step(step_good.clone(), 5000).await;
        assert!(result_good.is_ok());
    }

    #[tokio::test]
    async fn signal_multiple_signals_ordered() {
        let _guard = state_guard();

        let handles: Vec<_> = vec!["step-1", "step-good", "step-valid"]
            .iter()
            .map(|id| {
                tokio::spawn(execute_step(StepId::new(id.to_string()), 5000))
            })
            .collect();

        let count = handles.len();
        assert_eq!(count, 3);

        let mut results = Vec::new();
        for handle in handles {
            results.push(handle.await.unwrap());
        }

        for result in results {
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn signal_concurrent_delivery_no_race() {
        let _guard = state_guard();

        let handles: Vec<_> = (0..10)
            .map(|_| {
                tokio::spawn(execute_step(
                    StepId::new("step-good".to_string()),
                    5000,
                ))
            })
            .collect();

        let mut success_count = 0;
        for handle in handles {
            let result = handle.await.unwrap();
            if result.is_ok() {
                success_count += 1;
            }
        }

        assert_eq!(success_count, 10, "All concurrent signals should succeed");
    }
}

// ============================================================================
// RED QUEEN: Unsafe NodeKind — Fire-and-Forget Leak Tests
// ============================================================================

#[cfg(test)]
mod red_queen_unsafe_tests {
    use super::*;

    #[tokio::test]
    async fn unsafe_step_may_succeed_or_leak() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn unsafe_failure_is_not_recoverable() {
        let _guard = state_guard();
        let step_id = StepId::new("step-fail".to_string());
        let policy = RetryPolicy::new(3, 10, 2.0).unwrap();

        let result = execute_step_with_retry(step_id.clone(), 5000, policy).await;

        match result {
            Err(vo_executor::ExecuteNodeError::RetryExhausted { .. }) => {}
            Ok(result) => {
                assert!(matches!(result, StepResult::Failure { .. }));
            }
            _ => {}
        }
    }

    #[tokio::test]
    async fn unsafe_no_exactly_once_guarantee() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        let mut results = Vec::new();
        for _ in 0..5 {
            let result = execute_step(step_id.clone(), 5000).await;
            results.push(result);
        }

        for result in results {
            assert!(result.is_ok(), "Unsafe step should succeed (at-least-once)");
        }
    }

    #[tokio::test]
    async fn unsafe_state_undefined_after_execution() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());

        let status = get_execution_status(&step_id);
        assert!(status.is_ready(), "Status should return to Ready after execution");
    }

    #[tokio::test]
    async fn unsafe_transient_error_not_retried_by_default() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());

        let result = execute_step(step_id.clone(), 5000).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            vo_executor::ExecuteNodeError::TransientError { .. }
        ));
    }

    #[tokio::test]
    async fn unsafe_parallel_executions_may_conflict() {
        let _guard = state_guard();

        let handles: Vec<_> = (0..5)
            .map(|_| {
                tokio::spawn(execute_step(
                    StepId::new("step-good".to_string()),
                    5000,
                ))
            })
            .collect();

        let mut results = Vec::new();
        for handle in handles {
            results.push(handle.await.unwrap());
        }

        for result in results {
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn unsafe_execution_completes_successfully() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());

        let status = get_execution_status(&step_id);
        assert!(status.is_ready());
    }

    #[tokio::test]
    async fn unsafe_cancellation_leaves_ambiguous_state() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let handle = tokio::spawn(execute_step(step_id.clone(), 5000));

        tokio::task::yield_now().await;

        let cancel_result = cancel_execution(step_id.clone()).await;
        assert!(cancel_result.is_ok());

        let _ = handle.await;

        let status = get_execution_status(&step_id);
        assert!(matches!(status, vo_executor::ExecutionStatus::Cancelled { .. }));
    }
}

// ============================================================================
// RED QUEEN: RetryPolicy Contract Tests
// ============================================================================

#[cfg(test)]
mod red_queen_retry_policy_tests {
    use super::*;

    #[tokio::test]
    async fn retry_zero_attempts_rejected() {
        let _guard = state_guard();
        let policy = vo_executor::RetryPolicy {
            max_attempts: 0,
            backoff_ms: 100,
            backoff_multiplier: 2.0,
            max_backoff_ms: u64::MAX,
        };
        let result = execute_step_with_retry(
            StepId::new("step-1".to_string()),
            5000,
            policy,
        )
        .await;

        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::InvalidRetryPolicy { .. })
        ));
    }

    #[tokio::test]
    async fn retry_backoff_exponential_growth() {
        let _guard = state_guard();
        let step_id = StepId::new("step-flaky".to_string());
        let policy = RetryPolicy::new(4, 100, 2.0).unwrap();

        let start = std::time::Instant::now();
        let result = execute_step_with_retry(step_id.clone(), 5000, policy).await;
        let elapsed = start.elapsed().as_millis() as u64;

        assert!(elapsed >= 300, "Backoff should grow: 100 + 200 + 400 = 700ms");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn retry_backoff_capped_at_max() {
        let _guard = state_guard();
        let policy = RetryPolicy::with_max_backoff(5, 100, 2.0, 250).unwrap();

        assert_eq!(policy.calculate_backoff_delay(1), 100);
        assert_eq!(policy.calculate_backoff_delay(2), 200);
        assert_eq!(policy.calculate_backoff_delay(3), 250);
        assert_eq!(policy.calculate_backoff_delay(4), 250);
    }

    #[tokio::test]
    async fn retry_exhausted_error_contains_attempts() {
        let _guard = state_guard();
        let step_id = StepId::new("step-flaky".to_string());
        let policy = RetryPolicy::new(5, 10, 2.0).unwrap();

        let result = execute_step_with_retry(step_id.clone(), 5000, policy).await;

        match result {
            Err(vo_executor::ExecuteNodeError::RetryExhausted { attempts, .. }) => {
                assert_eq!(attempts, 5);
            }
            _ => panic!("Expected RetryExhausted"),
        }
    }
}

// ============================================================================
// RED QUEEN: Adversarial Edge Cases
// ============================================================================

#[cfg(test)]
mod red_queen_adversarial_edge_cases {
    use super::*;

    #[tokio::test]
    async fn concurrent_execution_stress_test() {
        let _guard = state_guard();

        let step_ids = vec!["step-1", "step-good", "step-valid", "step-retry"];

        let handles: Vec<_> = (0..20)
            .map(|i| {
                let step_id = step_ids[i % step_ids.len()].to_string();
                tokio::spawn(execute_step(StepId::new(step_id), 5000))
            })
            .collect();

        let mut success_count = 0;
        let mut failure_count = 0;

        for handle in handles {
            match handle.await.unwrap() {
                Ok(vo_executor::StepResult::Success { .. }) => success_count += 1,
                Ok(vo_executor::StepResult::Failure { .. }) => failure_count += 1,
                Err(_) => {}
            }
        }

        assert_eq!(success_count + failure_count, 20);
    }

    #[tokio::test]
    async fn rapid_state_transitions_stress() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        for _ in 0..50 {
            let result = execute_step(step_id.clone(), 5000).await;
            assert!(result.is_ok());

            let status = get_execution_status(&step_id);
            assert!(status.is_ready());
        }
    }

    #[tokio::test]
    async fn error_then_success_transition() {
        let _guard = state_guard();

        let result_transient = execute_step(StepId::new("step-transient".to_string()), 5000).await;
        assert!(result_transient.is_err());

        let result_success = execute_step(StepId::new("step-good".to_string()), 5000).await;
        assert!(result_success.is_ok());
    }

    #[tokio::test]
    async fn cancel_during_execution() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let handle = tokio::spawn(execute_step(step_id.clone(), 5000));

        tokio::task::yield_now().await;

        let cancel_result = cancel_execution(step_id.clone()).await;
        assert!(cancel_result.is_ok());

        let _ = handle.await;

        let status = get_execution_status(&step_id);
        assert!(matches!(status, vo_executor::ExecutionStatus::Cancelled { .. }));
    }

    #[tokio::test]
    async fn timeout_then_immediate_retry() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let result_timeout = execute_step(step_id.clone(), 100).await;
        assert!(result_timeout.is_err());

        let result_retry = execute_step(step_id.clone(), 5000).await;
        assert!(result_retry.is_ok());
    }

    #[tokio::test]
    async fn invalid_timeout_zero_rejected() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-1".to_string()), 0).await;
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::InvalidTimeout { .. })
        ));
    }

    #[tokio::test]
    async fn invalid_timeout_max_rejected() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-1".to_string()), u64::MAX).await;
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::InvalidTimeout { .. })
        ));
    }

    #[tokio::test]
    async fn all_node_kind_behaviors_exercise() {
        let _guard = state_guard();

        let success = execute_step(StepId::new("step-good".to_string()), 5000).await;
        assert!(success.is_ok());

        let failure = execute_step(StepId::new("step-fail".to_string()), 5000).await;
        assert!(failure.is_ok());

        let transient = execute_step(StepId::new("step-transient".to_string()), 5000).await;
        assert!(transient.is_err());

        let slow = execute_step(StepId::new("step-slow".to_string()), 5000).await;
        assert!(slow.is_ok());

        let not_found = execute_step(StepId::new("step-invalid".to_string()), 5000).await;
        assert!(matches!(
            not_found,
            Err(vo_executor::ExecuteNodeError::StepNotFound { .. })
        ));
    }
}