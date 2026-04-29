//! QA smoke tests: Rate limiter token bucket contract verification.
//!
//! These tests verify the contract invariants of the rate limiter and token bucket
//! implementation. They complement the existing integration, adversarial, and
//! property tests by focusing on end-to-end contract verification.

use std::sync::Arc;
use std::time::{Duration, Instant};

use vo_core::circuit_breaker::{
    check_rate_limit, evaluate_registration, record_failure, unquarantine, CircuitBreakerConfig,
    CircuitBreakerError, CircuitBreakerState, RegistrationOutcome, RegistrationRequest,
    RegistrationStatus, TokenBucketConfig, TokenBucketRateLimiter,
};
use vo_types::{BinaryHash, WorkflowName};

fn make_wf(s: &str) -> WorkflowName {
    WorkflowName::parse(s).expect("test workflow name should be valid")
}

fn make_hash(s: &str) -> BinaryHash {
    BinaryHash::parse(s).expect("test hash should be valid")
}

fn default_config() -> CircuitBreakerConfig {
    CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 5)
        .expect("default config should be valid")
}

fn make_request(workflow: &str, force: bool) -> RegistrationRequest {
    RegistrationRequest {
        workflow_name: make_wf(workflow),
        binary_hash: make_hash("abcdef01"),
        force: if force {
            Some("test-operator-token".into())
        } else {
            None
        },
    }
}

fn make_request_with_hash(workflow: &str, hash: &str, force: bool) -> RegistrationRequest {
    RegistrationRequest {
        workflow_name: make_wf(workflow),
        binary_hash: make_hash(hash),
        force: if force {
            Some("test-operator-token".into())
        } else {
            None
        },
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CR-001: check_rate_limit purity
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn qa_cr001_check_rate_limit_is_pure() {
    let t0 = Instant::now();
    let now = t0 + Duration::from_secs(10);
    let window = Duration::from_secs(60);

    let r1 = check_rate_limit(Some(t0), window, now);
    let r2 = check_rate_limit(Some(t0), window, now);
    let r3 = check_rate_limit(Some(t0), window, now);

    assert_eq!(
        r1, r2,
        "purity violation: repeated calls returned different results"
    );
    assert_eq!(
        r2, r3,
        "purity violation: repeated calls returned different results"
    );
}

#[test]
fn qa_cr001_check_rate_limit_none_is_idempotent() {
    let now = Instant::now();
    let window = Duration::from_secs(60);

    for _ in 0..100 {
        assert_eq!(
            check_rate_limit(None, window, now),
            None,
            "purity violation: None input should always return None"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CR-002: Ceiling arithmetic for sub-second precision
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn qa_cr002_ceiling_at_submillisecond_remaining() {
    let t0 = Instant::now();
    let window = Duration::from_secs(60);
    let now = t0 + Duration::from_nanos(59_999_999_999);

    let result = check_rate_limit(Some(t0), window, now);
    assert_eq!(
        result,
        Some(1),
        "1ns before window expiry should return Some(1)"
    );
}

#[test]
fn qa_cr002_ceiling_at_exactly_window_boundary() {
    let t0 = Instant::now();
    let window = Duration::from_secs(60);
    let now = t0 + window;

    let result = check_rate_limit(Some(t0), window, now);
    assert_eq!(
        result, None,
        "exactly at window boundary should return None"
    );
}

#[test]
fn qa_cr002_ceiling_one_nano_past_window() {
    let t0 = Instant::now();
    let window = Duration::from_secs(60);
    let now = t0 + Duration::from_nanos(60_000_000_001);

    let result = check_rate_limit(Some(t0), window, now);
    assert_eq!(result, None, "1ns past window should return None");
}

#[test]
fn qa_cr002_ceiling_half_second_remaining() {
    let t0 = Instant::now();
    let window = Duration::from_secs(60);
    let now = t0 + Duration::from_millis(59_500);

    let result = check_rate_limit(Some(t0), window, now);
    assert_eq!(result, Some(1), "500ms remaining should ceil to 1s");
}

// ═══════════════════════════════════════════════════════════════════════════════
// CR-003: TokenBucket atomic check-and-consume
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn qa_cr003_consume_decreases_tokens_atomically() {
    let config = TokenBucketConfig::new(5, 0.0, 1);
    let limiter = TokenBucketRateLimiter::new(config);
    let now = Instant::now();

    for i in 0..5 {
        let (allowed, _) = limiter.check_and_consume("key1", now);
        assert!(allowed, "request {} should be allowed", i + 1);
    }

    let (allowed, retry) = limiter.check_and_consume("key1", now);
    assert!(!allowed, "6th request should be denied (tokens exhausted)");
    assert!(retry > 0, "retry_after should be > 0 when denied");
}

#[test]
fn qa_cr003_consume_with_cost_gt_1() {
    let config = TokenBucketConfig::new(10, 0.0, 3);
    let limiter = TokenBucketRateLimiter::new(config);
    let now = Instant::now();

    assert!(
        limiter.check_and_consume("key1", now).0,
        "1st consume (10-3=7)"
    );
    assert!(
        limiter.check_and_consume("key1", now).0,
        "2nd consume (7-3=4)"
    );
    assert!(
        limiter.check_and_consume("key1", now).0,
        "3rd consume (4-3=1)"
    );
    assert!(
        !limiter.check_and_consume("key1", now).0,
        "4th consume (1<3, denied)"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// CR-004: Burst cap never violated
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn qa_cr004_tokens_never_exceed_burst_after_long_wait() {
    let config = TokenBucketConfig::new(5, 1000.0, 1);
    let limiter = TokenBucketRateLimiter::new(config);
    let now = Instant::now();

    limiter.check_and_consume("key1", now);

    let far_future = now + Duration::from_secs(86400);
    let tokens = limiter.available_tokens("key1", far_future);
    assert!(
        tokens <= 5.0,
        "tokens ({}) should never exceed burst (5.0) even after 1 day",
        tokens
    );
}

#[test]
fn qa_cr004_burst_cap_holds_across_multiple_replenishment_cycles() {
    let config = TokenBucketConfig::new(3, 10.0, 1);
    let limiter = TokenBucketRateLimiter::new(config);
    let now = Instant::now();

    for i in 1..=100 {
        let later = now + Duration::from_secs(i);
        let tokens = limiter.available_tokens("key1", later);
        assert!(
            tokens <= 3.0,
            "at t+{}s, tokens ({}) should not exceed burst (3.0)",
            i,
            tokens
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CR-005: Per-key isolation
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn qa_cr005_exhausting_one_key_does_not_affect_another() {
    let config = TokenBucketConfig::new(1, 0.0, 1);
    let limiter = TokenBucketRateLimiter::new(config);
    let now = Instant::now();

    limiter.check_and_consume("exhausted", now);
    let (allowed, _) = limiter.check_and_consume("exhausted", now);
    assert!(!allowed, "exhausted key should be denied");

    let (allowed, _) = limiter.check_and_consume("fresh", now);
    assert!(allowed, "fresh key should start with full burst");
}

#[test]
fn qa_cr005_many_keys_all_independent() {
    let config = TokenBucketConfig::new(1, 0.0, 1);
    let limiter = TokenBucketRateLimiter::new(config);
    let now = Instant::now();

    for i in 0..100 {
        let key = format!("key-{}", i);
        let (allowed, _) = limiter.check_and_consume(&key, now);
        assert!(allowed, "key {} should have full burst", key);
    }

    assert_eq!(
        limiter.key_count(),
        100,
        "should track 100 independent keys"
    );

    for i in 0..100 {
        let key = format!("key-{}", i);
        let (allowed, _) = limiter.check_and_consume(&key, now);
        assert!(!allowed, "key {} should be exhausted", key);
    }
}

#[test]
fn qa_cr005_reset_one_key_does_not_affect_others() {
    let config = TokenBucketConfig::new(1, 0.0, 1);
    let limiter = TokenBucketRateLimiter::new(config);
    let now = Instant::now();

    limiter.check_and_consume("a", now);
    limiter.check_and_consume("b", now);
    limiter.check_and_consume("c", now);

    limiter.reset("b");

    assert!(
        !limiter.check_and_consume("a", now).0,
        "a should still be exhausted"
    );
    assert!(limiter.check_and_consume("b", now).0, "b should be reset");
    assert!(
        !limiter.check_and_consume("c", now).0,
        "c should still be exhausted"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// CR-006: evaluate_registration TOCTOU-safe atomic rate limit update
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn qa_cr006_allowed_updates_rate_limiter_atomically() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();

    let req = make_request("deploy-prod", false);
    let result = evaluate_registration(&req, &config, &state, t0).unwrap();
    assert_eq!(result, RegistrationOutcome::Allowed);

    assert!(
        state.get_rate_limit(&req.workflow_name).is_some(),
        "rate limiter should be set after allowed registration"
    );

    let now_30s = t0 + Duration::from_secs(30);
    let result2 = evaluate_registration(&req, &config, &state, now_30s).unwrap();
    assert!(
        matches!(
            result2,
            RegistrationOutcome::RateLimited {
                retry_after_secs: 30
            }
        ),
        "second registration within window should be rate-limited, got {:?}",
        result2
    );
}

#[test]
fn qa_cr006_vacant_entry_creates_new_rate_limit() {
    let state = CircuitBreakerState::new();
    let config = default_config();

    assert_eq!(
        state.get_rate_limit(&make_wf("new-wf")),
        None,
        "new workflow should have no rate limit"
    );

    let req = make_request("new-wf", false);
    evaluate_registration(&req, &config, &state, Instant::now()).unwrap();

    assert!(
        state.get_rate_limit(&req.workflow_name).is_some(),
        "rate limiter should be set for new workflow after first registration"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// CR-007: Evaluation order (force > quarantine > deactivated > rate limit)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn qa_cr007_force_bypasses_quarantine() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let wf = make_wf("blocked-wf");

    state.set_status(wf, RegistrationStatus::Quarantined);

    state.register_operator_token("test-operator-token".into());
    let req = make_request("blocked-wf", true);
    let result = evaluate_registration(&req, &config, &state, Instant::now()).unwrap();
    assert_eq!(
        result,
        RegistrationOutcome::Allowed,
        "force should bypass quarantine"
    );
}

#[test]
fn qa_cr007_force_bypasses_deactivated() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let wf = make_wf("dead-wf");

    state.set_status(wf, RegistrationStatus::Deactivated);

    state.register_operator_token("test-operator-token".into());
    let req = make_request("dead-wf", true);
    let result = evaluate_registration(&req, &config, &state, Instant::now()).unwrap();
    assert_eq!(
        result,
        RegistrationOutcome::Allowed,
        "force should bypass deactivation"
    );
}

#[test]
fn qa_cr007_force_bypasses_rate_limit() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let wf = make_wf("limited-wf");

    state.set_rate_limit(wf, Instant::now());

    state.register_operator_token("test-operator-token".into());
    let req = make_request("limited-wf", true);
    let result = evaluate_registration(&req, &config, &state, Instant::now()).unwrap();
    assert_eq!(
        result,
        RegistrationOutcome::Allowed,
        "force should bypass rate limit"
    );
}

#[test]
fn qa_cr007_quarantine_takes_precedence_over_rate_limit() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let wf = make_wf("both-wf");
    let t0 = Instant::now();

    state.set_status(wf.clone(), RegistrationStatus::Quarantined);
    state.set_rate_limit(wf, t0);

    let req = make_request("both-wf", false);
    let result = evaluate_registration(&req, &config, &state, t0).unwrap();
    assert!(
        matches!(result, RegistrationOutcome::WorkflowQuarantined { .. }),
        "quarantine should take precedence over rate limit, got {:?}",
        result
    );
}

#[test]
fn qa_cr007_deactivated_takes_precedence_over_rate_limit() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let wf = make_wf("deactivated-limited-wf");
    let t0 = Instant::now();

    state.set_status(wf.clone(), RegistrationStatus::Deactivated);
    state.set_rate_limit(wf, t0);

    let req = make_request("deactivated-limited-wf", false);
    let result = evaluate_registration(&req, &config, &state, t0).unwrap();
    assert!(
        matches!(result, RegistrationOutcome::WorkflowDeactivated { .. }),
        "deactivated should take precedence over rate limit, got {:?}",
        result
    );
}

#[test]
fn qa_cr007_full_ordering_force_wins_over_all() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let wf = make_wf("triple-wf");
    let t0 = Instant::now();

    state.set_status(wf.clone(), RegistrationStatus::Quarantined);
    state.set_rate_limit(wf, t0);

    state.register_operator_token("test-operator-token".into());
    let req = make_request("triple-wf", true);
    let result = evaluate_registration(&req, &config, &state, t0).unwrap();
    assert_eq!(
        result,
        RegistrationOutcome::Allowed,
        "force wins over quarantine+rate_limit"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// CR-008: unquarantine resets all state
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn qa_cr008_unquarantine_clears_rate_limiter() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let wf = make_wf("reset-wf");
    let t0 = Instant::now();

    let hashes = ["aaaa0001", "aaaa0002", "aaaa0003", "aaaa0004", "aaaa0005"];
    for h in &hashes {
        let hash = make_hash(h);
        let event = record_failure(&wf, &hash, &config, &state, t0).unwrap();
        assert!(event.is_some() || hashes.iter().position(|x| x == h).unwrap() < 4);
    }

    assert_eq!(state.get_status(&wf), RegistrationStatus::Quarantined);
    state.set_rate_limit(wf.clone(), t0);

    let result = unquarantine(&wf, "admin", &state).unwrap();
    assert_eq!(result.failures_cleared, 5);
    assert_eq!(result.previous_status, RegistrationStatus::Quarantined);
    assert_eq!(result.new_status, RegistrationStatus::Active);

    assert_eq!(
        state.get_rate_limit(&wf),
        None,
        "rate limiter should be cleared"
    );
    assert_eq!(
        state.get_failure_count(&wf),
        0,
        "failures should be cleared"
    );
    assert_eq!(state.get_status(&wf), RegistrationStatus::Active);
}

#[test]
fn qa_cr008_unquarantine_allows_immediate_registration() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let wf = make_wf("fast-wf");
    let t0 = Instant::now();

    state.set_status(wf.clone(), RegistrationStatus::Quarantined);
    state.set_rate_limit(wf.clone(), t0);

    unquarantine(&wf, "admin", &state).unwrap();

    let req = make_request("fast-wf", false);
    let result = evaluate_registration(&req, &config, &state, t0).unwrap();
    assert_eq!(
        result,
        RegistrationOutcome::Allowed,
        "should be allowed immediately after unquarantine"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// CR-009: Zero sustained rate = permanent denial
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn qa_cr009_zero_rate_never_replenishes() {
    let config = TokenBucketConfig::new(3, 0.0, 1);
    let limiter = TokenBucketRateLimiter::new(config);
    let now = Instant::now();

    for _ in 0..3 {
        limiter.check_and_consume("key1", now);
    }

    let far_future = now + Duration::from_secs(999_999);
    let (allowed, retry) = limiter.check_and_consume("key1", far_future);
    assert!(!allowed, "zero rate should never replenish");
    assert_eq!(
        retry,
        u64::MAX,
        "wait_time should be u64::MAX for zero rate"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// CR-010: Config validation completeness
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn qa_cr010_config_rejects_each_zero_field_independently() {
    use vo_core::circuit_breaker::ConfigValidationError;

    assert_eq!(
        CircuitBreakerConfig::new(Duration::ZERO, Duration::from_secs(600), 5),
        Err(ConfigValidationError::ZeroRateLimitWindow)
    );
    assert_eq!(
        CircuitBreakerConfig::new(Duration::from_secs(60), Duration::ZERO, 5),
        Err(ConfigValidationError::ZeroFailureWindow)
    );
    assert_eq!(
        CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 0),
        Err(ConfigValidationError::ZeroFailureThreshold)
    );
}

#[test]
fn qa_cr010_default_config_is_valid() {
    let config = CircuitBreakerConfig::default_config().expect("default config should be valid");
    assert_eq!(config.rate_limit_window, Duration::from_secs(60));
    assert_eq!(config.failure_window, Duration::from_secs(600));
    assert_eq!(config.failure_threshold, 5);
}

// ═══════════════════════════════════════════════════════════════════════════════
// CR-011: Concurrent token bucket safety
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn qa_cr011_concurrent_check_and_consume_no_panics() {
    let config = TokenBucketConfig::new(100, 10.0, 1);
    let limiter = Arc::new(TokenBucketRateLimiter::new(config));
    let now = Instant::now();

    let mut handles = vec![];
    for i in 0..10 {
        let limiter = Arc::clone(&limiter);
        handles.push(std::thread::spawn(move || {
            let key = format!("key-{}", i % 3);
            for _ in 0..50 {
                let (allowed, _) = limiter.check_and_consume(&key, now);
                let _ = allowed;
            }
        }));
    }

    for handle in handles {
        handle.join().expect("thread should not panic");
    }
}

#[test]
fn qa_cr011_concurrent_peek_does_not_modify_state() {
    let config = TokenBucketConfig::new(10, 10.0, 1);
    let limiter = Arc::new(TokenBucketRateLimiter::new(config));
    let now = Instant::now();

    limiter.check_and_consume("key1", now);
    let tokens_before = limiter.available_tokens("key1", now);

    let mut handles = vec![];
    for _ in 0..100 {
        let limiter = Arc::clone(&limiter);
        handles.push(std::thread::spawn(move || {
            limiter.peek_tokens("key1", now);
        }));
    }

    for handle in handles {
        handle.join().expect("thread should not panic");
    }

    let tokens_after = limiter.available_tokens("key1", now);
    assert!(
        (tokens_before - tokens_after).abs() < 0.001,
        "peek_tokens should not modify state: before={}, after={}",
        tokens_before,
        tokens_after
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// CR-012: Full lifecycle
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn qa_cr012_full_lifecycle_happy_path() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let wf = make_wf("lifecycle-wf");
    let t0 = Instant::now();

    let req = make_request("lifecycle-wf", false);
    let result = evaluate_registration(&req, &config, &state, t0).unwrap();
    assert_eq!(result, RegistrationOutcome::Allowed);

    let hashes = ["aaaa0001", "aaaa0002", "aaaa0003", "aaaa0004", "aaaa0005"];
    for (i, h) in hashes.iter().enumerate() {
        let hash = make_hash(h);
        let event = record_failure(&wf, &hash, &config, &state, t0).unwrap();
        if i < 4 {
            assert_eq!(event, None, "should not quarantine before threshold");
        } else {
            assert!(event.is_some(), "should quarantine at threshold");
        }
    }

    assert_eq!(state.get_status(&wf), RegistrationStatus::Quarantined);

    let req = make_request("lifecycle-wf", false);
    let result = evaluate_registration(&req, &config, &state, t0).unwrap();
    assert!(
        matches!(result, RegistrationOutcome::WorkflowQuarantined { .. }),
        "quarantined workflow should block registration"
    );

    let result = unquarantine(&wf, "admin", &state).unwrap();
    assert_eq!(result.failures_cleared, 5);

    assert_eq!(state.get_status(&wf), RegistrationStatus::Active);

    let req = make_request("lifecycle-wf", false);
    let result = evaluate_registration(&req, &config, &state, t0).unwrap();
    assert_eq!(
        result,
        RegistrationOutcome::Allowed,
        "should be allowed after unquarantine"
    );
}

#[test]
fn qa_cr012_rate_limited_requests_dont_count_as_failures() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let wf = make_wf("rate-only-wf");
    let t0 = Instant::now();

    let req = make_request("rate-only-wf", false);
    evaluate_registration(&req, &config, &state, t0).unwrap();

    let req2 = make_request_with_hash("rate-only-wf", "abcdef02", false);
    let result = evaluate_registration(&req2, &config, &state, t0).unwrap();
    assert!(matches!(result, RegistrationOutcome::RateLimited { .. }));

    assert_eq!(
        state.get_failure_count(&wf),
        0,
        "rate-limited request should not create failures"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// CR-013: TokenBucket wait_time accuracy
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn qa_cr013_wait_time_is_zero_when_tokens_available() {
    let config = TokenBucketConfig::new(10, 5.0, 1);
    let limiter = TokenBucketRateLimiter::new(config);
    let now = Instant::now();

    assert_eq!(
        limiter.wait_time("key1", now),
        0,
        "new key should have zero wait time"
    );
}

#[test]
fn qa_cr013_wait_time_matches_replenishment_rate() {
    let config = TokenBucketConfig::new(10, 10.0, 10);
    let limiter = TokenBucketRateLimiter::new(config);
    let now = Instant::now();

    limiter.check_and_consume("key1", now);

    let (allowed, _) = limiter.check_and_consume("key1", now);
    assert!(!allowed, "second consume should fail");

    let wait = limiter.wait_time("key1", now);
    assert!(
        wait >= 1,
        "wait_time should be >= 1s to replenish 10 tokens at 10/sec"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// CR-014: Error variant completeness
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn qa_cr014_all_error_variants_are_constructable() {
    let _ = CircuitBreakerError::RateLimited {
        retry_after_secs: 60,
    };
    let _ = CircuitBreakerError::WorkflowQuarantined {
        workflow_name: "test".to_string(),
    };
    let _ = CircuitBreakerError::WorkflowDeactivated {
        workflow_name: "test".to_string(),
    };
    let _ = CircuitBreakerError::StorageError {
        reason: "disk full".to_string(),
    };
    let _ = CircuitBreakerError::WorkflowNotFound {
        workflow_name: "test".to_string(),
    };
    let _ = CircuitBreakerError::NotQuarantined {
        workflow_name: "test".to_string(),
        current_status: RegistrationStatus::Active,
    };
}

#[test]
fn qa_cr014_error_display_formatting() {
    assert_eq!(
        format!(
            "{}",
            CircuitBreakerError::RateLimited {
                retry_after_secs: 42
            }
        ),
        "rate_limited: retry after 42s"
    );
    assert_eq!(
        format!(
            "{}",
            CircuitBreakerError::WorkflowQuarantined {
                workflow_name: "deploy".to_string()
            }
        ),
        "workflow_quarantined: deploy"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// CR-015: Unquarantine error cases
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn qa_cr015_unquarantine_unknown_workflow_returns_not_found() {
    let state = CircuitBreakerState::new();
    let wf = make_wf("unknown-wf");

    let result = unquarantine(&wf, "admin", &state);
    assert!(
        matches!(result, Err(CircuitBreakerError::WorkflowNotFound { .. })),
        "unknown workflow should return WorkflowNotFound"
    );
}

#[test]
fn qa_cr015_unquarantine_active_workflow_returns_not_quarantined() {
    let state = CircuitBreakerState::new();
    let wf = make_wf("active-wf");

    state.set_status(wf.clone(), RegistrationStatus::Active);

    let result = unquarantine(&wf, "admin", &state);
    assert!(
        matches!(result, Err(CircuitBreakerError::NotQuarantined { .. })),
        "active workflow should return NotQuarantined"
    );
}

#[test]
fn qa_cr015_unquarantine_deactivated_returns_not_quarantined() {
    let state = CircuitBreakerState::new();
    let wf = make_wf("deactivated-wf");

    state.set_status(wf.clone(), RegistrationStatus::Deactivated);

    let result = unquarantine(&wf, "admin", &state);
    assert!(
        matches!(result, Err(CircuitBreakerError::NotQuarantined { .. })),
        "deactivated workflow should return NotQuarantined"
    );
}
