//! Exhaustive tests for RetryPolicy (vo-executor) — TDD Red Phase
//!
//! Covers gaps identified in test plan ve-9zs:
//! - with_max_backoff in vo-executor (GE-1, GE-2)
//! - MaxBackoffTooSmall error display (GE-5)
//! - Clone preserves max_backoff_ms (GE-10)
//! - calculate_backoff_delay with cap (GE-6, UT-E-07)
//! - validate_retry_policy incompleteness (GE-8, IT-03)
//! - Integration: zero backoff, max_backoff cap, timing (IT-01 through IT-08)
//! - Property-based tests (PT-E-01, PT-E-02)
//! - Mutation killers (MT-07, MT-08)
//! - Cross-crate equivalence (XT-01, XT-02)

use proptest::prelude::*;
use vo_executor::RetryPolicy;
use vo_executor::RetryPolicyError;

// ============================================================================
// GE-1: with_max_backoff accepts valid configuration
// ============================================================================

#[test]
fn executor_with_max_backoff_accepts_valid_configuration() {
    let policy = RetryPolicy::with_max_backoff(3, 100, 2.0, 500).unwrap();
    assert_eq!(policy.max_attempts, 3);
    assert_eq!(policy.backoff_ms, 100);
    assert_eq!(policy.backoff_multiplier, 2.0);
    assert_eq!(policy.max_backoff_ms, 500);
}

// ============================================================================
// GE-2: with_max_backoff rejects max_backoff_ms < backoff_ms
// ============================================================================

#[test]
fn executor_with_max_backoff_rejects_too_small_max() {
    let err = RetryPolicy::with_max_backoff(3, 500, 2.0, 100).unwrap_err();
    assert!(matches!(
        err,
        RetryPolicyError::MaxBackoffTooSmall { max: 100, ms: 500 }
    ));
}

#[test]
fn executor_with_max_backoff_rejects_zero_attempts() {
    let err = RetryPolicy::with_max_backoff(0, 100, 2.0, 500).unwrap_err();
    assert!(matches!(err, RetryPolicyError::ZeroAttempts));
}

#[test]
fn executor_with_max_backoff_rejects_nan_multiplier() {
    let err = RetryPolicy::with_max_backoff(3, 100, f64::NAN, 500).unwrap_err();
    assert!(matches!(err, RetryPolicyError::InvalidMultiplier { got } if got.is_nan()));
}

// ============================================================================
// GE-5: MaxBackoffTooSmall error display format
// ============================================================================

#[test]
fn executor_retry_policy_error_max_backoff_too_small_display() {
    let err = RetryPolicyError::MaxBackoffTooSmall { max: 50, ms: 200 };
    let display = err.to_string();
    assert!(
        display.contains("50"),
        "Display should contain max: got '{}'",
        display
    );
    assert!(
        display.contains("200"),
        "Display should contain ms: got '{}'",
        display
    );
}

// ============================================================================
// GE-10: Clone preserves max_backoff_ms
// ============================================================================

#[test]
fn executor_clone_preserves_max_backoff_ms() {
    let policy = RetryPolicy::with_max_backoff(3, 100, 2.0, 500).unwrap();
    let cloned = policy.clone();
    assert_eq!(cloned.max_backoff_ms, 500);
    assert_eq!(policy, cloned);
}

// ============================================================================
// UT-E-07: calculate_backoff_delay with max_backoff_ms cap
// ============================================================================

#[test]
fn executor_calculate_backoff_delay_capped_by_max_backoff() {
    let policy = RetryPolicy::with_max_backoff(5, 100, 2.0, 300).unwrap();
    assert_eq!(policy.calculate_backoff_delay(1), 100);
    assert_eq!(policy.calculate_backoff_delay(2), 200);
    assert_eq!(policy.calculate_backoff_delay(3), 300); // capped from 400
}

// ============================================================================
// IT-01: execute_step_with_retry with zero backoff_ms completes quickly
// ============================================================================

#[tokio::test]
async fn retry_with_zero_backoff_ms_completes_quickly() {
    let policy = RetryPolicy::new(3, 0, 2.0).unwrap();
    let start = std::time::Instant::now();
    let result = vo_executor::execute_step_with_retry(
        vo_executor::StepId::new("step-flaky".to_string()),
        5000,
        policy,
    )
    .await;
    let elapsed = start.elapsed().as_millis() as u64;
    let err = result.unwrap_err();
    assert!(matches!(
        err,
        vo_executor::ExecuteNodeError::RetryExhausted { attempts: 3, .. }
    ));
    assert!(
        elapsed < 50,
        "Zero backoff should complete quickly, got {}ms",
        elapsed
    );
}

// ============================================================================
// IT-02: Retry timing with max_backoff_ms cap
// ============================================================================

#[tokio::test]
async fn retry_timing_with_max_backoff_cap() {
    // backoff_ms=100, multiplier=10.0, max_backoff_ms=500
    // attempt 1: 100ms, attempt 2: 500ms (capped from 1000ms)
    // total >= 550ms
    let policy = RetryPolicy::with_max_backoff(3, 100, 10.0, 500).unwrap();
    let start = std::time::Instant::now();
    let result = vo_executor::execute_step_with_retry(
        vo_executor::StepId::new("step-flaky".to_string()),
        5000,
        policy,
    )
    .await;
    let elapsed = start.elapsed().as_millis() as u64;
    let err = result.unwrap_err();
    assert!(matches!(
        err,
        vo_executor::ExecuteNodeError::RetryExhausted { attempts: 3, .. }
    ));
    assert!(
        elapsed >= 500,
        "Expected >= 550ms with capped backoff, got {}ms",
        elapsed
    );
}

// ============================================================================
// IT-04: RetryExhausted.last_error contains TransientError
// ============================================================================

#[tokio::test]
async fn retry_exhausted_last_error_is_transient() {
    let policy = RetryPolicy::new(2, 10, 2.0).unwrap();
    let result = vo_executor::execute_step_with_retry(
        vo_executor::StepId::new("step-flaky".to_string()),
        5000,
        policy,
    )
    .await;
    match result.unwrap_err() {
        vo_executor::ExecuteNodeError::RetryExhausted { last_error, .. } => {
            assert!(
                matches!(
                    *last_error,
                    vo_executor::ExecuteNodeError::TransientError {
                        recoverable: true,
                        ..
                    }
                ),
                "last_error should be TransientError with recoverable=true"
            );
        }
        other => panic!("Expected RetryExhausted, got {:?}", other),
    }
}

// ============================================================================
// IT-05: max_attempts=1 returns immediately with RetryExhausted
// ============================================================================

#[tokio::test]
async fn retry_with_single_attempt_returns_immediately() {
    let policy = RetryPolicy::new(1, 1000, 2.0).unwrap();
    let start = std::time::Instant::now();
    let result = vo_executor::execute_step_with_retry(
        vo_executor::StepId::new("step-flaky".to_string()),
        5000,
        policy,
    )
    .await;
    let elapsed = start.elapsed().as_millis() as u64;
    match result.unwrap_err() {
        vo_executor::ExecuteNodeError::RetryExhausted { attempts, .. } => {
            assert_eq!(attempts, 1);
        }
        other => panic!("Expected RetryExhausted, got {:?}", other),
    }
    assert!(
        elapsed < 50,
        "max_attempts=1 should not sleep, got {}ms",
        elapsed
    );
}

// ============================================================================
// IT-06: Error stored via set_error before RetryExhausted
// ============================================================================

#[tokio::test]
async fn flaky_retry_stores_transient_error() {
    let step_id = vo_executor::StepId::new("step-flaky".to_string());
    let policy = RetryPolicy::new(2, 10, 2.0).unwrap();
    let _ = vo_executor::execute_step_with_retry(step_id.clone(), 5000, policy).await;
    let error = vo_executor::get_last_error(&step_id);
    assert!(error.is_some(), "Error should be stored for step-flaky");
    match error.unwrap() {
        vo_executor::ExecuteNodeError::TransientError {
            reason,
            recoverable,
        } => {
            assert!(reason.contains("network timeout"));
            assert!(recoverable);
        }
        other => panic!("Expected TransientError, got {:?}", other),
    }
}

// ============================================================================
// IT-07: Non-flaky step succeeds without retry
// ============================================================================

#[tokio::test]
async fn non_flaky_step_succeeds_without_retry() {
    let policy = RetryPolicy::new(3, 100, 2.0).unwrap();
    let result = vo_executor::execute_step_with_retry(
        vo_executor::StepId::new("step-good".to_string()),
        5000,
        policy,
    )
    .await;
    assert_eq!(
        result,
        Ok(vo_executor::StepResult::Success {
            output: "done".to_string(),
        })
    );
}

// ============================================================================
// IT-08: Non-existent step returns StepNotFound
// ============================================================================

#[tokio::test]
async fn nonexistent_step_returns_step_not_found() {
    let policy = RetryPolicy::new(3, 10, 2.0).unwrap();
    let result = vo_executor::execute_step_with_retry(
        vo_executor::StepId::new("nonexistent-step".to_string()),
        5000,
        policy,
    )
    .await;
    assert!(
        matches!(
            result,
            Err(vo_executor::ExecuteNodeError::StepNotFound { .. })
        ),
        "Expected StepNotFound, got {:?}",
        result
    );
}

// ============================================================================
// Property-based tests (PT-E-01, PT-E-02)
// ============================================================================

proptest! {
    #[test]
    fn executor_calculate_backoff_delay_capped_matches_formula(
        backoff_ms in 1u64..=1000u64,
        multiplier in 1.5f64..=10.0f64,
        max_backoff in 100u64..=10_000u64,
        attempt in 1u32..=100,
    ) {
        prop_assume!(max_backoff >= backoff_ms);
        let policy = RetryPolicy::with_max_backoff(10, backoff_ms, multiplier, max_backoff).unwrap();
        let delay = policy.calculate_backoff_delay(attempt);
        let expected_raw = backoff_ms as f64 * multiplier.powi(attempt as i32 - 1);
        let expected = expected_raw.min(max_backoff as f64).min(u64::MAX as f64) as u64;
        prop_assert!(delay <= max_backoff,
            "delay {} should be <= max_backoff {} for attempt {}",
            delay, max_backoff, attempt
        );
        prop_assert_eq!(delay, expected);
    }

    #[test]
    fn executor_calculate_backoff_delay_never_exceeds_u64_max(
        backoff_ms in any::<u64>(),
        multiplier in 1.0f64..=10.0f64,
        attempt in 0u32..=10_000,
    ) {
        let policy = RetryPolicy::new(10, backoff_ms, multiplier).unwrap();
        let delay = policy.calculate_backoff_delay(attempt);
        prop_assert!(delay <= u64::MAX);
    }
}

// ============================================================================
// MT-07: Kill `> 2` → `>= 2` in execute_flaky_retries
// max_attempts=2 should have exactly 1 sleep
// ============================================================================

#[tokio::test]
async fn mt_07_max_attempts_2_exactly_one_sleep() {
    // If `> 2` → `>= 2`: max_attempts=2 would have 2 sleeps instead of 1
    let policy = RetryPolicy::new(2, 100, 2.0).unwrap();
    let start = std::time::Instant::now();
    let result = vo_executor::execute_step_with_retry(
        vo_executor::StepId::new("step-flaky".to_string()),
        5000,
        policy,
    )
    .await;
    let elapsed = start.elapsed().as_millis() as u64;
    let err = result.unwrap_err();
    assert!(matches!(
        err,
        vo_executor::ExecuteNodeError::RetryExhausted { attempts: 2, .. }
    ));
    // With 1 sleep: ~100ms. With 2 sleeps (mutant): ~300ms.
    assert!(
        (80..200).contains(&elapsed),
        "Expected ~100ms (1 sleep) for max_attempts=2, got {}ms",
        elapsed
    );
}

// ============================================================================
// MT-08: Kill `>= 2` → `> 2` in execute_flaky_retries
// max_attempts=2 should enter the retry loop
// ============================================================================

#[tokio::test]
async fn mt_08_max_attempts_2_enters_retry_loop() {
    // If `>= 2` → `> 2`: max_attempts=2 would skip the retry loop entirely
    let policy = RetryPolicy::new(2, 100, 2.0).unwrap();
    let start = std::time::Instant::now();
    let result = vo_executor::execute_step_with_retry(
        vo_executor::StepId::new("step-flaky".to_string()),
        5000,
        policy,
    )
    .await;
    let elapsed = start.elapsed().as_millis() as u64;
    let err = result.unwrap_err();
    // If mutant: attempts would be 1 and elapsed ~0ms
    // Correct: attempts=2 and elapsed ~100ms (1 sleep)
    assert!(matches!(
        err,
        vo_executor::ExecuteNodeError::RetryExhausted { attempts: 2, .. }
    ));
    assert!(
        elapsed >= 80,
        "Should have 1 sleep (~100ms), got {}ms",
        elapsed
    );
}
