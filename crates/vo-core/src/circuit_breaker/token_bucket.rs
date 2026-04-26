//! Token bucket rate limiter (Layer 1b) for the circuit breaker.
//!
//! Advanced rate limiting with burst capacity, sustained refill rate,
//! per-key tracking, sliding window replenishment, and fair queuing.

use std::time::Instant;

use dashmap::DashMap;

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

impl Default for TokenBucketConfig {
    fn default() -> Self {
        Self {
            burst: 100,
            sustained_rate: 10.0,
            cost_per_request: 1,
        }
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

#[cfg(test)]
#[path = "token_bucket_tests.rs"]
mod token_bucket_tests;
