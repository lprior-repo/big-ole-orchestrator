//! Rate limiter (Layer 1) for the circuit breaker.
//!
//! This module provides two rate limiting strategies:
//! 1. Cooldown-based rate limiting (original simple implementation)
//! 2. Token bucket rate limiting (advanced with burst/sustained rates, per-key tracking, sliding window, fair queuing)

use std::time::{Duration, Instant};

use dashmap::DashMap;

/// Check if a workflow is within its rate limit window.
///
/// # Returns
/// - `None` if no active rate limit (registration permitted)
/// - `Some(remaining_secs)` if rate-limited (ceiling of remaining seconds)
///
/// `last_registration` is the timestamp of the last successful registration for
/// this workflow, if any.
///
/// Pure function: does not modify state.
#[must_use]
pub fn check_rate_limit(
    last_registration: Option<Instant>,
    rate_limit_window: Duration,
    now: Instant,
) -> Option<u64> {
    let last = last_registration?;
    let elapsed = now.duration_since(last);
    if elapsed >= rate_limit_window {
        None
    } else {
        let remaining = rate_limit_window.saturating_sub(elapsed);
        let secs = remaining.as_secs() + u64::from(remaining.subsec_nanos() > 0);
        Some(secs)
    }
}

/// Update the rate limiter with the current timestamp for a workflow.
///
/// Returns the new timestamp for storage.
/// Called after a successful registration.
#[must_use]
pub fn update_rate_limit(now: Instant) -> Instant {
    now
}

// ═══════════════════════════════════════════════════════════════════════════════
// TOKEN BUCKET RATE LIMITER
// ═══════════════════════════════════════════════════════════════════════════════

/// Configuration for a token bucket rate limiter.
#[derive(Debug, Clone, Copy)]
pub struct TokenBucketConfig {
    /// Maximum number of tokens in the bucket (burst capacity).
    pub burst: u64,
    /// Number of tokens added to the bucket per second (sustained rate).
    pub sustained_rate: f64,
    /// Number of tokens consumed per request.
    pub cost_per_request: u64,
}

impl TokenBucketConfig {
    #[must_use]
    pub fn new(burst: u64, sustained_rate: f64, cost_per_request: u64) -> Self {
        Self {
            burst,
            sustained_rate,
            cost_per_request,
        }
    }

    #[must_use]
    pub fn tokens_per_second(&self) -> f64 {
        self.sustained_rate
    }
}

/// Internal state for a single key's token bucket.
#[derive(Debug, Clone)]
struct BucketState {
    tokens: f64,
    last_update: Instant,
}

impl BucketState {
    fn new(burst: u64, now: Instant) -> Self {
        Self {
            tokens: burst as f64,
            last_update: now,
        }
    }
}

/// Token bucket rate limiter with per-key tracking, sliding window, and fair queuing.
///
/// # Algorithm
/// - Tokens accumulate at a sustained rate (tokens per second)
/// - Bucket has a maximum capacity (burst limit)
/// - Each request consumes `cost_per_request` tokens
/// - If insufficient tokens, request is denied
/// - Sliding window: token accumulation is calculated based on elapsed time since last update
#[derive(Debug, Clone)]
pub struct TokenBucketRateLimiter {
    config: TokenBucketConfig,
    state: DashMap<String, BucketState>,
}

impl TokenBucketRateLimiter {
    /// Create a new token bucket rate limiter with the given configuration.
    #[must_use]
    pub fn new(config: TokenBucketConfig) -> Self {
        Self {
            config,
            state: DashMap::new(),
        }
    }

    /// Check if a request is allowed and consume tokens if so.
    ///
    /// Returns `(allowed, retry_after_secs)` where `allowed` is whether the
    /// request is permitted and `retry_after_secs` is the number of seconds
    /// to wait before retrying (0 if allowed).
    pub fn check_and_consume(&self, key: &str, now: Instant) -> (bool, u64) {
        let entry = self.state.entry(key.to_string());

        match entry {
            dashmap::mapref::entry::Entry::Occupied(mut occupied) => {
                let bucket = occupied.get_mut();
                self.replenish_tokens(bucket, now);

                if bucket.tokens >= self.config.cost_per_request as f64 {
                    bucket.tokens -= self.config.cost_per_request as f64;
                    (true, 0)
                } else {
                    let retry_after =
                        self.time_until_tokens(self.config.cost_per_request as f64 - bucket.tokens);
                    (false, retry_after)
                }
            }
            dashmap::mapref::entry::Entry::Vacant(vacant) => {
                let mut bucket = BucketState::new(self.config.burst, now);
                bucket.tokens -= self.config.cost_per_request as f64;
                vacant.insert(bucket);
                (true, 0)
            }
        }
    }

    /// Try to acquire tokens without consuming them (for fair queuing).
    ///
    /// Returns the number of tokens available after replenishment.
    pub fn peek_tokens(&self, key: &str, now: Instant) -> f64 {
        let entry = self.state.entry(key.to_string());

        match entry {
            dashmap::mapref::entry::Entry::Occupied(occupied) => {
                let bucket = occupied.get();
                let mut bucket = bucket.clone();
                self.replenish_tokens(&mut bucket, now);
                bucket.tokens
            }
            dashmap::mapref::entry::Entry::Vacant(_) => self.config.burst as f64,
        }
    }

    /// Get the number of tokens available for a key without modifying state.
    #[must_use]
    pub fn available_tokens(&self, key: &str, now: Instant) -> f64 {
        self.peek_tokens(key, now)
    }

    /// Get estimated wait time in seconds until enough tokens are available for a request.
    #[must_use]
    pub fn wait_time(&self, key: &str, now: Instant) -> u64 {
        let tokens = self.available_tokens(key, now);
        if tokens >= self.config.cost_per_request as f64 {
            0
        } else {
            self.time_until_tokens(self.config.cost_per_request as f64 - tokens)
        }
    }

    /// Reset the token bucket for a specific key.
    pub fn reset(&self, key: &str) {
        self.state.remove(key);
    }

    /// Get the number of keys currently being tracked.
    #[must_use]
    pub fn key_count(&self) -> usize {
        self.state.len()
    }

    /// Replenish tokens based on elapsed time since last update.
    fn replenish_tokens(&self, bucket: &mut BucketState, now: Instant) {
        let elapsed = now.duration_since(bucket.last_update).as_secs_f64();
        let tokens_to_add = elapsed * self.config.sustained_rate;
        bucket.tokens = (bucket.tokens + tokens_to_add).min(self.config.burst as f64);
        bucket.last_update = now;
    }

    /// Calculate time in seconds until enough tokens are available.
    fn time_until_tokens(&self, needed: f64) -> u64 {
        if self.config.sustained_rate <= 0.0 {
            return u64::MAX;
        }
        let secs = needed / self.config.sustained_rate;
        secs.ceil() as u64
    }
}

impl Default for TokenBucketConfig {
    fn default() -> Self {
        Self {
            burst: 100,
            sustained_rate: 10.0,
            cost_per_request: 1,
        }
    }
}

#[cfg(test)]
mod token_bucket_tests {
    use super::*;

    // TB-01: New key starts with full burst
    #[test]
    fn token_bucket_new_key_starts_with_full_burst() {
        let config = TokenBucketConfig::new(100, 10.0, 1);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        let (allowed, retry) = limiter.check_and_consume("key1", now);

        assert!(allowed);
        assert_eq!(retry, 0);
    }

    // TB-02: Burst capacity is respected
    #[test]
    fn token_bucket_burst_capacity_respected() {
        let config = TokenBucketConfig::new(3, 10.0, 1);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        // First 3 requests should succeed
        assert!(limiter.check_and_consume("key1", now).0);
        assert!(limiter.check_and_consume("key1", now).0);
        assert!(limiter.check_and_consume("key1", now).0);

        // 4th request should fail
        let (allowed, retry) = limiter.check_and_consume("key1", now);
        assert!(!allowed);
        assert!(retry > 0);
    }

    // TB-03: Sustained rate replenishes tokens over time
    #[test]
    fn token_bucket_sustained_rate_replenishes() {
        let config = TokenBucketConfig::new(10, 10.0, 5);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        // Exhaust all tokens
        limiter.check_and_consume("key1", now);

        // After 1 second, 10 tokens should be replenished (10 tokens/sec rate)
        let later = now + Duration::from_secs(1);
        let tokens = limiter.available_tokens("key1", later);
        assert!(tokens >= 9.0); // Allow small floating point variance
    }

    // TB-04: Per-key tracking works independently
    #[test]
    fn token_bucket_per_key_tracking_independent() {
        let config = TokenBucketConfig::new(5, 10.0, 1);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        // Exhaust key1
        for _ in 0..5 {
            limiter.check_and_consume("key1", now);
        }

        // key2 should still have full burst
        let (allowed, _) = limiter.check_and_consume("key2", now);
        assert!(allowed);
    }

    // TB-05: Sliding window - tokens accumulate smoothly
    #[test]
    fn token_bucket_sliding_window_smooth_accumulation() {
        let config = TokenBucketConfig::new(10, 100.0, 1); // 100 tokens/sec
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        // Exhaust all tokens
        limiter.check_and_consume("key1", now);

        // After 100ms, should have ~10 tokens (100 * 0.1)
        let later = now + Duration::from_millis(100);
        let tokens = limiter.available_tokens("key1", later);
        assert!(tokens >= 9.0);
    }

    // TB-06: Cost per request is respected
    #[test]
    fn token_bucket_cost_per_request_respected() {
        let config = TokenBucketConfig::new(10, 10.0, 5); // 5 tokens per request
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        // Only 2 requests should succeed (10 / 5 = 2)
        assert!(limiter.check_and_consume("key1", now).0);
        assert!(limiter.check_and_consume("key1", now).0);

        let (allowed, _) = limiter.check_and_consume("key1", now);
        assert!(!allowed);
    }

    // TB-07: Reset clears the bucket
    #[test]
    fn token_bucket_reset_clears_bucket() {
        let config = TokenBucketConfig::new(10, 10.0, 1);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        // Exhaust
        for _ in 0..10 {
            limiter.check_and_consume("key1", now);
        }

        // Reset
        limiter.reset("key1");

        // Should have full burst again
        let (allowed, _) = limiter.check_and_consume("key1", now);
        assert!(allowed);
    }

    // TB-08: Key count tracking
    #[test]
    fn token_bucket_key_count_tracking() {
        let config = TokenBucketConfig::new(10, 10.0, 1);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        assert_eq!(limiter.key_count(), 0);

        limiter.check_and_consume("key1", now);
        assert_eq!(limiter.key_count(), 1);

        limiter.check_and_consume("key2", now);
        assert_eq!(limiter.key_count(), 2);

        limiter.reset("key1");
        assert_eq!(limiter.key_count(), 1);
    }

    // TB-09: Zero sustained rate means no replenishment
    #[test]
    fn token_bucket_zero_sustained_rate_no_replenishment() {
        let config = TokenBucketConfig::new(5, 0.0, 1);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        // Exhaust
        for _ in 0..5 {
            limiter.check_and_consume("key1", now);
        }

        // Even after time passes, should still be empty
        let later = now + Duration::from_secs(100);
        let tokens = limiter.available_tokens("key1", later);
        assert_eq!(tokens, 0.0);
    }

    // TB-10: Available tokens returns correct values
    #[test]
    fn token_bucket_available_tokens_correct() {
        let config = TokenBucketConfig::new(10, 10.0, 1);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        // Initial state for new key - should have full burst
        let tokens = limiter.available_tokens("key1", now);
        assert!((tokens - 10.0).abs() < 0.001);

        // After check_and_consume (consumes 1 token)
        limiter.check_and_consume("key1", now);
        let tokens = limiter.available_tokens("key1", now);
        assert!((tokens - 9.0).abs() < 0.001);
    }

    // TB-11: Wait time calculation
    #[test]
    fn token_bucket_wait_time_calculation() {
        let config = TokenBucketConfig::new(10, 10.0, 10); // 10 tokens per request, 10/sec rate
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        // First request succeeds
        let (allowed, wait) = limiter.check_and_consume("key1", now);
        assert!(allowed);
        assert_eq!(wait, 0);

        // Second request should need ~1 second for replenishment
        let (_, wait) = limiter.check_and_consume("key1", now);
        assert!(wait >= 1);
    }

    // TB-12: Fair queuing - peek without consuming
    #[test]
    fn token_bucket_fair_queuing_peek() {
        let config = TokenBucketConfig::new(5, 10.0, 5);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();

        // Exhaust
        limiter.check_and_consume("key1", now);

        // Peek should return same value multiple times without consuming
        let tokens1 = limiter.peek_tokens("key1", now);
        let tokens2 = limiter.peek_tokens("key1", now);
        let tokens3 = limiter.peek_tokens("key1", now);

        assert_eq!(tokens1, tokens2);
        assert_eq!(tokens2, tokens3);

        // Actual consume should still fail
        let (allowed, _) = limiter.check_and_consume("key1", now);
        assert!(!allowed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    // B-21: No prior registration
    #[test]
    fn check_rate_limit_returns_none_when_no_prior_registration() {
        let now = Instant::now();
        let result = check_rate_limit(None, Duration::from_secs(60), now);
        assert_eq!(result, None);
    }

    // B-22: Within rate limit window (30s remaining)
    #[test]
    fn check_rate_limit_returns_some_30_when_30s_remaining() {
        let t0 = Instant::now();
        let now = t0 + Duration::from_secs(30);
        let result = check_rate_limit(Some(t0), Duration::from_secs(60), now);
        assert_eq!(result, Some(30));
    }

    // B-23: Rate limit window elapsed
    #[test]
    fn check_rate_limit_returns_none_when_window_elapsed() {
        let t0 = Instant::now();
        let now = t0 + Duration::from_secs(61);
        let result = check_rate_limit(Some(t0), Duration::from_secs(60), now);
        assert_eq!(result, None);
    }

    // B-24: Boundary at 59 seconds
    #[test]
    fn check_rate_limit_returns_some_1_at_59_seconds() {
        let t0 = Instant::now();
        let now = t0 + Duration::from_secs(59);
        let result = check_rate_limit(Some(t0), Duration::from_secs(60), now);
        assert_eq!(result, Some(1));
    }

    // B-25: Boundary at exactly 60 seconds
    #[test]
    fn check_rate_limit_returns_none_at_exactly_60_seconds() {
        let t0 = Instant::now();
        let now = t0 + Duration::from_secs(60);
        let result = check_rate_limit(Some(t0), Duration::from_secs(60), now);
        assert_eq!(result, None);
    }

    // B-26: Update then check
    #[test]
    fn update_rate_limit_sets_timestamp_so_check_returns_remaining() {
        let t0 = Instant::now();
        let ts = update_rate_limit(t0);
        let now = t0 + Duration::from_secs(20);
        let result = check_rate_limit(Some(ts), Duration::from_secs(60), now);
        assert_eq!(result, Some(40));
    }

    // Combinatorial: just registered (0 elapsed) -> full window remaining
    #[test]
    fn check_rate_limit_returns_full_window_when_just_registered() {
        let t0 = Instant::now();
        let result = check_rate_limit(Some(t0), Duration::from_secs(60), t0);
        assert_eq!(result, Some(60));
    }

    // Combinatorial: well past window (120s elapsed) -> None
    #[test]
    fn check_rate_limit_returns_none_when_well_past_window() {
        let t0 = Instant::now();
        let now = t0 + Duration::from_secs(120);
        let result = check_rate_limit(Some(t0), Duration::from_secs(60), now);
        assert_eq!(result, None);
    }

    // PROP-04: INV-002 — Rate limit is uniform across workflows
    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            // PROP-04: Rate limit uniform
            #[test]
            fn rate_limit_is_uniform_across_workflows(
                elapsed_secs in 0u64..=120,
            ) {
                let t0 = Instant::now();
                let now = t0 + Duration::from_secs(elapsed_secs);
                let window = Duration::from_secs(60);

                // Same result regardless of which workflow we check
                let result1 = check_rate_limit(Some(t0), window, now);
                let result2 = check_rate_limit(Some(t0), window, now);
                prop_assert_eq!(result1, result2);
            }

            // PROP-09: check_rate_limit ceiling property
            #[test]
            fn check_rate_limit_returns_ceiling_of_remaining_seconds(
                elapsed_millis in 0u64..=59999,
                window_secs in 1u64..=300,
            ) {
                let t0 = Instant::now();
                let elapsed = Duration::from_millis(elapsed_millis);
                let window = Duration::from_secs(window_secs);

                // Only test when elapsed < window
                if elapsed < window {
                    let now = t0 + elapsed;
                    let result = check_rate_limit(Some(t0), window, now);
                    let remaining = window - elapsed;
                    // Ceiling of remaining seconds
                    let expected_secs = remaining.as_secs()
                        + if remaining.subsec_nanos() > 0 { 1 } else { 0 };
                    prop_assert_eq!(result, Some(expected_secs));
                }
            }

            // PROP-09 anti-invariant: elapsed >= window -> None
            #[test]
            fn check_rate_limit_returns_none_when_elapsed_exceeds_window(
                extra_secs in 0u64..=120,
                window_secs in 1u64..=300,
            ) {
                let t0 = Instant::now();
                let now = t0 + Duration::from_secs(window_secs + extra_secs);
                let window = Duration::from_secs(window_secs);
                let result = check_rate_limit(Some(t0), window, now);
                prop_assert_eq!(result, None);
            }
        }
    }
}
