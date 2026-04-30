//! Connection Pool Manager Implementation
//!
//! This module provides the connection pool implementation for managing
//! NATS client connections in the veloxide distributed worker system.

pub mod circuit_breaker;
pub mod config;
pub mod hash_ring;
pub mod health_check;
#[allow(clippy::module_inception)]
mod pool;

pub use circuit_breaker::CircuitBreaker;
pub use config::{PoolConfig, PoolConfigError};
pub use hash_ring::{HashRing, HashRingConfig, RingNode};
pub use health_check::{determine_health_check_result, HealthCheck, HealthCheckFuture};
pub use pool::{ConnectionPool, DemandSignal, NatsConnectionWrapper, PoolScaler, ScaleResult};

use vo_common::connection_pool::{
    AcquireResult, CircuitBreakerState, ConnectionId, ConnectionPoolError, ConnectionStatus,
    ErrorCategory, ErrorContext, ErrorDetail, EvictionReason, PoolConfig as VoPoolConfig, PoolId,
    PoolStats, PooledConnection, ReleaseResult, WaitHandle,
};

use vo_common::types::TimestampMs;

pub use vo_common::connection_pool::HealthCheckResult;

pub(crate) use pool::PoolState;

#[cfg(test)]
mod tests {
    mod circuit_breaker_tests;
    mod config_tests;
    mod health_check_tests;
    mod pool_tests;
}
