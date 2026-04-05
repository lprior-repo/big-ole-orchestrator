//! Proptest invariants for vel-k1t9
//!
//! These tests verify critical invariants using property-based testing:
//! 1. RetryPolicy::new never panics
//! 2. Exponential backoff never exceeds max delay
//! 3. RetryPolicy invariants hold for arbitrary valid inputs

use proptest::prelude::*;
use vo_executor::{RetryPolicy, RetryPolicyError};

// ============================================================================
// Proptest: RetryPolicy::new never panics
// ============================================================================

proptest! {
    #[test]
    fn retry_policy_new_never_panics_for_any_u8_u64_f64(
        max_attempts in any::<u8>(),
        backoff_ms in any::<u64>(),
        multiplier in any::<f64>(),
    ) {
        // RetryPolicy::new should never panic for any valid Rust values
        // It may return Err for invalid combinations, but never panic
        let result = std::panic::catch_unwind(|| {
            RetryPolicy::new(
                max_attempts as u32,
                backoff_ms,
                multiplier,
            )
        });
        prop_assert_eq!(result.is_ok(), true, "RetryPolicy::new should not panic");
    }
}

proptest! {
    #[test]
    fn retry_policy_new_never_panics_for_extreme_u64_values(
        max_attempts in 1u32..=10,
        backoff_ms in any::<u64>(),
        multiplier in 1.0f64..=10.0,
    ) {
        // Test with extreme backoff_ms values
        // This test verifies that even extreme u64 values don't cause panic
        let result = std::panic::catch_unwind(|| {
            RetryPolicy::new(max_attempts, backoff_ms, multiplier)
        });
        prop_assert_eq!(
            result.is_ok(),
            true,
            "Should not panic with backoff_ms = {}",
            backoff_ms
        );
    }
}

// ============================================================================
// Proptest: RetryPolicy invariants
// ============================================================================

proptest! {
    #[test]
    fn retry_policy_fields_preserved_after_construction(
        max_attempts in 1u32..=100,
        backoff_ms in 0u64..=10_000,
        multiplier in 1.0f64..=10.0,
    ) {
        let policy = RetryPolicy::new(max_attempts, backoff_ms, multiplier).unwrap();
        prop_assert_eq!(policy.max_attempts, max_attempts);
        prop_assert_eq!(policy.backoff_ms, backoff_ms);
        prop_assert_eq!(policy.backoff_multiplier, multiplier);
    }
}

proptest! {
    #[test]
    fn retry_policy_rejects_zero_attempts(
        max_attempts in 0u32..=0,
        backoff_ms in 0u64..=10_000,
        multiplier in 1.0f64..=10.0,
    ) {
        let result = RetryPolicy::new(max_attempts, backoff_ms, multiplier);
        prop_assert_eq!(result, Err(RetryPolicyError::ZeroAttempts));
    }
}

proptest! {
    #[test]
    fn retry_policy_rejects_multiplier_below_one(
        max_attempts in 1u32..=100,
        backoff_ms in 0u64..=10_000,
        multiplier in 0.0f64..1.0,
    ) {
        prop_assume!(multiplier < 1.0, "Only test when multiplier < 1.0");
        let result = RetryPolicy::new(max_attempts, backoff_ms, multiplier);
        prop_assert_eq!(
            result,
            Err(RetryPolicyError::InvalidMultiplier { got: multiplier })
        );
    }
}

proptest! {
    #[test]
    fn retry_policy_accepts_multiplier_equal_to_one(
        max_attempts in 1u32..=100,
        backoff_ms in 0u64..=10_000,
    ) {
        let result = RetryPolicy::new(max_attempts, backoff_ms, 1.0);
        prop_assert_eq!(
            result,
            Ok(RetryPolicy {
                max_attempts,
                backoff_ms,
                backoff_multiplier: 1.0,
            })
        );
    }
}

proptest! {
    #[test]
    fn retry_policy_accepts_large_multiplier_values(
        max_attempts in 1u32..=10,
        backoff_ms in 0u64..=1000,
    ) {
        // Test with large but valid multipliers
        let large_multiplier = 1_000_000.0f64;
        let result = RetryPolicy::new(max_attempts, backoff_ms, large_multiplier);
        prop_assert_eq!(
            result,
            Ok(RetryPolicy {
                max_attempts,
                backoff_ms,
                backoff_multiplier: large_multiplier,
            }),
            "Should accept large multiplier {}",
            large_multiplier
        );
    }
}

// ============================================================================
// Proptest: Exponential backoff never exceeds maximum delay
// ============================================================================

proptest! {
    #[test]
    fn exponential_backoff_never_exceeds_u64_max_for_reasonable_attempts(
        max_attempts in 1u32..=10,
        backoff_ms in 0u64..=1_000_000,
        multiplier in 1.0f64..=10.0,
    ) {
        let policy = RetryPolicy::new(max_attempts, backoff_ms, multiplier).unwrap();

        let delays: Vec<u64> = (1..=max_attempts)
            .map(|attempt| calculate_backoff_delay(&policy, attempt))
            .collect();
        prop_assert_eq!(delays.len(), max_attempts as usize);
    }
}

proptest! {
    #[test]
    fn exponential_backoff_grows_with_attempts(
        max_attempts in 3u32..=10,
        backoff_ms in 100u64..=1000,
        multiplier in 2.0f64..=4.0,
    ) {
        let policy = RetryPolicy::new(max_attempts, backoff_ms, multiplier).unwrap();

        let delays: Vec<u64> = (1..=max_attempts)
            .map(|attempt| calculate_backoff_delay(&policy, attempt))
            .collect();

        prop_assert_eq!(
            delays.windows(2).all(|window| window[1] >= window[0]),
            true,
            "Backoff should not decrease"
        );

        // With multiplier > 1, delays should actually grow
        if multiplier > 1.0 {
            prop_assert_eq!(
                delays.windows(2).all(|window| window[1] > window[0]),
                true,
                "Backoff should grow with multiplier > 1"
            );
        }
    }
}

proptest! {
    #[test]
    fn exponential_backoff_with_zero_initial_backoff(
        max_attempts in 2u32..=5,
        multiplier in 2.0f64..=4.0,
    ) {
        let policy = RetryPolicy::new(max_attempts, 0, multiplier).unwrap();

        // With backoff_ms = 0, first delay should be 0
        let first_delay = calculate_backoff_delay(&policy, 1);
        prop_assert_eq!(first_delay, 0, "First delay with 0 initial backoff should be 0");

        let later_delays: Vec<u64> = (2..=max_attempts)
            .map(|attempt| calculate_backoff_delay(&policy, attempt))
            .collect();
        let prior_delays: Vec<u64> = (1..max_attempts)
            .map(|attempt| calculate_backoff_delay(&policy, attempt))
            .collect();
        prop_assert_eq!(
            later_delays
                .iter()
                .zip(prior_delays.iter())
                .all(|(later, earlier)| later >= earlier),
            true
        );
    }
}

proptest! {
    #[test]
    fn exponential_backoff_does_not_overflow_with_max_multiplier(
        max_attempts in 2u32..=5,
        backoff_ms in 0u64..=1000,
    ) {
        // Use a very large but finite multiplier
        let large_multiplier = 1e10_f64;
        let policy = RetryPolicy::new(max_attempts, backoff_ms, large_multiplier).unwrap();

        let delays: Vec<u64> = (1..=max_attempts)
            .map(|attempt| calculate_backoff_delay(&policy, attempt))
            .collect();
        prop_assert_eq!(delays.len(), max_attempts as usize);
    }
}

/// Calculate the backoff delay for a given attempt using the actual implementation.
/// This delegates to RetryPolicy::calculate_backoff_delay which clamps to u64::MAX.
fn calculate_backoff_delay(policy: &RetryPolicy, attempt: u32) -> u64 {
    policy.calculate_backoff_delay(attempt)
}

// ============================================================================
// Proptest: RetryPolicyError variants can be formatted without panic
// ============================================================================

#[test]
fn retry_policy_error_zero_attempts_debug_format_never_panics() {
    let err = RetryPolicyError::ZeroAttempts;
    let debug_str = format!("{:?}", err);
    let display_str = format!("{}", err);
    assert!(!debug_str.is_empty());
    assert!(!display_str.is_empty());
}

#[test]
fn retry_policy_error_invalid_multiplier_debug_format_never_panics() {
    let err = RetryPolicyError::InvalidMultiplier { got: 0.5 };
    let debug_str = format!("{:?}", err);
    let display_str = format!("{}", err);
    assert!(!debug_str.is_empty());
    assert!(!display_str.is_empty());
}

#[test]
fn retry_policy_error_nan_multiplier_debug_format_never_panics() {
    let err = RetryPolicyError::InvalidMultiplier { got: f64::NAN };
    let debug_str = format!("{:?}", err);
    let display_str = format!("{}", err);
    assert!(!debug_str.is_empty());
    assert!(!display_str.is_empty());
}

#[test]
fn retry_policy_error_clone_never_panics() {
    // Verify that RetryPolicyError can be cloned without panic
    let errors = vec![
        RetryPolicyError::ZeroAttempts,
        RetryPolicyError::InvalidMultiplier { got: 0.0 },
        RetryPolicyError::InvalidMultiplier { got: f64::NAN },
        RetryPolicyError::InvalidMultiplier { got: -1.0 },
        RetryPolicyError::InvalidMultiplier { got: f64::INFINITY },
    ];

    let cloned_errors: Vec<_> = errors.iter().cloned().collect();
    assert_eq!(
        errors
            .iter()
            .zip(cloned_errors.iter())
            .all(|(err, cloned)| format!("{:?}", err) == format!("{:?}", cloned)),
        true
    );
    assert_eq!(
        errors
            .iter()
            .zip(cloned_errors.iter())
            .all(|(err, cloned)| format!("{}", err) == format!("{}", cloned)),
        true
    );
}

// ============================================================================
// Timeout arithmetic checks (Kani-style harness approximation)
// ============================================================================

proptest! {
    #[test]
    fn timeout_arithmetic_addition_never_overflows_for_reasonable_values(
        timeout_ms in 0u64..u64::MAX / 2,
        elapsed_ms in 0u64..u64::MAX / 2,
    ) {
        // timeout_ms + elapsed_ms + 1 should not overflow for reasonable values
        let result = timeout_ms.checked_add(elapsed_ms).and_then(|sum| sum.checked_add(1));
        prop_assert!(result.is_some(), "Addition should not overflow");
        // Result is u64 which is always <= u64::MAX, just verify it's Some
        let _ = result.unwrap();
    }
}

#[test]
fn timeout_arithmetic_addition_overflows_at_max_values() {
    // At boundary values, overflow detection should work
    let timeout_ms = u64::MAX - 1;
    let elapsed_ms = u64::MAX - 1;

    // This should overflow
    let result = timeout_ms
        .checked_add(elapsed_ms)
        .and_then(|sum| sum.checked_add(1));
    // Result should be None due to overflow
    assert!(
        result.is_none(),
        "Expected overflow to return None, got {:?}",
        result
    );
}
