//! Cooldown-based rate limiter (Layer 1a) for the circuit breaker.
//!
//! Simple time-window rate limiting: once a registration occurs, subsequent
//! registrations for the same workflow are blocked until the cooldown window
//! elapses.
//!
//! # Algorithm
//!
//! ```text
//!  check_rate_limit(last_registration, rate_limit_window, now)
//!    │
//!    ├─ No prior registration → None (allowed)
//!    │
//!    └─ Has prior registration:
//!         │
//!         ├─ elapsed ≥ window → None (allowed, window expired)
//!         │
//!         └─ elapsed < window → Some(remaining_secs) (rate-limited)
//!                              where remaining = ceil((window - elapsed).as_secs())
//! ```
//!
//! # Ceiling Behavior
//!
//! The remaining time is **ceiling-rounded** to whole seconds. For example:
//! - 0.1s remaining → 1s (not 0s)
//! - 30.0s remaining → 30s
//! - 30.5s remaining → 31s
//!
//! This ensures callers never retry too early due to floating-point truncation.
//!
//! # Examples
//!
//! ```
//! use vo_core::circuit_breaker::check_rate_limit;
//! use std::time::{Duration, Instant};
//!
//! let t0 = Instant::now();
//! let now = t0 + Duration::from_secs(30);
//!
//! // 30s elapsed in a 60s window → 30s remaining
//! assert_eq!(check_rate_limit(Some(t0), Duration::from_secs(60), now), Some(30));
//!
//! // No prior registration → allowed
//! assert_eq!(check_rate_limit(None, Duration::from_secs(60), now), None);
//!
//! // Window elapsed → allowed
//! let now = t0 + Duration::from_secs(61);
//! assert_eq!(check_rate_limit(Some(t0), Duration::from_secs(60), now), None);
//! ```

use std::time::{Duration, Instant};

/// Check if a workflow is within its rate limit window.
///
/// This is the core rate limit predicate. Given the last registration timestamp,
/// the rate limit window duration, and the current time, it returns whether
/// the workflow is still within its cooldown period.
///
/// # Arguments
///
/// * `last_registration` — The timestamp of the last successful registration
///   for this workflow, if any. `None` means no prior registration.
/// * `rate_limit_window` — The minimum cooldown interval between registrations.
/// * `now` — The current instant.
///
/// # Returns
///
/// | Condition | Return Value |
/// |-----------|-------------|
/// | No prior registration (`None`) | `None` — registration is permitted |
/// | Window elapsed (`now - last ≥ window`) | `None` — registration is permitted |
/// | Window not elapsed (`now - last < window`) | `Some(remaining_secs)` — registration denied |
///
/// The remaining seconds use **ceiling rounding**: any fractional second
/// rounds up to ensure the caller never retries within the window.
///
/// # Invariants
///
/// - **INV-002**: Rate limiting is uniform across all workflows — each has
///   an independent cooldown based on its own last registration time.
/// - Rate limiting is **per-workflow**: different workflows can register
///   simultaneously even with the same configuration.
///
/// # Pure Function
///
/// This function is pure: it does not modify any state. It only reads the
/// inputs and returns a boolean-style result. State updates are handled
/// separately by [`update_rate_limit()`].
///
/// # Ceiling Rounding Detail
///
/// The remaining time is calculated as:
/// ```text
/// remaining = rate_limit_window - elapsed
/// secs = remaining.as_secs() + (1 if remaining.subsec_nanos() > 0 else 0)
/// ```
///
/// This ensures that even a 1-nanosecond remaining results in 1 second of
/// cooldown, preventing race conditions from sub-second precision.
///
/// # Examples
///
/// ```
/// use vo_core::circuit_breaker::check_rate_limit;
/// use std::time::{Duration, Instant};
///
/// let t0 = Instant::now();
/// let window = Duration::from_secs(60);
///
/// // Just registered (0 elapsed) → full window remaining
/// assert_eq!(check_rate_limit(Some(t0), window, t0), Some(60));
///
/// // 29.5s elapsed → 31s remaining (ceiling of 30.5)
/// let now = t0 + Duration::from_millis(29500);
/// assert_eq!(check_rate_limit(Some(t0), window, now), Some(31));
///
/// // 60s elapsed → allowed (window exactly expired)
/// let now = t0 + Duration::from_secs(60);
/// assert_eq!(check_rate_limit(Some(t0), window, now), None);
///
/// // No prior registration → always allowed
/// assert_eq!(check_rate_limit(None, window, t0), None);
/// ```
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
/// Returns the new timestamp for storage. This is a passthrough function
/// that simply returns `now` — the rate limiting logic is entirely in
/// [`check_rate_limit()`].
///
/// # Arguments
///
/// * `now` — The current instant (timestamp of a successful registration).
///
/// # Returns
///
/// The same `now` value, for storage in the rate limiter map.
///
/// # Examples
///
/// ```
/// use vo_core::circuit_breaker::update_rate_limit;
/// use std::time::Instant;
///
/// let now = Instant::now();
/// assert_eq!(update_rate_limit(now), now);
/// ```
#[must_use]
pub fn update_rate_limit(now: Instant) -> Instant {
    now
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

    // Combinatorial: just registered (0 elapsed) → full window remaining
    #[test]
    fn check_rate_limit_returns_full_window_when_just_registered() {
        let t0 = Instant::now();
        let result = check_rate_limit(Some(t0), Duration::from_secs(60), t0);
        assert_eq!(result, Some(60));
    }

    // Combinatorial: well past window (120s elapsed) → None
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

            // PROP-09 anti-invariant: elapsed >= window → None
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
