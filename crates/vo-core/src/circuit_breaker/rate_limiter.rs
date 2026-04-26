//! Rate limiter (Layer 1) for the circuit breaker.
//!
//! This module provides two rate limiting strategies:
//! 1. Cooldown-based rate limiting (simple time-window implementation)
//! 2. Token bucket rate limiting (advanced with burst/sustained rates, per-key tracking)
//!
//! Each strategy lives in its own submodule; this file re-exports the public API.

#[path = "cooldown.rs"]
mod cooldown;

#[path = "token_bucket.rs"]
mod token_bucket;

pub use cooldown::{check_rate_limit, update_rate_limit};
pub use token_bucket::{TokenBucketConfig, TokenBucketRateLimiter};
