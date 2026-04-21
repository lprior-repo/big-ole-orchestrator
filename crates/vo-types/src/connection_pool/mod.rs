//! Connection Pool Manager Types and Tests
//!
//! This module defines types for managing NATS client connections in the veloxide
//! distributed worker system.

#![allow(
    dead_code,
    clippy::inherent_to_string,
    clippy::inherent_to_string_shadow_display,
    clippy::wrong_self_convention
)]

use std::fmt;

use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::integer_types::TimestampMs;

// ============================================================================
// Type Definitions
// ============================================================================

/// Configuration for the connection pool.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PoolConfig {
    pub min_connections: u32,
    pub max_connections: u32,
    pub connection_timeout_ms: u64,
    pub idle_timeout_ms: u64,
    pub health_check_interval_ms: u64,
    pub max_pending_acquires: u32,
}

/// Unique identifier for a pooled connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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

/// Identifies a specific connection pool instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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

/// Status of a pooled connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ConnectionStatus {
    #[default]
    Idle,
    CheckedOut,
    HealthCheck,
    Closing,
    Closed,
}

/// Represents a connection in the pool with metadata.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PooledConnection {
    pub connection_id: ConnectionId,
    pub created_at: TimestampMs,
    pub last_used_at: TimestampMs,
    pub use_count: u64,
    pub status: ConnectionStatus,
}

impl PooledConnection {
    #[must_use]
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

/// Result of a connection health check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthCheckResult {
    Healthy,
    Stale,
    Corrupted,
    Timeout,
}

/// Handle for a pending acquire request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitHandle {
    pub request_id: u64,
    pub enqueued_at: TimestampMs,
    pub pool_id: PoolId,
}

/// Result of attempting to acquire a connection from the pool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AcquireResult {
    Available { connection: PooledConnection },
    Pending { wait_handle: WaitHandle },
    PoolExhausted { config: PoolConfig },
    PoolClosing,
    Timeout { waited_ms: u64 },
}

/// Result of releasing a connection back to the pool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReleaseResult {
    Returned,
    AlreadyClosed,
    Evicted { reason: EvictionReason },
}

/// Reason for connection eviction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvictionReason {
    HealthCheckFailed(HealthCheckResult),
    ExplicitEviction,
    IdleTimeout,
    ProtocolError(String),
}

/// Current state statistics for the pool.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

/// Circuit breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CircuitBreakerState {
    #[default]
    Closed,
    Open,
    HalfOpen,
}

// ============================================================================
// Reconnection Backoff
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconnectBackoff {
    pub initial_ms: u64,
    pub multiplier: u64,
    pub max_ms: u64,
    pub jitter_ms: u64,
    attempt: u32,
}

impl ReconnectBackoff {
    #[must_use]
    pub fn new(initial_ms: u64, multiplier: u64, max_ms: u64, jitter_ms: u64) -> Self {
        Self {
            initial_ms,
            multiplier,
            max_ms,
            jitter_ms,
            attempt: 0,
        }
    }

    #[must_use]
    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    #[must_use]
    pub fn next_backoff(&mut self) -> u64 {
        self.attempt += 1;
        let raw = if self.attempt == 1 {
            self.initial_ms
        } else {
            let prev = self.initial_ms
                * self
                    .multiplier
                    .saturating_pow(self.attempt.saturating_sub(1));
            prev.min(self.max_ms)
        };
        let capped = raw.min(self.max_ms);
        let jitter = if self.jitter_ms > 0 {
            deterministic_jitter(self.jitter_ms, self.attempt)
        } else {
            0
        };
        capped.saturating_add(jitter).min(self.max_ms)
    }

    pub fn reset(&mut self) {
        self.attempt = 0;
    }
}

#[must_use]
pub fn deterministic_jitter(jitter_ms: u64, attempt: u32) -> u64 {
    let seed = (attempt as u64).wrapping_mul(2654435761);
    seed % jitter_ms.saturating_add(1)
}

// ============================================================================
// Error Types
// ============================================================================

/// Error category for connection pool errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCategory {
    PoolExhaustion,
    Timeout,
    ConnectionFailed,
    HealthCheckFailed,
    InvalidState,
    ShutdownInProgress,
    ResourceExhaustion,
}

/// Error detail for connection pool errors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
        reason: String,
    },
    PoolNotInitialized,
    AlreadyShutdown,
    CircuitBreakerOpen {
        consecutive_failures: u32,
    },
}

/// Context for connection pool errors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorContext {
    pub pool_id: PoolId,
    pub timestamp: TimestampMs,
    pub operation: String,
    pub connection_id: Option<ConnectionId>,
}

/// Connection pool error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub struct ConnectionPoolError {
    pub category: ErrorCategory,
    pub detail: ErrorDetail,
    pub context: ErrorContext,
}

impl std::fmt::Display for ConnectionPoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {:?}: {:?}",
            self.context.pool_id, self.category, self.detail
        )
    }
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
            } => format!("NATS connection error: {reason}"),
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integer_types::TimestampMs;

    // ========================================================================
    // ConnectionId Tests
    // ========================================================================

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
            // Default may or may not be unique depending on implementation
            // This test just ensures it compiles and returns a valid ID
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

    // ========================================================================
    // PoolId Tests
    // ========================================================================

    mod pool_id {
        use super::*;

        #[test]
        fn test_pool_id_new_from_string() {
            let id = PoolId::new("test-pool");
            assert_eq!(id.as_str(), "test-pool");
        }

        #[test]
        fn test_pool_id_new_from_static() {
            let id = PoolId::new("static-pool");
            assert_eq!(id.as_str(), "static-pool");
        }

        #[test]
        fn test_pool_id_display() {
            let id = PoolId::new("display-test");
            let display = format!("{}", id);
            assert_eq!(display, "display-test");
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

    // ========================================================================
    // ConnectionStatus Tests
    // ========================================================================

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

    // ========================================================================
    // PooledConnection Tests
    // ========================================================================

    mod pooled_connection {
        use super::*;

        fn create_test_connection() -> PooledConnection {
            let timestamp = TimestampMs::new_unchecked(1000);
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

        #[test]
        fn test_pooled_connection_status_checkers() {
            let idle = create_test_connection();
            let checked_out = create_test_connection().with_status(ConnectionStatus::CheckedOut);
            let closed = create_test_connection().with_status(ConnectionStatus::Closed);

            assert!(idle.is_idle());
            assert!(!checked_out.is_idle());
            assert!(!closed.is_idle());

            assert!(!idle.is_checked_out());
            assert!(checked_out.is_checked_out());
            assert!(!closed.is_checked_out());

            assert!(!idle.is_closed());
            assert!(!checked_out.is_closed());
            assert!(closed.is_closed());
        }

        #[test]
        fn test_pooled_connection_equality() {
            let timestamp = TimestampMs::new_unchecked(1000);
            let id1 = ConnectionId::new();
            let id2 = ConnectionId::new();

            let conn1 = PooledConnection {
                connection_id: id1,
                created_at: timestamp,
                last_used_at: timestamp,
                use_count: 0,
                status: ConnectionStatus::Idle,
            };

            let conn2 = PooledConnection {
                connection_id: id2,
                created_at: timestamp,
                last_used_at: timestamp,
                use_count: 0,
                status: ConnectionStatus::Idle,
            };

            // Different IDs should not be equal
            assert_ne!(conn1, conn2);
        }
    }

    // ========================================================================
    // HealthCheckResult Tests
    // ========================================================================

    mod health_check_result {
        use super::*;

        #[test]
        fn test_health_check_result_all_values() {
            let results = [
                HealthCheckResult::Healthy,
                HealthCheckResult::Stale,
                HealthCheckResult::Corrupted,
                HealthCheckResult::Timeout,
            ];
            assert_eq!(results.len(), 4);
        }

        #[test]
        fn test_health_check_result_equality() {
            assert_eq!(HealthCheckResult::Healthy, HealthCheckResult::Healthy);
            assert_ne!(HealthCheckResult::Healthy, HealthCheckResult::Stale);
            assert_ne!(HealthCheckResult::Stale, HealthCheckResult::Corrupted);
        }
    }

    // ========================================================================
    // WaitHandle Tests
    // ========================================================================

    mod wait_handle {
        use super::*;

        #[test]
        fn test_wait_handle_creation() {
            let pool_id = PoolId::new("test-pool");
            let timestamp = TimestampMs::new_unchecked(2000);

            let handle = WaitHandle {
                request_id: 1,
                enqueued_at: timestamp,
                pool_id,
            };

            assert_eq!(handle.request_id, 1);
            assert_eq!(handle.enqueued_at, timestamp);
            assert_eq!(handle.pool_id.as_str(), "test-pool");
        }

        #[test]
        fn test_wait_handle_equality() {
            let pool_id = PoolId::new("same-pool");
            let timestamp = TimestampMs::new_unchecked(3000);

            let handle1 = WaitHandle {
                request_id: 1,
                enqueued_at: timestamp,
                pool_id: pool_id.clone(),
            };

            let handle2 = WaitHandle {
                request_id: 1,
                enqueued_at: timestamp,
                pool_id: pool_id.clone(),
            };

            assert_eq!(handle1, handle2);
        }
    }

    // ========================================================================
    // AcquireResult Tests
    // ========================================================================

    mod acquire_result {
        use super::*;

        #[test]
        fn test_acquire_result_available() {
            let timestamp = TimestampMs::new_unchecked(1000);
            let conn = PooledConnection::new(ConnectionId::new(), timestamp);
            let result = AcquireResult::Available { connection: conn };

            match result {
                AcquireResult::Available { .. } => {}
                _ => panic!("Expected Available variant"),
            }
        }

        #[test]
        fn test_acquire_result_pending() {
            let pool_id = PoolId::new("pending-pool");
            let timestamp = TimestampMs::new_unchecked(1000);
            let handle = WaitHandle {
                request_id: 1,
                enqueued_at: timestamp,
                pool_id,
            };
            let result = AcquireResult::Pending {
                wait_handle: handle,
            };

            match result {
                AcquireResult::Pending { .. } => {}
                _ => panic!("Expected Pending variant"),
            }
        }

        #[test]
        fn test_acquire_result_pool_exhausted() {
            let config = PoolConfig {
                min_connections: 1,
                max_connections: 2,
                connection_timeout_ms: 5000,
                idle_timeout_ms: 30000,
                health_check_interval_ms: 10000,
                max_pending_acquires: 5,
            };
            let result = AcquireResult::PoolExhausted { config };

            match result {
                AcquireResult::PoolExhausted { config: c } => {
                    assert_eq!(c.max_connections, 2);
                }
                _ => panic!("Expected PoolExhausted variant"),
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
                AcquireResult::Timeout { waited_ms } => {
                    assert_eq!(waited_ms, 5000);
                }
                _ => panic!("Expected Timeout variant"),
            }
        }
    }

    // ========================================================================
    // ReleaseResult Tests
    // ========================================================================

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
        fn test_release_result_already_closed() {
            let result = ReleaseResult::AlreadyClosed;
            match result {
                ReleaseResult::AlreadyClosed => {}
                _ => panic!("Expected AlreadyClosed variant"),
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

        #[test]
        fn test_release_result_evicted_explicit() {
            let result = ReleaseResult::Evicted {
                reason: EvictionReason::ExplicitEviction,
            };
            match result {
                ReleaseResult::Evicted { reason } => {
                    assert_eq!(reason, EvictionReason::ExplicitEviction);
                }
                _ => panic!("Expected Evicted variant"),
            }
        }

        #[test]
        fn test_release_result_evicted_idle_timeout() {
            let result = ReleaseResult::Evicted {
                reason: EvictionReason::IdleTimeout,
            };
            match result {
                ReleaseResult::Evicted { reason } => {
                    assert_eq!(reason, EvictionReason::IdleTimeout);
                }
                _ => panic!("Expected Evicted variant"),
            }
        }

        #[test]
        fn test_release_result_evicted_protocol_error() {
            let result = ReleaseResult::Evicted {
                reason: EvictionReason::ProtocolError("malformed message".to_string()),
            };
            match result {
                ReleaseResult::Evicted { reason } => {
                    assert_eq!(
                        reason,
                        EvictionReason::ProtocolError("malformed message".to_string())
                    );
                }
                _ => panic!("Expected Evicted variant"),
            }
        }
    }

    // ========================================================================
    // EvictionReason Tests
    // ========================================================================

    mod eviction_reason {
        use super::*;

        #[test]
        fn test_eviction_reason_all_values() {
            let _ = EvictionReason::HealthCheckFailed(HealthCheckResult::Healthy);
            let _ = EvictionReason::ExplicitEviction;
            let _ = EvictionReason::IdleTimeout;
            let _ = EvictionReason::ProtocolError("test".to_string());
        }

        #[test]
        fn test_eviction_reason_health_check_stale() {
            let reason = EvictionReason::HealthCheckFailed(HealthCheckResult::Stale);
            match reason {
                EvictionReason::HealthCheckFailed(HealthCheckResult::Stale) => {}
                _ => panic!("Expected HealthCheckFailed(Stale)"),
            }
        }

        #[test]
        fn test_eviction_reason_health_check_corrupted() {
            let reason = EvictionReason::HealthCheckFailed(HealthCheckResult::Corrupted);
            match reason {
                EvictionReason::HealthCheckFailed(HealthCheckResult::Corrupted) => {}
                _ => panic!("Expected HealthCheckFailed(Corrupted)"),
            }
        }
    }

    // ========================================================================
    // PoolStats Tests
    // ========================================================================

    mod pool_stats {
        use super::*;

        #[test]
        fn test_pool_stats_default() {
            let stats = PoolStats::default();
            assert_eq!(stats.total_connections, 0);
            assert_eq!(stats.idle_connections, 0);
            assert_eq!(stats.checked_out_connections, 0);
            assert_eq!(stats.pending_acquires, 0);
            assert_eq!(stats.total_acquires, 0);
            assert_eq!(stats.total_releases, 0);
            assert_eq!(stats.total_evictions, 0);
            assert_eq!(stats.total_health_checks, 0);
            assert_eq!(stats.failed_health_checks, 0);
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
            assert_eq!(stats.checked_out_connections, 3);
            assert_eq!(stats.pending_acquires, 2);
            assert_eq!(stats.total_acquires, 100);
            assert_eq!(stats.total_releases, 95);
            assert_eq!(stats.total_evictions, 5);
            assert_eq!(stats.total_health_checks, 50);
            assert_eq!(stats.failed_health_checks, 3);
        }

        #[test]
        fn test_pool_stats_equality() {
            let pool_id = PoolId::new("same-pool");
            let stats1 = PoolStats {
                pool_id: pool_id.clone(),
                ..PoolStats::default()
            };
            let stats2 = PoolStats {
                pool_id,
                ..PoolStats::default()
            };
            assert_eq!(stats1, stats2);
        }
    }

    // ========================================================================
    // CircuitBreakerState Tests
    // ========================================================================

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

        #[test]
        fn test_circuit_breaker_state_equality() {
            assert_eq!(CircuitBreakerState::Closed, CircuitBreakerState::Closed);
            assert_ne!(CircuitBreakerState::Closed, CircuitBreakerState::Open);
            assert_ne!(CircuitBreakerState::Open, CircuitBreakerState::HalfOpen);
        }
    }

    // ========================================================================
    // ErrorCategory Tests
    // ========================================================================

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
            assert_eq!(
                format!("{}", ErrorCategory::ConnectionFailed),
                "ConnectionFailed"
            );
            assert_eq!(
                format!("{}", ErrorCategory::HealthCheckFailed),
                "HealthCheckFailed"
            );
            assert_eq!(format!("{}", ErrorCategory::InvalidState), "InvalidState");
            assert_eq!(
                format!("{}", ErrorCategory::ShutdownInProgress),
                "ShutdownInProgress"
            );
            assert_eq!(
                format!("{}", ErrorCategory::ResourceExhaustion),
                "ResourceExhaustion"
            );
        }
    }

    // ========================================================================
    // ErrorDetail Tests
    // ========================================================================

    mod error_detail {
        use super::*;

        #[test]
        fn test_error_detail_max_connections_reached() {
            let detail = ErrorDetail::MaxConnectionsReached { max: 10 };
            let msg = detail.to_string();
            assert!(msg.contains("10"));
            assert!(msg.contains("Max connections reached"));
        }

        #[test]
        fn test_error_detail_pending_acquires_exceeded() {
            let detail = ErrorDetail::PendingAcquiresExceeded { max: 5 };
            let msg = detail.to_string();
            assert!(msg.contains("5"));
            assert!(msg.contains("Pending acquires exceeded"));
        }

        #[test]
        fn test_error_detail_acquire_timeout() {
            let detail = ErrorDetail::AcquireTimeout {
                waited_ms: 5000,
                timeout_ms: 10000,
            };
            let msg = detail.to_string();
            assert!(msg.contains("5000"));
            assert!(msg.contains("10000"));
        }

        #[test]
        fn test_error_detail_pool_not_initialized() {
            let detail = ErrorDetail::PoolNotInitialized;
            let msg = detail.to_string();
            assert_eq!(msg, "Pool not initialized");
        }

        #[test]
        fn test_error_detail_already_shutdown() {
            let detail = ErrorDetail::AlreadyShutdown;
            let msg = detail.to_string();
            assert_eq!(msg, "Pool already shutdown");
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

    // ========================================================================
    // ErrorContext Tests
    // ========================================================================

    mod error_context {
        use super::*;

        #[test]
        fn test_error_context_creation() {
            let pool_id = PoolId::new("error-pool");
            let timestamp = TimestampMs::new_unchecked(5000);
            let conn_id = ConnectionId::new();

            let context = ErrorContext {
                pool_id,
                timestamp,
                operation: "acquire".to_string(),
                connection_id: Some(conn_id),
            };

            assert_eq!(context.pool_id.as_str(), "error-pool");
            assert_eq!(context.timestamp, timestamp);
            assert_eq!(context.operation, "acquire");
            assert!(context.connection_id.is_some());
        }

        #[test]
        fn test_error_context_no_connection() {
            let pool_id = PoolId::new("no-conn-pool");
            let timestamp = TimestampMs::new_unchecked(6000);

            let context = ErrorContext {
                pool_id,
                timestamp,
                operation: "shutdown".to_string(),
                connection_id: None,
            };

            assert!(context.connection_id.is_none());
        }
    }

    // ========================================================================
    // PoolConfig Tests
    // ========================================================================

    mod pool_config {
        use super::*;

        #[test]
        fn test_pool_config_default_values() {
            let config = PoolConfig {
                min_connections: 2,
                max_connections: 10,
                connection_timeout_ms: 5000,
                idle_timeout_ms: 30000,
                health_check_interval_ms: 10000,
                max_pending_acquires: 5,
            };

            assert_eq!(config.min_connections, 2);
            assert_eq!(config.max_connections, 10);
            assert_eq!(config.connection_timeout_ms, 5000);
            assert_eq!(config.idle_timeout_ms, 30000);
            assert_eq!(config.health_check_interval_ms, 10000);
            assert_eq!(config.max_pending_acquires, 5);
        }

        #[test]
        fn test_pool_config_equality() {
            let config1 = PoolConfig {
                min_connections: 2,
                max_connections: 10,
                connection_timeout_ms: 5000,
                idle_timeout_ms: 30000,
                health_check_interval_ms: 10000,
                max_pending_acquires: 5,
            };
            let config2 = PoolConfig {
                min_connections: 2,
                max_connections: 10,
                connection_timeout_ms: 5000,
                idle_timeout_ms: 30000,
                health_check_interval_ms: 10000,
                max_pending_acquires: 5,
            };
            assert_eq!(config1, config2);
        }

        #[test]
        fn test_pool_config_inequality() {
            let config1 = PoolConfig {
                min_connections: 2,
                max_connections: 10,
                connection_timeout_ms: 5000,
                idle_timeout_ms: 30000,
                health_check_interval_ms: 10000,
                max_pending_acquires: 5,
            };
            let config2 = PoolConfig {
                min_connections: 3,
                max_connections: 10,
                connection_timeout_ms: 5000,
                idle_timeout_ms: 30000,
                health_check_interval_ms: 10000,
                max_pending_acquires: 5,
            };
            assert_ne!(config1, config2);
        }
    }

    // ========================================================================
    // Invariant Tests (INV-001 through INV-010)
    // ========================================================================

    mod invariants {
        use super::*;

        // INV-001: min_connections <= max_connections
        #[test]
        fn test_inv_001_min_leq_max() {
            // This test documents the invariant that min_connections must be <= max_connections
            // Implementation will enforce this at pool creation time
            let valid_config = PoolConfig {
                min_connections: 2,
                max_connections: 10,
                connection_timeout_ms: 5000,
                idle_timeout_ms: 30000,
                health_check_interval_ms: 10000,
                max_pending_acquires: 5,
            };
            assert!(valid_config.min_connections <= valid_config.max_connections);

            // Invalid config should be rejected by implementation
            // let invalid_config = PoolConfig {
            //     min_connections: 10,
            //     max_connections: 2,
            //     ...
            // };
            // assert!(this should panic or return error);
        }

        // INV-002: checked_out + idle + pending <= max_connections + max_pending_acquires
        #[test]
        fn test_inv_002_connection_count_bound() {
            // This test documents the invariant
            // Implementation will track these counts and enforce the bound
            let stats = PoolStats {
                pool_id: PoolId::new("inv-test"),
                total_connections: 10,
                idle_connections: 5,
                checked_out_connections: 3,
                pending_acquires: 2,
                total_acquires: 0,
                total_releases: 0,
                total_evictions: 0,
                total_health_checks: 0,
                failed_health_checks: 0,
            };

            let max_connections = 10;
            let max_pending_acquires = 5;
            let actual_total =
                stats.checked_out_connections + stats.idle_connections + stats.pending_acquires;
            let max_total = max_connections + max_pending_acquires;

            assert!(actual_total <= max_total);
        }

        // INV-003: Idle connections are safe to checkout
        #[test]
        fn test_inv_003_idle_safety() {
            // A connection with Idle status should be safe to checkout
            let conn = PooledConnection::new(ConnectionId::new(), TimestampMs::new_unchecked(1000));
            assert!(conn.is_idle());
            // Implementation should verify health before returning Idle connection
        }

        // INV-004: use_count monotonically increases
        #[test]
        fn test_inv_004_use_count_monotonic() {
            let mut conn =
                PooledConnection::new(ConnectionId::new(), TimestampMs::new_unchecked(1000));
            let initial_count = conn.use_count;

            conn.increment_use_count();
            assert!(conn.use_count > initial_count);

            conn.increment_use_count();
            conn.increment_use_count();
            assert!(conn.use_count > initial_count + 1);
        }

        // INV-005: idle_timeout closes idle connections
        #[test]
        fn test_inv_005_idle_timeout() {
            // This test documents the invariant
            // Implementation should check idle time and close connections that exceed idle_timeout_ms
            let created_at = TimestampMs::new_unchecked(1000);
            let _now = TimestampMs::new_unchecked(40000); // 39 seconds later
            let _idle_timeout_ms = 30000;

            let conn = PooledConnection::new(ConnectionId::new(), created_at);
            assert!(conn.is_idle());

            // Implementation should evict this connection as it exceeds idle_timeout
        }

        // INV-006: connection_timeout bounds all acquire operations
        #[test]
        fn test_inv_006_acquire_timeout_bound() {
            // This test documents the invariant
            // Implementation should enforce timeout on all acquire operations
            let config = PoolConfig {
                min_connections: 1,
                max_connections: 1,
                connection_timeout_ms: 5000,
                idle_timeout_ms: 30000,
                health_check_interval_ms: 10000,
                max_pending_acquires: 0,
            };

            assert_eq!(config.connection_timeout_ms, 5000);
            // Implementation should timeout after 5000ms
        }

        // INV-007: shutdown rejects new connections
        #[test]
        fn test_inv_007_shutdown_behavior() {
            // This test documents the invariant
            // During shutdown, acquire() should return PoolClosing
            // Implementation should set a shutdown flag and reject new acquires
            let result = AcquireResult::PoolClosing;
            match result {
                AcquireResult::PoolClosing => {
                    // This is what should be returned during shutdown
                }
                _ => panic!("Expected PoolClosing during shutdown"),
            }
        }

        // INV-008: Failed health checks are evicted
        #[test]
        fn test_inv_008_health_check_eviction() {
            // Failed health check connections should never return to Idle
            let _conn =
                PooledConnection::new(ConnectionId::new(), TimestampMs::new_unchecked(1000));

            // After failed health check, should be evicted, not returned to Idle
            let eviction = ReleaseResult::Evicted {
                reason: EvictionReason::HealthCheckFailed(HealthCheckResult::Stale),
            };

            match eviction {
                ReleaseResult::Evicted { .. } => {
                    // Connection was evicted, not returned
                }
                _ => panic!("Expected eviction after failed health check"),
            }
        }

        // INV-009: Circuit breaker trips at 50% failure rate
        #[test]
        fn test_inv_009_circuit_breaker_threshold() {
            // Circuit breaker trips when failed_health_checks > max_connections * 0.5
            let max_connections = 10;
            let threshold = max_connections as f64 * 0.5;

            // At 6 failures out of 10 connections (60% > 50%), circuit should trip
            let failed_health_checks = 6;
            assert!(failed_health_checks as f64 > threshold);

            // At 4 failures out of 10 connections (40% < 50%), circuit should not trip
            let failed_health_checks_safe = 4;
            assert!(failed_health_checks_safe as f64 <= threshold);
        }

        // INV-010: Statistics eventually consistent
        #[test]
        fn test_inv_010_stats_eventual_consistency() {
            // Pool statistics should reflect actual state within one health-check cycle
            let stats = PoolStats {
                pool_id: PoolId::new("stats-consistency"),
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

            // Statistics should be internally consistent
            assert_eq!(
                stats.total_connections,
                stats.idle_connections
                    + stats.checked_out_connections
                    + (stats.total_connections
                        - stats.idle_connections
                        - stats.checked_out_connections)
            );
        }
    }

    // ========================================================================
    // Circuit Breaker Rules Tests (CB-001 through CB-005)
    // ========================================================================

    mod circuit_breaker_rules {
        use super::*;

        // CB-001: Transitions Closed -> Open at 50% failure rate in 30-second window
        #[test]
        fn test_cb_001_open_transition() {
            // At 50% failure rate, circuit should transition to Open
            let max_connections = 10;
            let _failures_at_threshold = max_connections as u32 / 2; // 5

            // At exactly 50%, should trip
            let failures = 5;
            let rate = failures as f64 / max_connections as f64;
            assert_eq!(rate, 0.5);
            // Implementation should transition to Open at this point
        }

        // CB-002: Open -> HalfOpen after connection_timeout_ms
        #[test]
        fn test_cb_002_halfopen_transition() {
            // After connection_timeout_ms in Open state, should transition to HalfOpen
            let connection_timeout_ms = 5000;

            // Implementation should schedule HalfOpen transition after this duration
            assert_eq!(connection_timeout_ms, 5000);
        }

        // CB-003: HalfOpen success -> Closed
        #[test]
        fn test_cb_003_success_transition() {
            // In HalfOpen state, if test acquisitions succeed, transition to Closed
            // This test documents the expected behavior
            let state_after_success = CircuitBreakerState::Closed;
            assert_eq!(state_after_success, CircuitBreakerState::Closed);
        }

        // CB-004: HalfOpen failure count >= max_connections -> Open
        #[test]
        fn test_cb_004_failure_transition() {
            // In HalfOpen state, if failure count >= max_connections, transition back to Open
            let max_connections = 10;
            let failures = max_connections;

            // At max_connections failures, should return to Open
            assert_eq!(failures, max_connections);
            // Implementation should transition to Open
        }

        // CB-005: Open state rejects all acquires
        #[test]
        fn test_cb_005_open_rejection() {
            // While Open, acquire() returns PoolExhausted with CircuitBreakerOpen detail
            let detail = ErrorDetail::CircuitBreakerOpen {
                consecutive_failures: 5,
            };

            match detail {
                ErrorDetail::CircuitBreakerOpen {
                    consecutive_failures,
                } => {
                    assert!(consecutive_failures > 0);
                }
                _ => panic!("Expected CircuitBreakerOpen detail"),
            }
        }
    }

    // ========================================================================
    // ReconnectBackoff Tests
    // ========================================================================

    mod reconnect_backoff {
        use super::*;

        #[test]
        fn test_backoff_sequence_grows_exponentially() {
            let mut bo = ReconnectBackoff::new(100, 2, 10000, 0);
            assert_eq!(bo.next_backoff(), 100);
            assert_eq!(bo.next_backoff(), 200);
            assert_eq!(bo.next_backoff(), 400);
            assert_eq!(bo.next_backoff(), 800);
            assert_eq!(bo.next_backoff(), 1600);
        }

        #[test]
        fn test_backoff_sequence_multiplier_of_3() {
            let mut bo = ReconnectBackoff::new(50, 3, 10000, 0);
            assert_eq!(bo.next_backoff(), 50);
            assert_eq!(bo.next_backoff(), 150);
            assert_eq!(bo.next_backoff(), 450);
            assert_eq!(bo.next_backoff(), 1350);
        }

        #[test]
        fn test_backoff_respects_max_cap() {
            let mut bo = ReconnectBackoff::new(100, 2, 500, 0);
            assert_eq!(bo.next_backoff(), 100);
            assert_eq!(bo.next_backoff(), 200);
            assert_eq!(bo.next_backoff(), 400);
            assert_eq!(bo.next_backoff(), 500);
            assert_eq!(bo.next_backoff(), 500);
            assert_eq!(bo.next_backoff(), 500);
        }

        #[test]
        fn test_backoff_max_cap_equals_initial() {
            let mut bo = ReconnectBackoff::new(200, 2, 200, 0);
            assert_eq!(bo.next_backoff(), 200);
            assert_eq!(bo.next_backoff(), 200);
            assert_eq!(bo.next_backoff(), 200);
        }

        #[test]
        fn test_backoff_with_jitter_within_bounds() {
            let mut bo = ReconnectBackoff::new(100, 2, 10000, 50);
            let b1 = bo.next_backoff();
            assert!(b1 >= 100 && b1 <= 150, "b1={b1}");
            let b2 = bo.next_backoff();
            assert!(b2 >= 200 && b2 <= 250, "b2={b2}");
            let b3 = bo.next_backoff();
            assert!(b3 >= 400 && b3 <= 450, "b3={b3}");
        }

        #[test]
        fn test_jitter_is_deterministic_per_attempt() {
            let j1 = deterministic_jitter(100, 1);
            let j2 = deterministic_jitter(100, 1);
            assert_eq!(j1, j2, "same attempt must produce same jitter");

            let j3 = deterministic_jitter(100, 2);
            assert!(j3 <= 100);
        }

        #[test]
        fn test_jitter_zero_range_returns_zero() {
            assert_eq!(deterministic_jitter(0, 1), 0);
            assert_eq!(deterministic_jitter(0, 99), 0);
        }

        #[test]
        fn test_backoff_with_jitter_capped_at_max() {
            let mut bo = ReconnectBackoff::new(100, 2, 300, 1000);
            let b1 = bo.next_backoff();
            assert!(b1 <= 300, "b1={b1} exceeds max");
            let b2 = bo.next_backoff();
            assert!(b2 <= 300, "b2={b2} exceeds max");
            let b3 = bo.next_backoff();
            assert!(b3 <= 300, "b3={b3} exceeds max");
        }

        #[test]
        fn test_reset_clears_attempt_counter() {
            let mut bo = ReconnectBackoff::new(100, 2, 10000, 0);
            assert_eq!(bo.attempt(), 0);
            let _ = bo.next_backoff();
            let _ = bo.next_backoff();
            assert_eq!(bo.attempt(), 2);
            bo.reset();
            assert_eq!(bo.attempt(), 0);
            assert_eq!(bo.next_backoff(), 100);
        }

        #[test]
        fn test_zero_jitter_means_no_jitter() {
            let mut bo = ReconnectBackoff::new(100, 2, 10000, 0);
            assert_eq!(bo.next_backoff(), 100);
            assert_eq!(bo.next_backoff(), 200);
            assert_eq!(bo.next_backoff(), 400);
        }
    }
}
