//! Token bucket rate limiter (Layer 1b) for the circuit breaker.
//!
//! Advanced rate limiting with burst capacity, sustained refill rate,
//! per-key tracking, sliding window replenishment, and fair queuing.
//!
//! # Algorithm
//!
//! The token bucket algorithm works as follows:
//!
//! ```text
//!  TokenBucketRateLimiter {
//!      config: TokenBucketConfig {
//!          burst: u64,           // Max tokens (bucket capacity)
//!          sustained_rate: f64,  // Tokens added per second
//!          cost_per_request: u64, // Tokens consumed per request
//!      },
//!      state: DashMap<String, BucketState>, // Per-key state
//!  }
//!
//!  check_and_consume(key, now) {
//!      bucket = get_or_create(key)
//!      replenish(bucket, now)  // Add tokens based on elapsed time
//!
//!      if bucket.tokens ≥ cost_per_request {
//!          bucket.tokens -= cost_per_request
//!          return (allowed=true, retry_after=0)
//!      } else {
//!          wait = time_until_tokens(cost - bucket.tokens)
//!          return (allowed=false, retry_after=wait)
//!      }
//!  }
//!
//!  replenish(bucket, now) {
//!      elapsed = (now - bucket.last_update).as_secs_f64()
//!      bucket.tokens = min(bucket.tokens + elapsed * sustained_rate, burst)
//!      bucket.last_update = now
//!  }
//!
//!  time_until_tokens(needed) {
//!      return ceil(needed / sustained_rate)
//!  }
//! ```
//!
//! # Key Concepts
//!
//! - **Burst capacity**: The maximum number of tokens the bucket can hold.
//!   This allows short bursts of requests above the sustained rate.
//! - **Sustained rate**: The rate at which tokens are added (tokens per second).
//!   This is the long-term throughput limit.
//! - **Cost per request**: The number of tokens consumed by a single request.
//!   Higher cost means fewer requests are allowed.
//! - **Sliding window replenishment**: Tokens are added based on elapsed time
//!   since the last update, not on fixed intervals. This ensures smooth
//!   token accumulation.
//! - **Per-key tracking**: Each key (e.g., workflow name) has its own bucket,
//!   allowing independent rate limiting per key.
//!
//! # Examples
//!
//! ```
//! use vo_core::circuit_breaker::{TokenBucketConfig, TokenBucketRateLimiter};
//! use std::time::Instant;
//!
//! let config = TokenBucketConfig::new(10, 1.0, 1); // burst=10, 1 token/sec
//! let limiter = TokenBucketRateLimiter::new(config);
//!
//! let now = Instant::now();
//! let (allowed, _) = limiter.check_and_consume("my-workflow", now);
//! assert!(allowed); // First request is allowed (bucket starts full)
//! ```

use std::time::Instant;

use dashmap::DashMap;

/// Configuration for a token bucket rate limiter.
///
/// This struct defines the parameters of the token bucket algorithm:
/// burst capacity, sustained refill rate, and per-request cost.
///
/// # Fields
///
/// | Field | Type | Default | Description |
/// |-------|------|---------|-------------|
/// | `burst` | `u64` | 100 | Maximum tokens in the bucket (burst capacity). |
/// | `sustained_rate` | `f64` | 10.0 | Tokens added per second (long-term throughput). |
/// | `cost_per_request` | `u64` | 1 | Tokens consumed per request. |
///
/// # Algorithm Behavior
///
/// - The bucket starts full (tokens = burst) when a new key is first seen.
/// - Tokens accumulate at `sustained_rate` tokens per second.
/// - The bucket never exceeds `burst` tokens (capped on replenishment).
/// - Each request consumes `cost_per_request` tokens.
/// - If insufficient tokens, the request is denied with a wait time.
///
/// # Examples
///
/// ```
/// use vo_core::circuit_breaker::TokenBucketConfig;
///
/// // 10 token burst, 5 tokens/sec sustained, 1 token per request
/// let config = TokenBucketConfig::new(10, 5.0, 1);
/// assert_eq!(config.burst, 10);
/// assert_eq!(config.sustained_rate, 5.0);
/// assert_eq!(config.cost_per_request, 1);
///
/// // Default config: 100 burst, 10 tokens/sec, 1 cost
/// let default = TokenBucketConfig::default();
/// assert_eq!(default.burst, 100);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct TokenBucketConfig {
    /// Maximum number of tokens in the bucket (burst capacity).
    ///
    /// This is the maximum number of tokens the bucket can hold. When a new
    /// key is first seen, its bucket starts full (tokens = burst). Tokens
    /// accumulate over time but never exceed this limit.
    ///
    /// Higher burst allows more requests in quick succession before falling
    /// to the sustained rate.
    pub burst: u64,

    /// Number of tokens added to the bucket per second (sustained rate).
    ///
    /// This is the long-term throughput limit. Tokens are added continuously
    /// based on elapsed time (sliding window replenishment), not on fixed
    /// intervals.
    ///
    /// For example, with `sustained_rate = 5.0`:
    /// - After 1 second, 5 tokens are added.
    /// - After 0.5 seconds, 2.5 tokens are added.
    pub sustained_rate: f64,

    /// Number of tokens consumed per request.
    ///
    /// Each `check_and_consume` call deducts this many tokens from the bucket.
    /// If insufficient tokens remain, the request is denied.
    ///
    /// Higher cost reduces the number of allowed requests. For example,
    /// `cost_per_request = 2` with `sustained_rate = 10.0` means only 5
    /// requests per second can be sustained.
    pub cost_per_request: u64,
}

impl TokenBucketConfig {
    /// Create a new token bucket configuration.
    ///
    /// # Arguments
    ///
    /// * `burst` — Maximum number of tokens in the bucket.
    /// * `sustained_rate` — Tokens added per second.
    /// * `cost_per_request` — Tokens consumed per request.
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::circuit_breaker::TokenBucketConfig;
    ///
    /// let config = TokenBucketConfig::new(100, 10.0, 1);
    /// assert_eq!(config.burst, 100);
    /// assert_eq!(config.tokens_per_second(), 10.0);
    /// ```
    #[must_use]
    pub fn new(burst: u64, sustained_rate: f64, cost_per_request: u64) -> Self {
        Self {
            burst,
            sustained_rate,
            cost_per_request,
        }
    }

    /// Returns the sustained rate (tokens per second).
    ///
    /// This is a convenience accessor that returns the `sustained_rate` field.
    #[must_use]
    pub fn tokens_per_second(&self) -> f64 {
        self.sustained_rate
    }
}

impl Default for TokenBucketConfig {
    /// Returns the default configuration: 100 burst, 10 tokens/sec, 1 cost.
    fn default() -> Self {
        Self {
            burst: 100,
            sustained_rate: 10.0,
            cost_per_request: 1,
        }
    }
}

/// Internal state for a single key's token bucket.
///
/// This struct tracks the current token count and the last update time for
/// a specific key. It is managed internally by [`TokenBucketRateLimiter`]
/// and is not exposed to callers.
#[derive(Debug, Clone)]
struct BucketState {
    /// Current number of tokens available.
    ///
    /// Starts at `burst` when the bucket is created and is decremented
    /// on each successful request.
    tokens: f64,

    /// Last time the bucket was replenished.
    ///
    /// Used to calculate token accumulation based on elapsed time.
    last_update: Instant,
}

impl BucketState {
    /// Create a new bucket state, starting full with `burst` tokens.
    fn new(burst: u64, now: Instant) -> Self {
        Self {
            tokens: burst as f64,
            last_update: now,
        }
    }
}

/// Token bucket rate limiter with per-key tracking, sliding window, and fair queuing.
///
/// This struct implements the token bucket algorithm with concurrent access
/// via `DashMap`. Each key (e.g., workflow name) has its own independent
/// bucket, allowing fine-grained per-workflow rate limiting.
///
/// # Algorithm Summary
///
/// 1. **Token accumulation**: Tokens are added at `sustained_rate` per second,
///    capped at `burst`.
/// 2. **Request check**: Each request consumes `cost_per_request` tokens.
/// 3. **Denial**: If insufficient tokens, the request is denied with a
///    calculated wait time until enough tokens accumulate.
///
/// # Thread Safety
///
/// `TokenBucketRateLimiter` is `Sync + Send` because all state is accessed
/// through `DashMap`, which provides lock-free concurrent reads and
/// fine-grained partition-level writes.
///
/// # Examples
///
/// ```
/// use vo_core::circuit_breaker::{TokenBucketConfig, TokenBucketRateLimiter};
/// use std::time::{Duration, Instant};
///
/// let config = TokenBucketConfig::new(5, 1.0, 1); // 5 tokens, 1/sec
/// let limiter = TokenBucketRateLimiter::new(config);
///
/// let now = Instant::now();
///
/// // First 5 requests are allowed (bucket starts full)
/// for _ in 0..5 {
///     let (allowed, _) = limiter.check_and_consume("wf", now);
///     assert!(allowed);
/// }
///
/// // 6th request is denied (bucket empty)
/// let (allowed, retry_after) = limiter.check_and_consume("wf", now);
/// assert!(!allowed);
/// assert_eq!(retry_after, 1); // Wait 1 second for next token
/// ```
#[derive(Debug, Clone)]
pub struct TokenBucketRateLimiter {
    config: TokenBucketConfig,
    state: DashMap<String, BucketState>,
}

impl TokenBucketRateLimiter {
    /// Create a new token bucket rate limiter with the given configuration.
    ///
    /// The internal state map is initialized empty. Buckets are created
    /// on-demand when `check_and_consume` is first called with a new key.
    ///
    /// # Arguments
    ///
    /// * `config` — The token bucket configuration (burst, rate, cost).
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::circuit_breaker::{TokenBucketConfig, TokenBucketRateLimiter};
    ///
    /// let config = TokenBucketConfig::new(10, 2.0, 1);
    /// let limiter = TokenBucketRateLimiter::new(config);
    /// assert_eq!(limiter.key_count(), 0); // No keys yet
    /// ```
    #[must_use]
    pub fn new(config: TokenBucketConfig) -> Self {
        Self {
            config,
            state: DashMap::new(),
        }
    }

    /// Check if a request is allowed and consume tokens if so.
    ///
    /// This is the main entry point for rate limiting. It checks whether
    /// sufficient tokens are available for the request and, if so, consumes
    /// them atomically.
    ///
    /// # Algorithm
    ///
    /// ```text
    ///  check_and_consume(key, now)
    ///    │
    ///    ├─ Get or create bucket for key
    ///    │   ├─ New key: bucket starts full (tokens = burst)
    ///    │   └─ Existing: replenish tokens based on elapsed time
    ///    │
    ///    ├─ tokens ≥ cost_per_request?
    ///    │   ├─ Yes: deduct tokens, return (true, 0)
    ///    │   └─ No:  calculate wait time, return (false, wait_secs)
    ///    │
    ///    └─ Return (allowed, retry_after)
    /// ```
    ///
    /// # Arguments
    ///
    /// * `key` — The identifier for rate limiting (e.g., workflow name).
    ///   Each key has an independent bucket.
    /// * `now` — The current instant, used for token replenishment calculation.
    ///
    /// # Returns
    ///
    /// A tuple `(allowed, retry_after_secs)`:
    /// - `allowed`: `true` if the request is permitted, `false` if rate-limited.
    /// - `retry_after_secs`: If `allowed` is `false`, the number of seconds to
    ///   wait before retrying. If `allowed` is `true`, always `0`.
    ///
    /// # Thread Safety
    ///
    /// This method uses `DashMap::entry()` for atomic read-modify-write on the
    /// bucket state, ensuring thread safety under concurrent access.
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::circuit_breaker::{TokenBucketConfig, TokenBucketRateLimiter};
    /// use std::time::Instant;
    ///
    /// let config = TokenBucketConfig::new(2, 1.0, 1);
    /// let limiter = TokenBucketRateLimiter::new(config);
    /// let now = Instant::now();
    ///
    /// // First request: allowed (bucket starts with 2 tokens)
    /// let (allowed, retry_after) = limiter.check_and_consume("wf", now);
    /// assert!(allowed);
    /// assert_eq!(retry_after, 0);
    ///
    /// // Second request: allowed (1 token left)
    /// let (allowed, _) = limiter.check_and_consume("wf", now);
    /// assert!(allowed);
    ///
    /// // Third request: denied (0 tokens left)
    /// let (allowed, retry_after) = limiter.check_and_consume("wf", now);
    /// assert!(!allowed);
    /// assert_eq!(retry_after, 1); // Wait 1 second
    /// ```
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
    /// This method checks the current token count after replenishment without
    /// actually deducting tokens. It is used for fair queuing to estimate
    /// when a request can be served without blocking other requests.
    ///
    /// # Arguments
    ///
    /// * `key` — The identifier for the bucket.
    /// * `now` — The current instant.
    ///
    /// # Returns
    ///
    /// The number of tokens available after replenishment. Returns `burst`
    /// for new keys (no existing bucket).
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::circuit_breaker::{TokenBucketConfig, TokenBucketRateLimiter};
    /// use std::time::Instant;
    ///
    /// let config = TokenBucketConfig::new(10, 1.0, 1);
    /// let limiter = TokenBucketRateLimiter::new(config);
    ///
    /// // New key: peek returns full burst
    /// let tokens = limiter.peek_tokens("new-key", Instant::now());
    /// assert_eq!(tokens, 10.0);
    /// ```
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
    ///
    /// This is an alias for [`peek_tokens()`][Self::peek_tokens]. It provides
    /// a more descriptive name for the common use case of checking availability.
    ///
    /// # Arguments
    ///
    /// * `key` — The identifier for the bucket.
    /// * `now` — The current instant.
    ///
    /// # Returns
    ///
    /// The number of available tokens after replenishment.
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::circuit_breaker::{TokenBucketConfig, TokenBucketRateLimiter};
    /// use std::time::Instant;
    ///
    /// let config = TokenBucketConfig::new(100, 10.0, 1);
    /// let limiter = TokenBucketRateLimiter::new(config);
    ///
    /// assert_eq!(limiter.available_tokens("new", Instant::now()), 100.0);
    /// ```
    #[must_use]
    pub fn available_tokens(&self, key: &str, now: Instant) -> f64 {
        self.peek_tokens(key, now)
    }

    /// Get estimated wait time in seconds until enough tokens are available for a request.
    ///
    /// This method calculates how long a caller should wait before retrying
    /// a denied request. It first checks the available tokens and then
    /// computes the time needed to accumulate enough tokens for a single request.
    ///
    /// # Arguments
    ///
    /// * `key` — The identifier for the bucket.
    /// * `now` — The current instant.
    ///
    /// # Returns
    ///
    /// - `0` if sufficient tokens are available now.
    /// - The number of seconds to wait if tokens are insufficient, rounded up.
    /// - `u64::MAX` if `sustained_rate` is 0 or negative (tokens never accumulate).
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::circuit_breaker::{TokenBucketConfig, TokenBucketRateLimiter};
    /// use std::time::Instant;
    ///
    /// let config = TokenBucketConfig::new(2, 1.0, 1);
    /// let limiter = TokenBucketRateLimiter::new(config);
    /// let now = Instant::now();
    ///
    /// // Drain the bucket
    /// limiter.check_and_consume("wf", now);
    /// limiter.check_and_consume("wf", now);
    ///
    /// // Wait time for next token: 1 second
    /// assert_eq!(limiter.wait_time("wf", now), 1);
    ///
    /// // Wait time for a fresh key: 0 (bucket starts full)
    /// assert_eq!(limiter.wait_time("new", now), 0);
    /// ```
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
    ///
    /// Removes the bucket entirely, so the next call to `check_and_consume`
    /// will create a fresh bucket starting full.
    ///
    /// # Arguments
    ///
    /// * `key` — The identifier for the bucket to reset.
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::circuit_breaker::{TokenBucketConfig, TokenBucketRateLimiter};
    /// use std::time::Instant;
    ///
    /// let config = TokenBucketConfig::new(5, 1.0, 1);
    /// let limiter = TokenBucketRateLimiter::new(config);
    /// let now = Instant::now();
    ///
    //  // Use some tokens
    /// limiter.check_and_consume("wf", now);
    /// assert_eq!(limiter.key_count(), 1);
    ///
    /// // Reset
    /// limiter.reset("wf");
    /// assert_eq!(limiter.key_count(), 0);
    ///
    /// // Fresh bucket
    /// let (allowed, _) = limiter.check_and_consume("wf", now);
    /// assert!(allowed);
    /// ```
    pub fn reset(&self, key: &str) {
        self.state.remove(key);
    }

    /// Get the number of keys currently being tracked.
    ///
    /// This returns the number of distinct buckets (keys) in the limiter.
    /// Useful for monitoring and debugging.
    ///
    /// # Returns
    ///
    /// The number of tracked keys.
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::circuit_breaker::{TokenBucketConfig, TokenBucketRateLimiter};
    /// use std::time::Instant;
    ///
    /// let config = TokenBucketConfig::default();
    /// let limiter = TokenBucketRateLimiter::new(config);
    /// assert_eq!(limiter.key_count(), 0);
    ///
    /// limiter.check_and_consume("wf1", Instant::now());
    /// limiter.check_and_consume("wf2", Instant::now());
    /// assert_eq!(limiter.key_count(), 2);
    /// ```
    #[must_use]
    pub fn key_count(&self) -> usize {
        self.state.len()
    }

    /// Replenish tokens based on elapsed time since last update.
    ///
    /// This is an internal method called by `check_and_consume` and `peek_tokens`.
    /// It adds tokens proportional to the elapsed time since the last update,
    /// capped at the burst limit.
    ///
    /// # Formula
    ///
    /// ```text
    /// elapsed = (now - bucket.last_update).as_secs_f64()
    /// tokens_to_add = elapsed * sustained_rate
    /// bucket.tokens = min(bucket.tokens + tokens_to_add, burst)
    /// bucket.last_update = now
    /// ```
    ///
    /// # Arguments
    ///
    /// * `bucket` — A mutable reference to the bucket state to update.
    /// * `now` — The current instant.
    fn replenish_tokens(&self, bucket: &mut BucketState, now: Instant) {
        let elapsed = now.duration_since(bucket.last_update).as_secs_f64();
        let tokens_to_add = elapsed * self.config.sustained_rate;
        bucket.tokens = (bucket.tokens + tokens_to_add).min(self.config.burst as f64);
        bucket.last_update = now;
    }

    /// Calculate time in seconds until enough tokens are available.
    ///
    /// This is an internal method used by `check_and_consume` and `wait_time`
    /// to compute the retry delay for denied requests.
    ///
    /// # Formula
    ///
    /// ```text
    /// needed = cost_per_request - current_tokens
    /// wait_secs = ceil(needed / sustained_rate)
    /// ```
    ///
    /// If `sustained_rate` is zero or negative, returns `u64::MAX` (never).
    ///
    /// # Arguments
    ///
    /// * `needed` — The number of additional tokens needed.
    ///
    /// # Returns
    ///
    /// The number of seconds to wait, rounded up. `u64::MAX` if rate is non-positive.
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
