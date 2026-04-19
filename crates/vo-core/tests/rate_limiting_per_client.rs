//! Integration tests for admission control rate limiting per client (ve-4jp1u).
//!
//! Tests the token bucket rate limiter's per-client behavior:
//! under limit, at limit, over limit, and multi-client isolation.

use std::time::{Duration, Instant};

use vo_core::circuit_breaker::rate_limiter::{TokenBucketConfig, TokenBucketRateLimiter};

fn make_limiter(burst: u64, rate: f64, cost: u64) -> TokenBucketRateLimiter {
    TokenBucketRateLimiter::new(TokenBucketConfig::new(burst, rate, cost))
}

// ---------------------------------------------------------------------------
// Under limit: requests succeed while tokens available
// ---------------------------------------------------------------------------

#[test]
fn under_limit_single_client_requests_succeed() {
    let limiter = make_limiter(10, 1.0, 1);
    let now = Instant::now();

    // All 10 requests within burst should succeed
    for _ in 0..10 {
        let (allowed, retry) = limiter.check_and_consume("client-a", now);
        assert!(allowed, "request should succeed under limit");
        assert_eq!(retry, 0);
    }
}

#[test]
fn under_limit_multiple_clients_all_succeed() {
    let limiter = make_limiter(5, 1.0, 1);
    let now = Instant::now();

    for client in ["client-a", "client-b", "client-c"] {
        for _ in 0..5 {
            let (allowed, _) = limiter.check_and_consume(client, now);
            assert!(allowed, "{client} should succeed under limit");
        }
    }
}

#[test]
fn under_limit_with_cost_greater_than_one() {
    let limiter = make_limiter(10, 1.0, 3);
    let now = Instant::now();

    // 10 tokens / 3 cost = 3 requests before exhausted
    assert!(limiter.check_and_consume("client-a", now).0);
    assert!(limiter.check_and_consume("client-a", now).0);
    assert!(limiter.check_and_consume("client-a", now).0);
    // 4th request exceeds (only 1 token left, needs 3)
    assert!(!limiter.check_and_consume("client-a", now).0);
}

// ---------------------------------------------------------------------------
// At limit: boundary behavior
// ---------------------------------------------------------------------------

#[test]
fn at_limit_last_request_consumes_exact_burst() {
    let limiter = make_limiter(5, 0.0, 1);
    let now = Instant::now();

    // Exactly 5 requests should succeed
    for i in 0..5 {
        let (allowed, _) = limiter.check_and_consume("client-a", now);
        assert!(allowed, "request {i} should succeed at limit boundary");
    }

    // The 6th request should fail (at limit)
    let (allowed, retry) = limiter.check_and_consume("client-a", now);
    assert!(!allowed, "request beyond burst should fail");
    assert!(retry > 0, "should report positive retry time");
}

#[test]
fn at_limit_zero_tokens_after_exact_burst() {
    let limiter = make_limiter(5, 0.0, 5);
    let now = Instant::now();

    // One request consumes entire burst
    let (allowed, _) = limiter.check_and_consume("client-a", now);
    assert!(allowed);

    // Zero tokens remaining
    let tokens = limiter.available_tokens("client-a", now);
    assert!((tokens).abs() < 0.001, "tokens should be 0, got {tokens}");
}

#[test]
fn at_limit_cost_equals_burst_succeeds_once() {
    let limiter = make_limiter(5, 0.0, 5);
    let now = Instant::now();

    let (allowed, retry) = limiter.check_and_consume("client-a", now);
    assert!(allowed, "single request with cost=burst should succeed");
    assert_eq!(retry, 0);
}

// ---------------------------------------------------------------------------
// Over limit: requests denied when tokens exhausted
// ---------------------------------------------------------------------------

#[test]
fn over_limit_denies_request_after_burst_exhausted() {
    let limiter = make_limiter(3, 0.0, 1);
    let now = Instant::now();

    // Exhaust burst
    for _ in 0..3 {
        limiter.check_and_consume("client-a", now);
    }

    // Over limit
    let (allowed, retry) = limiter.check_and_consume("client-a", now);
    assert!(!allowed, "request over limit should be denied");
    assert!(retry > 0, "should report retry time");
}

#[test]
fn over_limit_retry_time_increases_with_deficit() {
    let limiter = make_limiter(2, 10.0, 1);
    let now = Instant::now();

    // Exhaust
    limiter.check_and_consume("client-a", now);
    limiter.check_and_consume("client-a", now);

    let (_, retry1) = limiter.check_and_consume("client-a", now);
    // More failed requests = larger deficit = longer retry
    let (_, retry2) = limiter.check_and_consume("client-a", now);

    assert!(retry2 >= retry1, "retry time should not decrease");
}

#[test]
fn over_limit_succeeds_after_replenishment() {
    let limiter = make_limiter(5, 10.0, 1);
    let now = Instant::now();

    // Exhaust
    for _ in 0..5 {
        limiter.check_and_consume("client-a", now);
    }

    // Over limit at current time
    assert!(!limiter.check_and_consume("client-a", now).0);

    // After 1 second, 10 tokens replenished (capped at burst=5)
    let later = now + Duration::from_secs(1);
    let (allowed, _) = limiter.check_and_consume("client-a", later);
    assert!(allowed, "should succeed after token replenishment");
}

// ---------------------------------------------------------------------------
// Per-client isolation
// ---------------------------------------------------------------------------

#[test]
fn per_client_isolation_one_exhausted_does_not_affect_other() {
    let limiter = make_limiter(3, 0.0, 1);
    let now = Instant::now();

    // Exhaust client-a
    for _ in 0..3 {
        limiter.check_and_consume("client-a", now);
    }
    assert!(!limiter.check_and_consume("client-a", now).0);

    // client-b should still have full burst
    let (allowed, _) = limiter.check_and_consume("client-b", now);
    assert!(allowed, "client-b should not be affected by client-a's exhaustion");
}

#[test]
fn per_client_isolation_replenishment_is_independent() {
    let limiter = make_limiter(3, 10.0, 1);
    let now = Instant::now();

    // Exhaust both clients
    for _ in 0..3 {
        limiter.check_and_consume("client-a", now);
        limiter.check_and_consume("client-b", now);
    }

    // Replenish for client-a only (by checking later)
    let later = now + Duration::from_secs(1);

    let (allowed_a, _) = limiter.check_and_consume("client-a", later);
    assert!(allowed_a, "client-a should replenish independently");

    let (allowed_b, _) = limiter.check_and_consume("client-b", later);
    assert!(allowed_b, "client-b should also replenish");
}

#[test]
fn per_client_many_clients_each_get_own_budget() {
    let limiter = make_limiter(2, 0.0, 1);
    let now = Instant::now();

    for i in 0..20 {
        let key = format!("client-{i}");
        assert!(limiter.check_and_consume(&key, now).0, "{key} first request");
        assert!(limiter.check_and_consume(&key, now).0, "{key} second request");
        assert!(!limiter.check_and_consume(&key, now).0, "{key} third should fail");
    }

    assert_eq!(limiter.key_count(), 20);
}
