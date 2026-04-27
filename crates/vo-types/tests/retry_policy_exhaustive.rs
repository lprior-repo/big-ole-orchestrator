//! Exhaustive tests for RetryPolicy (vo-types) — TDD Red Phase
//!
//! Covers gaps identified in test plan ve-9zs:
//! - with_max_backoff validation priority (GT-1, GT-2)
//! - Negative infinity multiplier (GT-3)
//! - Multiplier 1.0 monotonicity (GT-4)
//! - Large attempt overflow safety (GT-5)
//! - Exact max_backoff_ms cap (GT-6)
//! - Serde missing/explicit max_backoff_ms (GT-7, GT-8, GT-15)
//! - Copy/PartialEq traits (GT-9, GT-16, GT-17, GT-18, GT-19)
//! - Error display/equality (GT-12, GT-13, GT-14)
//! - Property-based tests (GT-10, GT-11)
//! - Adversarial (RQ-02, RQ-03, RQ-04)
//! - Mutation killers (MT-01 through MT-06)

use proptest::prelude::*;
use vo_types::RetryPolicy;
use vo_types::RetryPolicyError;

// ============================================================================
// GT-1: with_max_backoff validation priority — ZeroAttempts before MaxBackoffTooSmall
// ============================================================================

#[test]
fn with_max_backoff_rejects_zero_attempts_before_max_backoff_too_small() {
    let err = RetryPolicy::with_max_backoff(0, 100, 2.0, 50).unwrap_err();
    assert!(
        matches!(err, RetryPolicyError::ZeroAttempts),
        "Expected ZeroAttempts, got {:?}",
        err
    );
}

// ============================================================================
// GT-2: with_max_backoff rejects InvalidMultiplier before MaxBackoffTooSmall
// ============================================================================

#[test]
fn with_max_backoff_rejects_invalid_multiplier_before_max_backoff_too_small() {
    let err = RetryPolicy::with_max_backoff(3, 100, 0.5, 50).unwrap_err();
    assert!(
        matches!(err, RetryPolicyError::InvalidMultiplier { got } if got == 0.5),
        "Expected InvalidMultiplier(0.5), got {:?}",
        err
    );
}

// ============================================================================
// GT-3: Negative infinity multiplier rejected
// ============================================================================

#[test]
fn with_max_backoff_rejects_negative_infinity_multiplier() {
    let err = RetryPolicy::with_max_backoff(3, 100, f64::NEG_INFINITY, u64::MAX).unwrap_err();
    assert!(
        matches!(err, RetryPolicyError::InvalidMultiplier { got } if got == f64::NEG_INFINITY),
        "Expected InvalidMultiplier(NEG_INFINITY), got {:?}",
        err
    );
}

// ============================================================================
// GT-4: Multiplier 1.0 produces constant delay across many attempts
// ============================================================================

#[test]
fn calculate_backoff_delay_constant_with_multiplier_one_across_many_attempts() {
    let policy = RetryPolicy::new(5, 500, 1.0).unwrap();
    for attempt in 1..=100u32 {
        assert_eq!(
            policy.calculate_backoff_delay(attempt),
            500,
            "Multiplier 1.0 should produce constant delay for attempt {}",
            attempt
        );
    }
}

// ============================================================================
// GT-5: Large attempt number does not panic or wrap
// ============================================================================

#[test]
fn calculate_backoff_delay_large_attempt_does_not_panic() {
    let policy = RetryPolicy::new(255, 100, 2.0).unwrap();
    let delay = policy.calculate_backoff_delay(1000);
    assert!(delay <= u64::MAX, "Delay should not exceed u64::MAX");
}

// ============================================================================
// GT-6: Backoff delay equals exactly max_backoff_ms at cap boundary
// ============================================================================

#[test]
fn calculate_backoff_delay_equals_max_backoff_at_cap_boundary() {
    let policy = RetryPolicy::with_max_backoff(10, 100, 2.0, 500).unwrap();
    assert_eq!(policy.calculate_backoff_delay(3), 400);
    assert_eq!(policy.calculate_backoff_delay(4), 500);
    assert_eq!(policy.calculate_backoff_delay(5), 500);
}

// ============================================================================
// GT-7: Serde deserialization with missing max_backoff_ms defaults to u64::MAX
// ============================================================================

#[test]
fn serde_deserialization_missing_max_backoff_defaults_to_u64_max() {
    let json = r#"{"max_attempts":3,"backoff_ms":100,"backoff_multiplier":2.0}"#;
    let policy: RetryPolicy = serde_json::from_str(json).unwrap();
    assert_eq!(policy.max_backoff_ms, u64::MAX);
}

// ============================================================================
// GT-8/UT-T-15: Serde with explicit max_backoff_ms preserves value
// ============================================================================

#[test]
fn serde_deserialization_explicit_max_backoff_preserves_value() {
    let json = r#"{"max_attempts":3,"backoff_ms":100,"backoff_multiplier":2.0,"max_backoff_ms":5000}"#;
    let policy: RetryPolicy = serde_json::from_str(json).unwrap();
    assert_eq!(policy.max_backoff_ms, 5000);
}

// ============================================================================
// UT-T-15: Serde round-trip with custom max_backoff_ms
// ============================================================================

#[test]
fn serde_round_trip_with_custom_max_backoff() {
    let original = RetryPolicy::with_max_backoff(3, 100, 2.0, 10000).unwrap();
    let json = serde_json::to_string(&original).unwrap();
    let round_tripped: RetryPolicy = serde_json::from_str(&json).unwrap();
    assert_eq!(original.max_attempts, round_tripped.max_attempts);
    assert_eq!(original.backoff_ms, round_tripped.backoff_ms);
    assert_eq!(original.backoff_multiplier, round_tripped.backoff_multiplier);
    assert_eq!(original.max_backoff_ms, round_tripped.max_backoff_ms);
}

// ============================================================================
// UT-T-13: Serialization produces correct JSON structure
// ============================================================================

#[test]
fn serde_serialization_produces_correct_json_structure() {
    let policy = RetryPolicy::new(3, 100, 2.0).unwrap();
    let json = serde_json::to_string(&policy).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["max_attempts"], 3);
    assert_eq!(parsed["backoff_ms"], 100);
    assert!((parsed["backoff_multiplier"].as_f64().unwrap() - 2.0).abs() < f64::EPSILON);
    assert_eq!(parsed["max_backoff_ms"], u64::MAX);
}

// ============================================================================
// UT-T-05: MaxBackoffTooSmall display format
// ============================================================================

#[test]
fn retry_policy_error_max_backoff_too_small_display_format() {
    let err = RetryPolicyError::MaxBackoffTooSmall { max: 50, ms: 200 };
    let display = err.to_string();
    assert!(display.contains("50"), "Display should contain max value 50: got '{}'", display);
    assert!(display.contains("200"), "Display should contain ms value 200: got '{}'", display);
}

// ============================================================================
// UT-T-06: RetryPolicyError PartialEq across variants
// ============================================================================

#[test]
fn retry_policy_error_partial_eq_same_variants_equal() {
    assert_eq!(RetryPolicyError::ZeroAttempts, RetryPolicyError::ZeroAttempts);
    assert_eq!(
        RetryPolicyError::InvalidMultiplier { got: 0.5 },
        RetryPolicyError::InvalidMultiplier { got: 0.5 }
    );
    assert_eq!(
        RetryPolicyError::MaxBackoffTooSmall { max: 1, ms: 2 },
        RetryPolicyError::MaxBackoffTooSmall { max: 1, ms: 2 }
    );
}

#[test]
fn retry_policy_error_partial_eq_different_variants_not_equal() {
    assert_ne!(RetryPolicyError::ZeroAttempts, RetryPolicyError::InvalidMultiplier { got: 0.5 });
    assert_ne!(
        RetryPolicyError::InvalidMultiplier { got: 0.5 },
        RetryPolicyError::MaxBackoffTooSmall { max: 1, ms: 2 }
    );
}

// ============================================================================
// UT-T-07: Exponential growth with multiplier 1.5
// ============================================================================

#[test]
fn calculate_backoff_delay_fractional_multiplier_growth() {
    let policy = RetryPolicy::new(5, 100, 1.5).unwrap();
    assert_eq!(policy.calculate_backoff_delay(1), 100);
    assert_eq!(policy.calculate_backoff_delay(2), 150);
    assert_eq!(policy.calculate_backoff_delay(3), 225);
    assert_eq!(policy.calculate_backoff_delay(4), 337);
}

// ============================================================================
// UT-T-10: backoff_ms=1 with multiplier=2.0
// ============================================================================

#[test]
fn calculate_backoff_delay_minimum_nonzero_backoff_ms() {
    let policy = RetryPolicy::new(10, 1, 2.0).unwrap();
    assert_eq!(policy.calculate_backoff_delay(1), 1);
    assert_eq!(policy.calculate_backoff_delay(2), 2);
    assert_eq!(policy.calculate_backoff_delay(10), 512);
}

// ============================================================================
// UT-T-12: calculate_backoff_delay with max_backoff_ms=0 and backoff_ms=0
// ============================================================================

#[test]
fn calculate_backoff_delay_both_zero_backoff_and_max() {
    let policy = RetryPolicy {
        max_attempts: 3,
        backoff_ms: 0,
        backoff_multiplier: 2.0,
        max_backoff_ms: 0,
    };
    for attempt in 0..=10u32 {
        assert_eq!(policy.calculate_backoff_delay(attempt), 0);
    }
}

// ============================================================================
// GT-17: Equal policies compare equal
// ============================================================================

#[test]
fn equal_policies_compare_equal() {
    let p1 = RetryPolicy::new(3, 100, 2.0).unwrap();
    let p2 = RetryPolicy::new(3, 100, 2.0).unwrap();
    assert_eq!(p1, p2);
}

// ============================================================================
// GT-18: Different max_backoff_ms makes policies unequal
// ============================================================================

#[test]
fn different_max_backoff_makes_policies_unequal() {
    let p1 = RetryPolicy::new(3, 100, 2.0).unwrap();
    let p2 = RetryPolicy::with_max_backoff(3, 100, 2.0, 500).unwrap();
    assert_ne!(p1, p2);
}

// ============================================================================
// GT-19: Copy trait allows implicit copying
// ============================================================================

#[test]
fn copy_trait_allows_implicit_copying() {
    let p1 = RetryPolicy::new(3, 100, 2.0).unwrap();
    let p2 = p1;
    assert_eq!(p1, p2);
}

// ============================================================================
// Property-based tests (GT-10, GT-11)
// ============================================================================

proptest! {
    #[test]
    fn calculate_backoff_delay_monotonically_non_decreasing(
        backoff_ms in 1u64..=10_000u64,
        multiplier in 1.0f64..=5.0f64,
        attempt_n in 1u32..=50,
        attempt_m in 1u32..=50,
    ) {
        prop_assume!(attempt_n < attempt_m);
        let policy = RetryPolicy::new(10, backoff_ms, multiplier).unwrap();
        let delay_n = policy.calculate_backoff_delay(attempt_n);
        let delay_m = policy.calculate_backoff_delay(attempt_m);
        prop_assert!(delay_n <= delay_m,
            "delay({}) = {} should be <= delay({}) = {}",
            attempt_n, delay_n, attempt_m, delay_m
        );
    }

    #[test]
    fn calculate_backoff_delay_never_exceeds_max_backoff(
        backoff_ms in 1u64..=1000u64,
        multiplier in 1.5f64..=10.0f64,
        max_backoff in 100u64..=10_000u64,
        attempt in 1u32..=100,
    ) {
        prop_assume!(max_backoff >= backoff_ms, "max_backoff must be >= backoff_ms for with_max_backoff");
        let policy = RetryPolicy::with_max_backoff(10, backoff_ms, multiplier, max_backoff).unwrap();
        let delay = policy.calculate_backoff_delay(attempt);
        prop_assert!(delay <= max_backoff,
            "delay {} should be <= max_backoff {} for attempt {}",
            delay, max_backoff, attempt
        );
    }

    #[test]
    fn calculate_backoff_delay_always_zero_for_attempt_zero(
        max_attempts in 1u8..=10,
        backoff_ms in any::<u64>(),
        multiplier in 1.0f64..=5.0f64,
    ) {
        let policy = RetryPolicy::new(max_attempts, backoff_ms, multiplier).unwrap();
        prop_assert_eq!(policy.calculate_backoff_delay(0), 0u64);
    }

    #[test]
    fn calculate_backoff_delay_zero_when_backoff_ms_zero(
        max_attempts in 1u8..=10,
        multiplier in 1.0f64..=5.0f64,
        attempt in 0u32..=100,
    ) {
        let policy = RetryPolicy::new(max_attempts, 0, multiplier).unwrap();
        prop_assert_eq!(policy.calculate_backoff_delay(attempt), 0u64);
    }

    #[test]
    fn calculate_backoff_delay_no_panic_for_any_attempt(
        backoff_ms in any::<u64>(),
        multiplier in 1.0f64..=10.0f64,
        attempt in 0u32..=10_000,
    ) {
        let policy = RetryPolicy::new(10, backoff_ms, multiplier).unwrap();
        let delay = policy.calculate_backoff_delay(attempt);
        prop_assert!(delay <= u64::MAX);
    }

    #[test]
    fn with_max_backoff_zero_attempts_always_returns_zero_attempts(
        backoff_ms in any::<u64>(),
        multiplier in any::<f64>(),
        max_backoff in any::<u64>(),
    ) {
        let result = RetryPolicy::with_max_backoff(0, backoff_ms, multiplier, max_backoff);
        prop_assert!(matches!(result, Err(RetryPolicyError::ZeroAttempts)));
    }
}

// ============================================================================
// Adversarial tests (RQ-02, RQ-03, RQ-04)
// ============================================================================

#[test]
fn rq_struct_literal_nan_multiplier_no_panic() {
    let policy = RetryPolicy {
        max_attempts: 3,
        backoff_ms: 100,
        backoff_multiplier: f64::NAN,
        max_backoff_ms: u64::MAX,
    };
    let _delay = policy.calculate_backoff_delay(5);
}

#[test]
fn rq_calculate_backoff_delay_attempt_u32_max_no_panic() {
    let policy = RetryPolicy::new(10, 100, 2.0).unwrap();
    let delay = policy.calculate_backoff_delay(u32::MAX);
    assert!(delay <= u64::MAX);
}

#[test]
fn rq_calculate_backoff_delay_f64_max_multiplier_no_panic() {
    let policy = RetryPolicy {
        max_attempts: 3,
        backoff_ms: 1,
        backoff_multiplier: f64::MAX,
        max_backoff_ms: u64::MAX,
    };
    let delay = policy.calculate_backoff_delay(2);
    assert!(delay <= u64::MAX);
}

// ============================================================================
// Mutation killers (MT-01 through MT-06)
// ============================================================================

#[test]
fn mt_01_multiplier_exactly_one_is_accepted() {
    let result = RetryPolicy::new(3, 100, 1.0);
    assert!(result.is_ok(), "Multiplier exactly 1.0 should be accepted");
}

#[test]
fn mt_02_max_backoff_equals_backoff_ms_is_accepted() {
    let result = RetryPolicy::with_max_backoff(3, 100, 2.0, 100);
    assert!(result.is_ok(), "max_backoff_ms == backoff_ms should be accepted");
}

#[test]
fn mt_03_overflow_cap_with_large_backoff_and_attempt() {
    let policy = RetryPolicy::new(255, u64::MAX, 2.0).unwrap();
    let delay = policy.calculate_backoff_delay(63);
    assert_eq!(delay, u64::MAX, "Should cap at u64::MAX, not overflow");
}

#[test]
fn mt_04_saturating_sub_prevents_wrap_for_attempt_zero() {
    let policy = RetryPolicy::new(5, 100, 2.0).unwrap();
    assert_eq!(policy.calculate_backoff_delay(0), 0);
}

#[test]
fn mt_05_min_not_max_in_double_cap() {
    let policy = RetryPolicy::with_max_backoff(10, 100, 2.0, 300).unwrap();
    let delay = policy.calculate_backoff_delay(10);
    assert!(delay <= 300, "Delay should NOT exceed max_backoff_ms (300), got {}", delay);
}

#[test]
fn mt_06_attempt_zero_and_zero_backoff_both_return_zero() {
    let policy = RetryPolicy::new(5, 100, 2.0).unwrap();
    assert_eq!(policy.calculate_backoff_delay(0), 0, "attempt=0 with non-zero backoff_ms must return 0");

    let policy_zero = RetryPolicy::new(5, 0, 2.0).unwrap();
    assert_eq!(policy_zero.calculate_backoff_delay(1), 0, "attempt=1 with backoff_ms=0 must return 0");
}
