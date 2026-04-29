//! Circuit breaker types for connection pool.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CircuitBreakerState {
    #[default]
    Closed,
    Open,
    HalfOpen,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_state_default_is_closed() {
        assert_eq!(CircuitBreakerState::default(), CircuitBreakerState::Closed);
    }

    #[test]
    fn test_circuit_breaker_state_all_values() {
        let states = [
            CircuitBreakerState::Closed,
            CircuitBreakerState::Open,
            CircuitBreakerState::HalfOpen,
        ];
        assert_eq!(states.len(), 3);
    }

    #[test]
    fn test_circuit_breaker_state_equality() {
        assert_eq!(CircuitBreakerState::Closed, CircuitBreakerState::Closed);
        assert_ne!(CircuitBreakerState::Closed, CircuitBreakerState::Open);
        assert_ne!(CircuitBreakerState::Open, CircuitBreakerState::HalfOpen);
    }
}
