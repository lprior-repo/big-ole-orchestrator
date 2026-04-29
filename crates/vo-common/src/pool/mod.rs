//! Connection Pool Types
//!
//! This module defines core types for managing NATS client connections.

use std::fmt;

use serde::{Deserialize, Serialize};
use ulid::Ulid;

pub mod circuit_breaker;
pub mod health;

pub use circuit_breaker::CircuitBreakerState;
pub use health::HealthCheckResult;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoolConfig {
    pub min_connections: u32,
    pub max_connections: u32,
    pub connection_timeout_ms: u64,
    pub idle_timeout_ms: u64,
    pub health_check_interval_ms: u64,
    pub max_pending_acquires: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConnectionId(pub(crate) Ulid);

impl ConnectionId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }

    #[must_use]
    pub fn as_u128(&self) -> u128 {
        self.0.into()
    }

    #[must_use]
    pub fn to_string(self) -> String {
        self.0.to_string()
    }
}

impl Default for ConnectionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PoolId(pub(crate) String);

impl PoolId {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PoolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ConnectionStatus {
    #[default]
    Idle,
    CheckedOut,
    HealthCheck,
    Closing,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PooledConnection {
    pub connection_id: ConnectionId,
    pub created_at: TimestampMs,
    pub last_used_at: TimestampMs,
    pub use_count: u64,
    pub status: ConnectionStatus,
}

impl PooledConnection {
    pub fn new(connection_id: ConnectionId, created_at: TimestampMs) -> Self {
        Self {
            connection_id,
            created_at,
            last_used_at: created_at,
            use_count: 0,
            status: ConnectionStatus::Idle,
        }
    }

    #[must_use]
    pub fn with_status(mut self, status: ConnectionStatus) -> Self {
        self.status = status;
        self
    }

    #[must_use]
    pub fn with_use_count(mut self, use_count: u64) -> Self {
        self.use_count = use_count;
        self
    }

    pub fn increment_use_count(&mut self) {
        self.use_count += 1;
    }

    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.status == ConnectionStatus::Idle
    }

    #[must_use]
    pub fn is_checked_out(&self) -> bool {
        self.status == ConnectionStatus::CheckedOut
    }

    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.status == ConnectionStatus::Closed
    }
}

pub type TimestampMs = u64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitHandle {
    pub request_id: u64,
    pub enqueued_at: TimestampMs,
    pub pool_id: PoolId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcquireResult {
    Available { connection: PooledConnection },
    Pending { wait_handle: WaitHandle },
    PoolExhausted { config: PoolConfig },
    PoolClosing,
    Timeout { waited_ms: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseResult {
    Returned,
    AlreadyClosed,
    Evicted { reason: EvictionReason },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvictionReason {
    HealthCheckFailed(HealthCheckResult),
    ExplicitEviction,
    IdleTimeout,
    ProtocolError(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoolStats {
    pub pool_id: PoolId,
    pub total_connections: u32,
    pub idle_connections: u32,
    pub checked_out_connections: u32,
    pub pending_acquires: u32,
    pub total_acquires: u64,
    pub total_releases: u64,
    pub total_evictions: u64,
    pub total_health_checks: u64,
    pub failed_health_checks: u64,
}

impl Default for PoolStats {
    fn default() -> Self {
        Self {
            pool_id: PoolId::new(""),
            total_connections: 0,
            idle_connections: 0,
            checked_out_connections: 0,
            pending_acquires: 0,
            total_acquires: 0,
            total_releases: 0,
            total_evictions: 0,
            total_health_checks: 0,
            failed_health_checks: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    PoolExhaustion,
    Timeout,
    ConnectionFailed,
    HealthCheckFailed,
    InvalidState,
    ShutdownInProgress,
    ResourceExhaustion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorDetail {
    MaxConnectionsReached {
        max: u32,
    },
    PendingAcquiresExceeded {
        max: u32,
    },
    AcquireTimeout {
        waited_ms: u64,
        timeout_ms: u64,
    },
    NatsConnectionError {
        connection_id: ConnectionId,
        reason: String,
    },
    HealthCheckTimeout {
        connection_id: ConnectionId,
    },
    ConnectionCorrupted {
        connection_id: ConnectionId,
    },
    InvalidRelease {
        reason: &'static str,
    },
    PoolNotInitialized,
    AlreadyShutdown,
    CircuitBreakerOpen {
        consecutive_failures: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorContext {
    pub pool_id: PoolId,
    pub timestamp: TimestampMs,
    pub operation: &'static str,
    pub connection_id: Option<ConnectionId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionPoolError {
    pub category: ErrorCategory,
    pub detail: ErrorDetail,
    pub context: ErrorContext,
}

impl ErrorDetail {
    #[must_use]
    pub fn to_string(self) -> String {
        match self {
            ErrorDetail::MaxConnectionsReached { max } => {
                format!("Max connections reached: {max}")
            }
            ErrorDetail::PendingAcquiresExceeded { max } => {
                format!("Pending acquires exceeded: {max}")
            }
            ErrorDetail::AcquireTimeout {
                waited_ms,
                timeout_ms,
            } => {
                format!("Acquire timed out after {waited_ms}ms (timeout: {timeout_ms}ms)")
            }
            ErrorDetail::NatsConnectionError {
                connection_id: _,
                reason,
            } => {
                format!("NATS connection error: {reason}")
            }
            ErrorDetail::HealthCheckTimeout { connection_id } => {
                format!("Health check timed out for {connection_id}")
            }
            ErrorDetail::ConnectionCorrupted { connection_id } => {
                format!("Connection corrupted: {connection_id}")
            }
            ErrorDetail::InvalidRelease { reason } => format!("Invalid release: {reason}"),
            ErrorDetail::PoolNotInitialized => "Pool not initialized".to_string(),
            ErrorDetail::AlreadyShutdown => "Pool already shutdown".to_string(),
            ErrorDetail::CircuitBreakerOpen {
                consecutive_failures,
            } => {
                format!("Circuit breaker open: {consecutive_failures} consecutive failures")
            }
        }
    }
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorCategory::PoolExhaustion => write!(f, "PoolExhaustion"),
            ErrorCategory::Timeout => write!(f, "Timeout"),
            ErrorCategory::ConnectionFailed => write!(f, "ConnectionFailed"),
            ErrorCategory::HealthCheckFailed => write!(f, "HealthCheckFailed"),
            ErrorCategory::InvalidState => write!(f, "InvalidState"),
            ErrorCategory::ShutdownInProgress => write!(f, "ShutdownInProgress"),
            ErrorCategory::ResourceExhaustion => write!(f, "ResourceExhaustion"),
        }
    }
}

impl fmt::Display for ConnectionPoolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {:?}: {:?}",
            self.context.pool_id, self.category, self.detail
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod pool_id {
        use super::*;

        #[test]
        fn test_pool_id_new_from_string() {
            let id = PoolId::new("test-pool");
            assert_eq!(id.as_str(), "test-pool");
        }

        #[test]
        fn test_pool_id_display() {
            let id = PoolId::new("display-test");
            assert_eq!(format!("{}", id), "display-test");
        }

        #[test]
        fn test_pool_id_equality() {
            let id1 = PoolId::new("same-id");
            let id2 = PoolId::new("same-id");
            assert_eq!(id1, id2);
        }

        #[test]
        fn test_pool_id_inequality() {
            let id1 = PoolId::new("id1");
            let id2 = PoolId::new("id2");
            assert_ne!(id1, id2);
        }
    }

    mod connection_id {
        use super::*;

        #[test]
        fn test_connection_id_new_generates_unique_id() {
            let id1 = ConnectionId::new();
            let id2 = ConnectionId::new();
            assert_ne!(id1, id2);
        }

        #[test]
        fn test_connection_id_default() {
            let id1 = ConnectionId::default();
            let _id2 = ConnectionId::default();
            assert!(id1.as_u128() > 0);
        }

        #[test]
        fn test_connection_id_display() {
            let id = ConnectionId::new();
            let display = id.to_string();
            assert!(!display.is_empty());
        }

        #[test]
        fn test_connection_id_to_u128() {
            let id = ConnectionId::new();
            let u128_val = id.as_u128();
            assert!(u128_val > 0);
            assert!(u128_val < u128::MAX);
        }
    }

    mod connection_status {
        use super::*;

        #[test]
        fn test_connection_status_default_is_idle() {
            assert_eq!(ConnectionStatus::default(), ConnectionStatus::Idle);
        }

        #[test]
        fn test_connection_status_all_values() {
            let statuses = [
                ConnectionStatus::Idle,
                ConnectionStatus::CheckedOut,
                ConnectionStatus::HealthCheck,
                ConnectionStatus::Closing,
                ConnectionStatus::Closed,
            ];
            assert_eq!(statuses.len(), 5);
        }

        #[test]
        fn test_connection_status_equality() {
            assert_eq!(ConnectionStatus::Idle, ConnectionStatus::Idle);
            assert_eq!(ConnectionStatus::Closed, ConnectionStatus::Closed);
            assert_ne!(ConnectionStatus::Idle, ConnectionStatus::CheckedOut);
        }
    }

    mod pooled_connection {
        use super::*;

        fn create_test_connection() -> PooledConnection {
            let timestamp = 1000u64;
            let conn_id = ConnectionId::new();
            PooledConnection::new(conn_id, timestamp)
        }

        #[test]
        fn test_pooled_connection_new() {
            let conn = create_test_connection();
            assert_eq!(conn.status, ConnectionStatus::Idle);
            assert_eq!(conn.use_count, 0);
            assert!(conn.is_idle());
            assert!(!conn.is_checked_out());
            assert!(!conn.is_closed());
        }

        #[test]
        fn test_pooled_connection_with_status() {
            let conn = create_test_connection().with_status(ConnectionStatus::CheckedOut);
            assert_eq!(conn.status, ConnectionStatus::CheckedOut);
            assert!(conn.is_checked_out());
        }

        #[test]
        fn test_pooled_connection_with_use_count() {
            let conn = create_test_connection().with_use_count(42);
            assert_eq!(conn.use_count, 42);
        }

        #[test]
        fn test_pooled_connection_increment_use_count() {
            let mut conn = create_test_connection();
            conn.increment_use_count();
            assert_eq!(conn.use_count, 1);
            conn.increment_use_count();
            conn.increment_use_count();
            assert_eq!(conn.use_count, 3);
        }
    }

    mod acquire_result {
        use super::*;

        #[test]
        fn test_acquire_result_available() {
            let timestamp = 1000u64;
            let conn = PooledConnection::new(ConnectionId::new(), timestamp);
            let result = AcquireResult::Available { connection: conn };
            match result {
                AcquireResult::Available { .. } => {}
                _ => panic!("Expected Available variant"),
            }
        }

        #[test]
        fn test_acquire_result_pool_closing() {
            let result = AcquireResult::PoolClosing;
            match result {
                AcquireResult::PoolClosing => {}
                _ => panic!("Expected PoolClosing variant"),
            }
        }

        #[test]
        fn test_acquire_result_timeout() {
            let result = AcquireResult::Timeout { waited_ms: 5000 };
            match result {
                AcquireResult::Timeout { waited_ms } => assert_eq!(waited_ms, 5000),
                _ => panic!("Expected Timeout variant"),
            }
        }
    }

    mod release_result {
        use super::*;

        #[test]
        fn test_release_result_returned() {
            let result = ReleaseResult::Returned;
            match result {
                ReleaseResult::Returned => {}
                _ => panic!("Expected Returned variant"),
            }
        }

        #[test]
        fn test_release_result_evicted_health_check() {
            let result = ReleaseResult::Evicted {
                reason: EvictionReason::HealthCheckFailed(HealthCheckResult::Stale),
            };
            match result {
                ReleaseResult::Evicted { reason } => {
                    assert_eq!(
                        reason,
                        EvictionReason::HealthCheckFailed(HealthCheckResult::Stale)
                    );
                }
                _ => panic!("Expected Evicted variant"),
            }
        }
    }

    mod pool_stats {
        use super::*;

        #[test]
        fn test_pool_stats_default() {
            let stats = PoolStats::default();
            assert_eq!(stats.total_connections, 0);
            assert_eq!(stats.idle_connections, 0);
            assert_eq!(stats.checked_out_connections, 0);
        }

        #[test]
        fn test_pool_stats_with_values() {
            let pool_id = PoolId::new("stats-test");
            let stats = PoolStats {
                pool_id,
                total_connections: 10,
                idle_connections: 5,
                checked_out_connections: 3,
                pending_acquires: 2,
                total_acquires: 100,
                total_releases: 95,
                total_evictions: 5,
                total_health_checks: 50,
                failed_health_checks: 3,
            };
            assert_eq!(stats.total_connections, 10);
            assert_eq!(stats.idle_connections, 5);
        }
    }

    mod error_category {
        use super::*;

        #[test]
        fn test_error_category_all_values() {
            let _ = ErrorCategory::PoolExhaustion;
            let _ = ErrorCategory::Timeout;
            let _ = ErrorCategory::ConnectionFailed;
            let _ = ErrorCategory::HealthCheckFailed;
            let _ = ErrorCategory::InvalidState;
            let _ = ErrorCategory::ShutdownInProgress;
            let _ = ErrorCategory::ResourceExhaustion;
        }

        #[test]
        fn test_error_category_display() {
            assert_eq!(
                format!("{}", ErrorCategory::PoolExhaustion),
                "PoolExhaustion"
            );
            assert_eq!(format!("{}", ErrorCategory::Timeout), "Timeout");
        }
    }

    mod error_detail {
        use super::*;

        #[test]
        fn test_error_detail_to_string() {
            let detail = ErrorDetail::MaxConnectionsReached { max: 10 };
            let msg = detail.to_string();
            assert!(msg.contains("10"));
            assert!(msg.contains("Max connections reached"));
        }

        #[test]
        fn test_error_detail_pool_not_initialized() {
            let detail = ErrorDetail::PoolNotInitialized;
            assert_eq!(detail.to_string(), "Pool not initialized");
        }

        #[test]
        fn test_error_detail_circuit_breaker_open() {
            let detail = ErrorDetail::CircuitBreakerOpen {
                consecutive_failures: 5,
            };
            let msg = detail.to_string();
            assert!(msg.contains("5"));
            assert!(msg.contains("Circuit breaker open"));
        }
    }

    mod circuit_breaker_state {
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
    }
}
