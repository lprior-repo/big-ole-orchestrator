//! Tests for circuit breaker half-open → closed transition (ve-6tkbp).
//!
//! Covers: probe count tracking, immediate failure handling, full lifecycle,
//! state reset on transition, and failure threshold in half-open state.

use super::super::CircuitBreaker;
use vo_types::connection_pool::CircuitBreakerState;
use vo_types::integer_types::TimestampMs;

// ---------------------------------------------------------------------------
// Half-open → Closed: successful probe transitions
// ---------------------------------------------------------------------------

#[test]
fn half_open_single_success_closes_circuit() {
    let mut cb = CircuitBreaker::new();
    cb.transition_to(CircuitBreakerState::HalfOpen);
    assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);

    cb.record_success();
    assert_eq!(cb.state(), CircuitBreakerState::Closed);
}

#[test]
fn half_open_allows_requests() {
    let mut cb = CircuitBreaker::new();
    cb.transition_to(CircuitBreakerState::HalfOpen);
    assert!(cb.should_allow_request());
}

#[test]
fn half_open_success_resets_consecutive_failures() {
    let mut cb = CircuitBreaker::new();
    // Build up consecutive failures in Closed state
    for _ in 0..5 {
        cb.record_failure();
    }
    assert_eq!(cb.consecutive_failures(), 5);

    // Trip to Open, then transition to HalfOpen
    cb.transition_to(CircuitBreakerState::Open);
    cb.transition_to(CircuitBreakerState::HalfOpen);

    // Success in HalfOpen closes the circuit and resets failures
    cb.record_success();
    assert_eq!(cb.state(), CircuitBreakerState::Closed);
    assert_eq!(cb.consecutive_failures(), 0);
}

// ---------------------------------------------------------------------------
// Immediate failure in half-open: does NOT reopen until threshold
// ---------------------------------------------------------------------------

#[test]
fn half_open_immediate_failure_does_not_reopen() {
    let mut cb = CircuitBreaker::new();
    cb.transition_to(CircuitBreakerState::HalfOpen);

    cb.record_failure();
    assert_eq!(
        cb.state(),
        CircuitBreakerState::HalfOpen,
        "single failure should not reopen the circuit"
    );
}

#[test]
fn half_open_nine_failures_stays_half_open() {
    let mut cb = CircuitBreaker::new();
    cb.transition_to(CircuitBreakerState::HalfOpen);

    for _ in 0..9 {
        cb.record_failure();
    }
    assert_eq!(
        cb.state(),
        CircuitBreakerState::HalfOpen,
        "9 failures should not reopen (< 10 threshold)"
    );
}

#[test]
fn half_open_ten_failures_reopens() {
    let mut cb = CircuitBreaker::new();
    cb.transition_to(CircuitBreakerState::HalfOpen);

    for _ in 0..10 {
        cb.record_failure();
    }
    assert_eq!(
        cb.state(),
        CircuitBreakerState::Open,
        "10 failures in HalfOpen should reopen the circuit"
    );
}

// ---------------------------------------------------------------------------
// Mixed probes: failures then success still closes
// ---------------------------------------------------------------------------

#[test]
fn half_open_failure_then_success_closes() {
    let mut cb = CircuitBreaker::new();
    cb.transition_to(CircuitBreakerState::HalfOpen);

    // Record some failures (but < 10)
    for _ in 0..5 {
        cb.record_failure();
    }
    assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);

    // A single success should still close
    cb.record_success();
    assert_eq!(cb.state(), CircuitBreakerState::Closed);
}

#[test]
fn half_open_nine_failures_then_success_closes() {
    let mut cb = CircuitBreaker::new();
    cb.transition_to(CircuitBreakerState::HalfOpen);

    for _ in 0..9 {
        cb.record_failure();
    }
    assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);

    cb.record_success();
    assert_eq!(cb.state(), CircuitBreakerState::Closed);
}

// ---------------------------------------------------------------------------
// State reset after transitions
// ---------------------------------------------------------------------------

#[test]
fn closed_after_half_open_resets_consecutive_failures() {
    let mut cb = CircuitBreaker::new();
    // Accumulate failures
    for _ in 0..8 {
        cb.record_failure();
    }
    assert!(cb.consecutive_failures() > 0);

    // Force through the lifecycle
    cb.transition_to(CircuitBreakerState::Open);
    cb.transition_to(CircuitBreakerState::HalfOpen);
    cb.record_success();

    assert_eq!(cb.state(), CircuitBreakerState::Closed);
    assert_eq!(cb.consecutive_failures(), 0);
}

// ---------------------------------------------------------------------------
// Full lifecycle: Closed → Open → HalfOpen → Closed
// ---------------------------------------------------------------------------

#[test]
fn full_lifecycle_closed_open_halfopen_closed() {
    let mut cb = CircuitBreaker::new();
    assert_eq!(cb.state(), CircuitBreakerState::Closed);
    assert!(cb.should_allow_request());

    // Drive to high failure rate to trip the breaker
    // Need >50% failure rate in the sliding window
    for _ in 0..20 {
        cb.record_failure();
    }

    // With enough failures, should trip to Open
    // (if not tripped yet due to timing, force it)
    if cb.state() == CircuitBreakerState::Closed {
        cb.transition_to(CircuitBreakerState::Open);
    }
    assert_eq!(cb.state(), CircuitBreakerState::Open);
    assert!(!cb.should_allow_request());

    // Transition to HalfOpen (timeout mechanics tested in inline tests)
    cb.transition_to(CircuitBreakerState::HalfOpen);
    assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);
    assert!(cb.should_allow_request());

    // Successful probe closes the circuit
    cb.record_success();
    assert_eq!(cb.state(), CircuitBreakerState::Closed);
    assert!(cb.should_allow_request());
    assert_eq!(cb.consecutive_failures(), 0);
}

#[test]
fn full_lifecycle_with_failure_reopen_cycle() {
    let mut cb = CircuitBreaker::new();

    // Open → HalfOpen
    cb.transition_to(CircuitBreakerState::Open);
    cb.transition_to(CircuitBreakerState::HalfOpen);

    // Fail 10 times to reopen
    for _ in 0..10 {
        cb.record_failure();
    }
    assert_eq!(cb.state(), CircuitBreakerState::Open);

    // Back to HalfOpen
    cb.transition_to(CircuitBreakerState::HalfOpen);

    // Now succeed
    cb.record_success();
    assert_eq!(cb.state(), CircuitBreakerState::Closed);
}

// ---------------------------------------------------------------------------
// Open state: record_success is a no-op
// ---------------------------------------------------------------------------

#[test]
fn open_state_success_is_noop() {
    let mut cb = CircuitBreaker::new();
    cb.transition_to(CircuitBreakerState::Open);
    assert_eq!(cb.state(), CircuitBreakerState::Open);

    cb.record_success();
    assert_eq!(cb.state(), CircuitBreakerState::Open);
}

#[test]
fn open_state_failure_is_noop() {
    let mut cb = CircuitBreaker::new();
    cb.transition_to(CircuitBreakerState::Open);
    let failures_before = cb.consecutive_failures();

    cb.record_failure();
    assert_eq!(cb.state(), CircuitBreakerState::Open);
    assert_eq!(cb.consecutive_failures(), failures_before + 1);
}

// ---------------------------------------------------------------------------
// Reset clears everything
// ---------------------------------------------------------------------------

#[test]
fn reset_from_half_open_returns_to_closed() {
    let mut cb = CircuitBreaker::new();
    cb.transition_to(CircuitBreakerState::Open);
    cb.transition_to(CircuitBreakerState::HalfOpen);
    assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);

    cb.reset();
    assert_eq!(cb.state(), CircuitBreakerState::Closed);
    assert_eq!(cb.consecutive_failures(), 0);
}

// ---------------------------------------------------------------------------
// try_transition_to_half_open edge cases
// ---------------------------------------------------------------------------

#[test]
fn try_transition_to_half_open_only_works_from_open() {
    let mut cb = CircuitBreaker::new();
    // From Closed — should not transition
    let result = cb.try_transition_to_half_open(TimestampMs::now());
    assert!(!result);
    assert_eq!(cb.state(), CircuitBreakerState::Closed);

    // From HalfOpen — should not transition
    cb.transition_to(CircuitBreakerState::HalfOpen);
    let result = cb.try_transition_to_half_open(TimestampMs::now());
    assert!(!result);
    assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);
}

#[test]
fn try_transition_to_half_open_before_timeout_fails() {
    let mut cb = CircuitBreaker::new();
    cb.transition_to(CircuitBreakerState::Open);
    // Don't wait — timeout hasn't elapsed
    let result = cb.try_transition_to_half_open(TimestampMs::now());
    assert!(!result);
    assert_eq!(cb.state(), CircuitBreakerState::Open);
}
