//! Rate limiter (Layer 1) for the circuit breaker.
//!
//! This module provides two rate limiting strategies:
//! 1. Cooldown-based rate limiting (simple per-workflow window)
//! 2. Token bucket rate limiting (advanced with burst/sustained rates,
//!    per-key tracking, sliding window, and fair queuing)

use std::time::{Duration, Instant};

// Re-export token bucket types for backward-compatible consumers
pub use self::token_bucket::{TokenBucketConfig, TokenBucketRateLimiter};

mod token_bucket;

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

#[cfg(test)]
mod tests {
    use super::*;

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

    // PROP-04: INV-002 - Rate limit is uniform across workflows
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
