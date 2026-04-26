//! Cooldown tracking for rate-limited workflows.
//!
//! Provides per-workflow cooldown state tracking used by the token bucket
//! rate limiter for backoff after rate limit events.

use std::time::Instant;

/// Cooldown state for a single rate-limited entity.
#[derive(Debug, Clone)]
pub struct CooldownState {
    last_rejected_at: Option<Instant>,
    cooldown_duration_secs: u64,
}

impl CooldownState {
    /// Create a new cooldown state with the given duration in seconds.
    #[must_use]
    pub fn new(cooldown_duration_secs: u64) -> Self {
        Self {
            last_rejected_at: None,
            cooldown_duration_secs,
        }
    }

    /// Record a rejection event at the given time.
    pub fn record_rejection(&mut self, now: Instant) {
        self.last_rejected_at = Some(now);
    }

    /// Check whether the entity is still in cooldown at the given time.
    #[must_use]
    pub fn is_cooled_down(&self, now: Instant) -> bool {
        match self.last_rejected_at {
            None => true,
            Some(rejected_at) => {
                now.duration_since(rejected_at).as_secs() >= self.cooldown_duration_secs
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn new_cooldown_is_cooled_down() {
        let state = CooldownState::new(60);
        assert!(state.is_cooled_down(Instant::now()));
    }

    #[test]
    fn after_rejection_not_cooled_down() {
        let mut state = CooldownState::new(60);
        let now = Instant::now();
        state.record_rejection(now);
        assert!(!state.is_cooled_down(now));
    }

    #[test]
    fn after_cooldown_duration_is_cooled_down() {
        let mut state = CooldownState::new(60);
        let now = Instant::now();
        state.record_rejection(now);
        let later = now + Duration::from_secs(60);
        assert!(state.is_cooled_down(later));
    }
}
