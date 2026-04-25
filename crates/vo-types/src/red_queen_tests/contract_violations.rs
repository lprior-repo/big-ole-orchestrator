//! Red Queen tests: contract-violations dimension.
//!
//! Tests NaN/INFINITY bypass and direct construction attacks on RetryPolicy.

use crate::*;

// RQ-01: NaN multiplier is rejected by RetryPolicy::new()
// NaN < 1.0 is FALSE in IEEE 754, but we explicitly check is_nan().
#[test]
fn rq_nan_multiplier_rejected_by_retry_policy_new() {
    let result = RetryPolicy::new(1, 0, f64::NAN);
    assert!(
        matches!(result, Err(RetryPolicyError::InvalidMultiplier { .. })),
        "NaN must be rejected"
    );
    let err = result.unwrap_err();
    assert!(err.to_string().contains("backoff_multiplier"));
}

// RQ-02: INFINITY multiplier is rejected by RetryPolicy::new()
// Updated: is_finite() check now rejects INFINITY.
#[test]
fn rq_infinity_multiplier_rejected_by_retry_policy_new() {
    let result = RetryPolicy::new(1, 0, f64::INFINITY);
    assert!(
        matches!(result, Err(RetryPolicyError::InvalidMultiplier { .. })),
        "INFINITY must be rejected by is_finite() check"
    );
}

// RQ-03: NEG_INFINITY multiplier is correctly rejected
#[test]
fn rq_neg_infinity_multiplier_rejected() {
    let result = RetryPolicy::new(1, 0, f64::NEG_INFINITY);
    assert!(matches!(
        result,
        Err(RetryPolicyError::InvalidMultiplier { .. })
    ));
}

// RQ-04: NaN multiplier in JSON is rejected by serde
#[test]
fn rq_nan_multiplier_in_json_rejected_by_serde() {
    let json = r#"{"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": NaN}"#;
    let result: Result<RetryPolicy, _> = serde_json::from_str(json);
    result.unwrap_err();
}

// RQ-05: INFINITY multiplier in JSON is rejected by serde
#[test]
fn rq_infinity_multiplier_in_json_rejected_by_serde() {
    let json = r#"{"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": Infinity}"#;
    let result: Result<RetryPolicy, _> = serde_json::from_str(json);
    result.unwrap_err();
}

// RQ-05b: -INFINITY multiplier in JSON is rejected by serde
#[test]
fn rq_neg_infinity_multiplier_in_json_rejected_by_serde() {
    let json = r#"{"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": -Infinity}"#;
    let result: Result<RetryPolicy, _> = serde_json::from_str(json);
    result.unwrap_err();
}

// RQ-06: Direct RetryPolicy construction bypasses validation (fields are pub)
#[test]
fn rq_direct_retry_policy_construction_allows_invalid_state() {
    // Fields are pub, so direct construction bypasses RetryPolicy::new() validation
    let policy = RetryPolicy {
        max_attempts: 0,
        backoff_ms: 0,
        backoff_multiplier: 0.0,
        max_backoff_ms: u64::MAX,
    };
    assert_eq!(policy.max_attempts, 0);
    assert_eq!(policy.backoff_multiplier, 0.0);
    // This is "by design" (pub fields) but means invariants are only enforced
    // through the parse() + new() constructors.
}
