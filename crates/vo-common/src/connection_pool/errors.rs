//! Error types for the connection pool.

#![allow(clippy::inherent_to_string)]

use std::fmt;

use crate::pool::TimestampMs;

use super::types::{ConnectionId, PoolId};

/// Error category for connection pool errors.
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

/// Error detail for connection pool errors.
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

/// Context for connection pool errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorContext {
    pub pool_id: PoolId,
    pub timestamp: TimestampMs,
    pub operation: &'static str,
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
