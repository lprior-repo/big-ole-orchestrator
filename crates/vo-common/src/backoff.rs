//! Exponential backoff policy with configurable max delay cap.

use std::time::Duration;

/// Configurable exponential backoff strategy.
///
/// Computes delays as `base_delay * (multiplier ^ iteration)`, capped at `max_delay`.
#[derive(Debug, Clone)]
pub struct BackoffPolicy {
    base_delay: Duration,
    max_delay: Duration,
    multiplier: f64,
}

impl BackoffPolicy {
    /// Create a new backoff policy.
    ///
    /// # Panics
    /// Panics if `base_delay` is zero or `max_delay` is less than `base_delay`.
    pub fn new(base_delay: Duration, max_delay: Duration, multiplier: f64) -> Self {
        assert!(base_delay.as_millis() > 0, "base_delay must be > 0");
        assert!(
            max_delay >= base_delay,
            "max_delay must be >= base_delay"
        );
        assert!(multiplier > 1.0, "multiplier must be > 1.0");
        Self {
            base_delay,
            max_delay,
            multiplier,
        }
    }

    /// Compute the delay for the given iteration count.
    ///
    /// The delay is `base_delay * multiplier^iteration`, capped at `max_delay`.
    /// The result is never greater than `max_delay`.
    pub fn delay(&self, iteration: u32) -> Duration {
        let delay = if iteration == 0 {
            self.base_delay.as_secs_f64()
        } else {
            self.base_delay.as_secs_f64() * self.multiplier.powi(iteration as i32)
        };
        let capped = delay.min(self.max_delay.as_secs_f64());
        Duration::from_secs_f64(capped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify delay(0) returns base_delay exactly.
    #[test]
    fn delay_zero_returns_base() {
        let policy = BackoffPolicy::new(Duration::from_millis(100), Duration::from_secs(5), 2.0);
        assert_eq!(policy.delay(0), Duration::from_millis(100));
    }

    /// Verify delay grows with iteration when below max.
    #[test]
    fn delay_grows_with_iteration() {
        let policy = BackoffPolicy::new(Duration::from_millis(100), Duration::from_secs(5), 2.0);
        for i in 1..u32::MAX {
            assert!(
                policy.delay(i) >= policy.delay(i - 1),
                "delay should be non-decreasing at iteration {}",
                i
            );
        }
    }

    /// TEST: BackoffPolicy max delay cap.
    ///
    /// Create a BackoffPolicy with a low max_delay (5 seconds), call delay()
    /// for iteration counts 0 through 200, assert every returned duration is
    /// <= max_delay, and confirm the delay eventually plateaus at max_delay
    /// and stays there for all subsequent iterations.
    #[test]
    fn max_delay_cap_never_exceeded() {
        let policy = BackoffPolicy::new(Duration::from_millis(100), Duration::from_secs(5), 2.0);
        let max_delay = policy.max_delay;

        // Check 0..=200 iterations
        for i in 0..=200 {
            let d = policy.delay(i);
            assert!(
                d <= max_delay,
                "iteration {}: delay {:?} exceeds max_delay {:?}",
                i, d, max_delay
            );
        }

        // Confirm plateau: find first iteration where delay == max_delay
        let plateau_iter = (0..=200u32)
            .find(|&i| policy.delay(i) == max_delay)
            .expect("delay should plateau at max_delay within 0..=200");

        // All iterations from plateau point onward must equal max_delay
        for i in (plateau_iter + 1)..=200 {
            assert_eq!(
                policy.delay(i),
                max_delay,
                "iteration {} should have plateaued at max_delay",
                i
            );
        }

        // Also verify beyond 200 to be thorough
        for i in 201..=500 {
            let d = policy.delay(i);
            assert!(
                d <= max_delay,
                "iteration {}: delay {:?} exceeds max_delay {:?}",
                i, d, max_delay
            );
            assert_eq!(
                d, max_delay,
                "iteration {} should equal max_delay after plateau",
                i
            );
        }
    }

    /// Edge case: max_delay equals base_delay — every iteration returns base_delay.
    #[test]
    fn max_equals_base_returns_base_always() {
        let d = Duration::from_secs(1);
        let policy = BackoffPolicy::new(d, d, 2.0);
        for i in 0..=100u32 {
            assert_eq!(policy.delay(i), d);
        }
    }

    /// Edge case: very large multiplier — hits max_delay at iteration 1.
    #[test]
    fn large_multiplier_hits_max_immediately() {
        let policy = BackoffPolicy::new(Duration::from_millis(1), Duration::from_millis(10), 100.0);
        assert_eq!(policy.delay(0), Duration::from_millis(1));
        assert_eq!(policy.delay(1), Duration::from_millis(10));
        for i in 2..=200 {
            assert_eq!(policy.delay(i), Duration::from_millis(10));
        }
    }
}
