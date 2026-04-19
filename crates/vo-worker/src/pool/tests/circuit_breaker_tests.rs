use vo_types::connection_pool::{CircuitBreakerState, PoolId};

use crate::pool::{CircuitBreaker, PoolConfig, PoolState};

fn test_pool() -> PoolState {
    PoolState::new(PoolId::new("test-pool"), PoolConfig::with_defaults())
}

#[test]
fn manual_reset_from_open_transitions_to_closed() {
    let mut pool = test_pool();
    pool.circuit_breaker.transition_to(CircuitBreakerState::Open);
    assert_eq!(pool.circuit_breaker.state(), CircuitBreakerState::Open);

    pool.reset_circuit_breaker();
    assert_eq!(pool.circuit_breaker.state(), CircuitBreakerState::Closed);
}

#[test]
fn manual_reset_from_half_open_transitions_to_closed() {
    let mut pool = test_pool();
    pool.circuit_breaker.transition_to(CircuitBreakerState::HalfOpen);
    assert_eq!(pool.circuit_breaker.state(), CircuitBreakerState::HalfOpen);

    pool.reset_circuit_breaker();
    assert_eq!(pool.circuit_breaker.state(), CircuitBreakerState::Closed);
}
