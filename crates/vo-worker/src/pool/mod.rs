//! Connection Pool Manager Implementation
//!
//! This module provides the connection pool implementation for managing
//! NATS client connections in the veloxide distributed worker system.

mod circuit_breaker;
mod config;
mod health_check;
mod pool;

pub use circuit_breaker::CircuitBreaker;
pub use config::{PoolConfig, PoolConfigError};
pub use health_check::{HealthCheck, HealthCheckFuture};
pub use pool::{ConnectionPool, NatsConnectionWrapper};

use vo_types::connection_pool::{
    AcquireResult, CircuitBreakerState, ConnectionId, ConnectionPoolError, ConnectionStatus,
    ErrorCategory, ErrorContext, ErrorDetail, EvictionReason, PoolConfig as VoPoolConfig, PoolId,
    PoolStats, PooledConnection, ReleaseResult, WaitHandle,
};

use vo_types::integer_types::TimestampMs;

pub use vo_types::connection_pool::HealthCheckResult;

pub(crate) use pool::PoolState;

#[cfg(test)]
mod tests {
    mod circuit_breaker_tests;
    mod config_tests;
    mod health_check_tests;
    mod pool_tests;
}
