//! Cross-crate equivalence tests (XT-01, XT-02) — TDD Red Phase
//!
//! Verifies behavioral equivalence between vo-types and vo-executor RetryPolicy.
//! Since vo-executor is not a dependency of vo-types, we replicate the
//! vo-executor formula here and compare with vo-types implementation.

use vo_types::RetryPolicy;
use vo_types::RetryPolicyError;

/// Replicate vo-executor's calculate_backoff_delay formula for comparison.
/// Key difference: vo-executor uses u32 max_attempts, vo-types uses u8.
/// The formula is identical: min(backoff_ms * multiplier^(attempt-1), max_backoff_ms)
fn vo_executor_calculate_backoff(
    backoff_ms: u64,
    multiplier: f64,
    max_backoff_ms: u64,
    attempt: u32,
) -> u64 {
    if attempt == 0 || backoff_ms == 0 {
        return 0;
    }
    let exponent = attempt.saturating_sub(1);
    let multiplier_pow = multiplier.powi(exponent as i32);
    let product = backoff_ms as f64 * multiplier_pow;
    let capped = product.min(max_backoff_ms as f64).min(u64::MAX as f64);
    capped as u64
}

// ============================================================================
// XT-01: calculate_backoff_delay produces identical results
// ============================================================================

#[test]
fn cross_crate_calculate_backoff_delay_matches() {
    let test_cases: &[(u64, f64, u64, u32)] = &[
        (100, 2.0, u64::MAX, 1),
        (100, 2.0, u64::MAX, 2),
        (100, 2.0, u64::MAX, 3),
        (100, 2.0, u64::MAX, 10),
        (100, 1.5, u64::MAX, 1),
        (100, 1.5, u64::MAX, 4),
        (500, 1.0, u64::MAX, 1),
        (500, 1.0, u64::MAX, 50),
        (100, 2.0, 300, 1),
        (100, 2.0, 300, 2),
        (100, 2.0, 300, 3),
        (100, 2.0, 300, 10),
        (1, 2.0, u64::MAX, 10),
        (0, 2.0, u64::MAX, 5),
        (100, 2.0, u64::MAX, 0),
        (u64::MAX, 2.0, u64::MAX, 63),
    ];

    for &(backoff_ms, multiplier, max_backoff_ms, attempt) in test_cases {
        let vo_types_policy = if max_backoff_ms == u64::MAX {
            RetryPolicy::new(10, backoff_ms, multiplier).unwrap()
        } else {
            RetryPolicy::with_max_backoff(10, backoff_ms, multiplier, max_backoff_ms).unwrap()
        };
        let types_result = vo_types_policy.calculate_backoff_delay(attempt);
        let executor_result =
            vo_executor_calculate_backoff(backoff_ms, multiplier, max_backoff_ms, attempt);
        assert_eq!(
            types_result, executor_result,
            "Mismatch at backoff_ms={}, mult={}, max={}, attempt={}: types={}, executor_formula={}",
            backoff_ms, multiplier, max_backoff_ms, attempt, types_result, executor_result
        );
    }
}

// ============================================================================
// XT-02: Error variant structure matches across crates
// vo-executor RetryPolicyError:
//   ZeroAttempts: "Zero attempts not allowed"
//   InvalidMultiplier { got: f64 }: "Invalid multiplier: {got} (must be >= 1.0)"
//   MaxBackoffTooSmall { max, ms }: "max_backoff_ms ({max}) must be >= backoff_ms ({ms})"
//
// vo-types RetryPolicyError:
//   ZeroAttempts: "max_attempts must be >= 1, got 0"
//   InvalidMultiplier { got: f64 }: "backoff_multiplier must be >= 1.0, got {got}"
//   MaxBackoffTooSmall { max, ms }: "max_backoff_ms ({max}) must be >= backoff_ms ({ms})"
// ============================================================================

#[test]
fn cross_crate_zero_attempts_error_exists_in_both() {
    // Both crates have this variant
    let types_err = RetryPolicyError::ZeroAttempts;
    assert!(!types_err.to_string().is_empty());
}

#[test]
fn cross_crate_invalid_multiplier_error_structure_matches() {
    let types_err = RetryPolicyError::InvalidMultiplier { got: 0.5 };
    assert!(types_err.to_string().contains("0.5"));
}

#[test]
fn cross_crate_max_backoff_too_small_error_structure_matches() {
    let types_err = RetryPolicyError::MaxBackoffTooSmall { max: 50, ms: 200 };
    let display = types_err.to_string();
    assert!(display.contains("50"));
    assert!(display.contains("200"));
}
