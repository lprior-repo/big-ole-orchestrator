//! Circuit breaker implementation for connection pool resilience.

use std::collections::VecDeque;
use vo_types::connection_pool::CircuitBreakerState;
use vo_types::integer_types::TimestampMs;

const FAILURE_WINDOW_MS: u64 = 30000;

#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    state: CircuitBreakerState,
    failure_history: VecDeque<(u64, bool)>,
    last_transition_at: Option<TimestampMs>,
    consecutive_failures: u32,
    half_open_test_connections: u32,
}

impl CircuitBreaker {
    pub fn new() -> Self {
        Self {
            state: CircuitBreakerState::Closed,
            failure_history: VecDeque::new(),
            last_transition_at: None,
            consecutive_failures: 0,
            half_open_test_connections: 0,
        }
    }

    pub fn state(&self) -> CircuitBreakerState {
        self.state
    }

    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
        match self.state {
            CircuitBreakerState::Closed => {
                self.consecutive_failures = 0;
                self.failure_history
                    .push_back((TimestampMs::now().as_u64(), true));
                self.trim_history();
            }
            CircuitBreakerState::HalfOpen => {
                self.half_open_test_connections += 1;
                if self.half_open_test_connections >= 1 {
                    self.transition_to(CircuitBreakerState::Closed);
                }
            }
            CircuitBreakerState::Open => {}
        }
    }

    pub fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        self.failure_history
            .push_back((TimestampMs::now().as_u64(), false));
        self.trim_history();

        match self.state {
            CircuitBreakerState::Closed => {
                if self.should_trip() {
                    self.transition_to(CircuitBreakerState::Open);
                }
            }
            CircuitBreakerState::HalfOpen => {
                self.half_open_test_connections += 1;
                if self.half_open_test_connections >= 10 {
                    self.transition_to(CircuitBreakerState::Open);
                }
            }
            CircuitBreakerState::Open => {}
        }
    }

    pub fn should_allow_request(&self) -> bool {
        match self.state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => false,
            CircuitBreakerState::HalfOpen => true,
        }
    }

    pub fn try_transition_to_half_open(&mut self, current_time: TimestampMs) -> bool {
        if self.state != CircuitBreakerState::Open {
            return false;
        }

        if let Some(last_transition) = self.last_transition_at {
            let elapsed = current_time
                .as_u64()
                .saturating_sub(last_transition.as_u64());
            if elapsed >= FAILURE_WINDOW_MS {
                self.transition_to(CircuitBreakerState::HalfOpen);
                return true;
            }
        }

        false
    }

    pub(crate) fn transition_to(&mut self, new_state: CircuitBreakerState) {
        self.state = new_state;
        self.last_transition_at = Some(TimestampMs::now());

        match new_state {
            CircuitBreakerState::Closed => {
                self.consecutive_failures = 0;
                self.half_open_test_connections = 0;
            }
            CircuitBreakerState::HalfOpen => {
                self.half_open_test_connections = 0;
            }
            CircuitBreakerState::Open => {}
        }
    }

    fn should_trip(&self) -> bool {
        let recent_failures: usize = self
            .failure_history
            .iter()
            .filter(|(time, success)| {
                !success && TimestampMs::now().as_u64().saturating_sub(*time) <= FAILURE_WINDOW_MS
            })
            .count();

        let total_recent: usize = self
            .failure_history
            .iter()
            .filter(|(time, _)| {
                TimestampMs::now().as_u64().saturating_sub(*time) <= FAILURE_WINDOW_MS
            })
            .count();

        if total_recent == 0 {
            return false;
        }

        let failure_rate = recent_failures as f64 / total_recent as f64;
        failure_rate > 0.5
    }

    fn trim_history(&mut self) {
        let cutoff = TimestampMs::now()
            .as_u64()
            .saturating_sub(FAILURE_WINDOW_MS * 2);
        while let Some((time, _)) = self.failure_history.front() {
            if *time < cutoff {
                self.failure_history.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }

    pub fn reset(&mut self) {
        self.state = CircuitBreakerState::Closed;
        self.failure_history.clear();
        self.last_transition_at = None;
        self.consecutive_failures = 0;
        self.half_open_test_connections = 0;
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod circuit_breaker_tests {
    use super::*;

    #[test]
    fn test_initial_state_is_closed() {
        let cb = CircuitBreaker::new();
        assert_eq!(cb.state(), CircuitBreakerState::Closed);
    }

    #[test]
    fn test_success_resets_consecutive_failures() {
        let mut cb = CircuitBreaker::new();
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.consecutive_failures(), 2);
        cb.record_success();
        assert_eq!(cb.consecutive_failures(), 0);
    }

    #[test]
    fn test_half_open_transition_on_success() {
        let mut cb = CircuitBreaker::new();
        cb.transition_to(CircuitBreakerState::HalfOpen);
        assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);
        cb.record_success();
        assert_eq!(cb.state(), CircuitBreakerState::Closed);
    }

    #[test]
    fn test_open_state_rejects_requests() {
        let mut cb = CircuitBreaker::new();
        cb.transition_to(CircuitBreakerState::Open);
        assert!(!cb.should_allow_request());
    }

    #[test]
    fn test_half_open_state_allows_requests() {
        let mut cb = CircuitBreaker::new();
        cb.transition_to(CircuitBreakerState::HalfOpen);
        assert!(cb.should_allow_request());
    }

    #[test]
    fn test_closed_state_allows_requests() {
        let cb = CircuitBreaker::new();
        assert!(cb.should_allow_request());
    }

    #[test]
    fn test_failure_in_half_open_transitions_to_open() {
        let mut cb = CircuitBreaker::new();
        cb.transition_to(CircuitBreakerState::HalfOpen);
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitBreakerState::Open);
    }

    #[test]
    fn test_reset_clears_state() {
        let mut cb = CircuitBreaker::new();
        cb.transition_to(CircuitBreakerState::Open);
        cb.reset();
        assert_eq!(cb.state(), CircuitBreakerState::Closed);
    }

    #[test]
    fn test_try_transition_to_half_open_after_timeout() {
        let mut cb = CircuitBreaker::new();
        cb.transition_to(CircuitBreakerState::Open);
        cb.last_transition_at = Some(TimestampMs::new_unchecked(
            TimestampMs::now()
                .as_u64()
                .saturating_sub(FAILURE_WINDOW_MS + 1000),
        ));

        let result = cb.try_transition_to_half_open(TimestampMs::now());
        assert!(result);
        assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);
    }

    #[test]
    fn test_try_transition_to_half_open_before_timeout() {
        let mut cb = CircuitBreaker::new();
        cb.transition_to(CircuitBreakerState::Open);
        cb.last_transition_at = Some(TimestampMs::now());

        let result = cb.try_transition_to_half_open(TimestampMs::now());
        assert!(!result);
        assert_eq!(cb.state(), CircuitBreakerState::Open);
    }

    // ========================================================================
    // Half-Open → Open Transition Tests (ve-gltfe)
    // ========================================================================

    /// Given: Circuit breaker in HalfOpen state
    /// When: A single failure is recorded
    /// Then: The breaker does NOT immediately transition to Open (threshold=10)
    #[test]
    fn test_half_open_single_failure_stays_half_open() {
        let mut cb = CircuitBreaker::new();
        cb.transition_to(CircuitBreakerState::HalfOpen);
        assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);

        cb.record_failure();
        assert_eq!(
            cb.state(),
            CircuitBreakerState::HalfOpen,
            "single failure should not immediately trip to Open"
        );
        assert_eq!(cb.consecutive_failures(), 1);
    }

    /// Given: Circuit breaker in HalfOpen state
    /// When: Failures are recorded up to the threshold (10)
    /// Then: The breaker transitions back to Open
    #[test]
    fn test_half_open_threshold_failures_transitions_to_open() {
        let mut cb = CircuitBreaker::new();
        cb.transition_to(CircuitBreakerState::HalfOpen);

        // 9 failures: still half-open
        for _ in 0..9 {
            cb.record_failure();
        }
        assert_eq!(
            cb.state(),
            CircuitBreakerState::HalfOpen,
            "9 failures should not yet trip to Open"
        );

        // 10th failure: transitions to Open
        cb.record_failure();
        assert_eq!(
            cb.state(),
            CircuitBreakerState::Open,
            "10 failures in half-open should trip to Open"
        );
    }

    /// Given: Circuit breaker in HalfOpen state
    /// When: Interleaved successes and failures
    /// Then: Success resets to Closed; subsequent failures start fresh count
    #[test]
    fn test_half_open_success_resets_then_failures_accumulate() {
        let mut cb = CircuitBreaker::new();
        cb.transition_to(CircuitBreakerState::HalfOpen);

        // Some failures first
        cb.record_failure();
        cb.record_failure();

        // Success transitions to Closed immediately
        cb.record_success();
        assert_eq!(cb.state(), CircuitBreakerState::Closed);

        // Now record failures in Closed state — should eventually trip to Open
        // via failure rate (need >50% failure rate in window)
        for _ in 0..100 {
            cb.record_failure();
        }
        assert_eq!(
            cb.state(),
            CircuitBreakerState::Open,
            "sustained failures should trip breaker back to Open"
        );
    }

    /// Given: Circuit breaker in HalfOpen state
    /// When: 9 failures followed by a success
    /// Then: Success transitions to Closed, resetting the failure counter
    #[test]
    fn test_half_open_near_threshold_then_success_closes() {
        let mut cb = CircuitBreaker::new();
        cb.transition_to(CircuitBreakerState::HalfOpen);

        for _ in 0..9 {
            cb.record_failure();
        }
        assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);

        cb.record_success();
        assert_eq!(
            cb.state(),
            CircuitBreakerState::Closed,
            "success at 9 failures should close the breaker"
        );
        assert_eq!(cb.consecutive_failures(), 0);
    }

    // ========================================================================
    // Open → Half-Open Transition Tests (ve-xx2g7)
    // ========================================================================

    /// Given: Circuit breaker in Open state with timeout window just expired
    /// When: try_transition_to_half_open is called at exact boundary
    /// Then: Transition succeeds
    #[test]
    fn test_open_to_half_open_at_exact_timeout_boundary() {
        let mut cb = CircuitBreaker::new();
        cb.transition_to(CircuitBreakerState::Open);
        // Set last transition exactly FAILURE_WINDOW_MS ago
        cb.last_transition_at = Some(TimestampMs::new_unchecked(
            TimestampMs::now().as_u64().saturating_sub(FAILURE_WINDOW_MS),
        ));

        let result = cb.try_transition_to_half_open(TimestampMs::now());
        assert!(result, "should transition at exact boundary");
        assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);
    }

    /// Given: Circuit breaker in Open state, just barely past timeout
    /// When: try_transition_to_half_open is called
    /// Then: Transition succeeds (1ms past is enough)
    #[test]
    fn test_open_to_half_open_one_ms_past_timeout() {
        let mut cb = CircuitBreaker::new();
        cb.transition_to(CircuitBreakerState::Open);
        cb.last_transition_at = Some(TimestampMs::new_unchecked(
            TimestampMs::now().as_u64().saturating_sub(FAILURE_WINDOW_MS + 1),
        ));

        let result = cb.try_transition_to_half_open(TimestampMs::now());
        assert!(result);
        assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);
    }

    /// Given: Circuit breaker in Open state, 1ms before timeout
    /// When: try_transition_to_half_open is called
    /// Then: Transition fails, stays Open
    #[test]
    fn test_open_stays_open_one_ms_before_timeout() {
        let mut cb = CircuitBreaker::new();
        cb.transition_to(CircuitBreakerState::Open);
        cb.last_transition_at = Some(TimestampMs::new_unchecked(
            TimestampMs::now().as_u64().saturating_sub(FAILURE_WINDOW_MS - 1),
        ));

        let result = cb.try_transition_to_half_open(TimestampMs::now());
        assert!(!result, "should NOT transition before timeout");
        assert_eq!(cb.state(), CircuitBreakerState::Open);
    }

    /// Given: Circuit breaker in Open state (rejects requests)
    /// When: Requests are attempted during Open state
    /// Then: All requests are rejected
    #[test]
    fn test_open_state_rejects_all_requests_early_access() {
        let mut cb = CircuitBreaker::new();
        cb.transition_to(CircuitBreakerState::Open);

        // Multiple attempts while Open — all rejected
        for _ in 0..5 {
            assert!(
                !cb.should_allow_request(),
                "Open state must reject all requests"
            );
        }
        assert_eq!(cb.state(), CircuitBreakerState::Open);
    }

    /// Given: Circuit breaker in Open state
    /// When: HalfOpen is entered via timeout, then success
    /// Then: Full cycle Open → HalfOpen → Closed completes
    #[test]
    fn test_full_cycle_open_to_half_open_to_closed() {
        let mut cb = CircuitBreaker::new();
        cb.transition_to(CircuitBreakerState::Open);
        assert_eq!(cb.state(), CircuitBreakerState::Open);
        assert!(!cb.should_allow_request());

        // Advance past timeout
        cb.last_transition_at = Some(TimestampMs::new_unchecked(
            TimestampMs::now().as_u64().saturating_sub(FAILURE_WINDOW_MS + 1),
        ));
        assert!(cb.try_transition_to_half_open(TimestampMs::now()));
        assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);
        assert!(cb.should_allow_request());

        // Success closes the breaker
        cb.record_success();
        assert_eq!(cb.state(), CircuitBreakerState::Closed);
        assert!(cb.should_allow_request());
    }

    /// Given: Circuit breaker NOT in Open state
    /// When: try_transition_to_half_open is called
    /// Then: Returns false, state unchanged
    #[test]
    fn test_try_half_open_from_closed_is_noop() {
        let mut cb = CircuitBreaker::new();
        assert_eq!(cb.state(), CircuitBreakerState::Closed);

        let result = cb.try_transition_to_half_open(TimestampMs::now());
        assert!(!result);
        assert_eq!(cb.state(), CircuitBreakerState::Closed);
    }

    #[test]
    fn test_try_half_open_from_half_open_is_noop() {
        let mut cb = CircuitBreaker::new();
        cb.transition_to(CircuitBreakerState::HalfOpen);

        let result = cb.try_transition_to_half_open(TimestampMs::now());
        assert!(!result);
        assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);
    }
}
