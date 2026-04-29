//! Core type definitions for the connection pool.

#![allow(
    dead_code,
    clippy::inherent_to_string,
    clippy::inherent_to_string_shadow_display,
    clippy::wrong_self_convention
)]

use std::fmt;

use ulid::Ulid;

use crate::pool::TimestampMs;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthCheckResult {
    Healthy,
    Stale,
    Corrupted,
    Timeout,
}

/// Handle for a pending acquire request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitHandle {
    pub request_id: u64,
    pub enqueued_at: TimestampMs,
    pub pool_id: PoolId,
}

/// Result of attempting to acquire a connection from the pool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcquireResult {
    Available { connection: PooledConnection },
    Pending { wait_handle: WaitHandle },
    PoolExhausted { config: PoolConfig },
    PoolClosing,
    Timeout { waited_ms: u64 },
}

/// Result of releasing a connection back to the pool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseResult {
    Returned,
    AlreadyClosed,
    Evicted { reason: EvictionReason },
}

/// Reason for connection eviction.
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CircuitBreakerState {
    #[default]
    Closed,
    Open,
    HalfOpen,
}
