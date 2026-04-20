//! Connection Pool Implementation
//!
//! This module provides the core connection pool implementation for managing
//! NATS client connections in the veloxide distributed worker system.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{mpsc, OwnedSemaphorePermit, Semaphore};
use tracing::{debug, error, info, warn};

use vo_types::connection_pool::{
    AcquireResult, CircuitBreakerState, ConnectionId, ConnectionOwnerId, ConnectionPoolError,
    ConnectionStatus, ErrorCategory, ErrorContext, ErrorDetail, EvictionReason,
    HealthCheckResult, PoolConfig as VoPoolConfig, PoolId, PoolStats, PooledConnection,
    ReleaseResult, WaitHandle,
};
use vo_types::integer_types::TimestampMs;

use super::circuit_breaker::CircuitBreaker;
use super::config::{PoolConfig, PoolConfigError};
use super::health_check::{determine_health_check_result, HealthCheck};

#[derive(Debug, Clone)]
pub struct NatsConnectionWrapper {
    pub connection_id: ConnectionId,
    pub server_url: String,
    pub status: ConnectionStatus,
}

impl NatsConnectionWrapper {
    pub fn new(connection_id: ConnectionId, server_url: impl Into<String>) -> Self {
        Self {
            connection_id,
            server_url: server_url.into(),
            status: ConnectionStatus::Idle,
        }
    }

    pub fn with_status(mut self, status: ConnectionStatus) -> Self {
        self.status = status;
        self
    }

    pub fn is_healthy(&self) -> bool {
        self.status != ConnectionStatus::Closed && self.status != ConnectionStatus::Closing
    }
}

#[derive(Debug, Clone)]
pub struct PoolState {
    pub pool_id: PoolId,
    pub connections: HashMap<ConnectionId, PooledConnection>,
    pub idle_connections: VecDeque<ConnectionId>,
    pub checked_out_connections: HashMap<ConnectionId, (ConnectionId, ConnectionOwnerId)>,
    pub pending_acquires: VecDeque<WaitHandle>,
    pub config: PoolConfig,
    pub circuit_breaker: CircuitBreaker,
    pub health_check: HealthCheck,
    pub total_acquires: u64,
    pub total_releases: u64,
    pub total_evictions: u64,
    pub total_health_checks: u64,
    pub failed_health_checks: u64,
    pub is_shutting_down: bool,
}

impl PoolState {
    pub fn new(pool_id: PoolId, config: PoolConfig) -> Self {
        Self {
            pool_id,
            connections: HashMap::new(),
            idle_connections: VecDeque::new(),
            checked_out_connections: HashMap::new(),
            pending_acquires: VecDeque::new(),
            circuit_breaker: CircuitBreaker::new(),
            health_check: HealthCheck::new(config.health_check_interval_ms),
            config,
            total_acquires: 0,
            total_releases: 0,
            total_evictions: 0,
            total_health_checks: 0,
            failed_health_checks: 0,
            is_shutting_down: false,
        }
    }

    pub fn stats(&self) -> PoolStats {
        PoolStats {
            pool_id: self.pool_id.clone(),
            total_connections: self.connections.len() as u32,
            idle_connections: self.idle_connections.len() as u32,
            checked_out_connections: self.checked_out_connections.len() as u32,
            pending_acquires: self.pending_acquires.len() as u32,
            total_acquires: self.total_acquires,
            total_releases: self.total_releases,
            total_evictions: self.total_evictions,
            total_health_checks: self.total_health_checks,
            failed_health_checks: self.failed_health_checks,
        }
    }

    pub fn idle_count(&self) -> u32 {
        self.idle_connections.len() as u32
    }

    pub fn checked_out_count(&self) -> u32 {
        self.checked_out_connections.len() as u32
    }

    pub fn total_connections(&self) -> u32 {
        self.connections.len() as u32
    }

    pub fn can_acquire(&self) -> bool {
        !self.is_shutting_down
            && self.circuit_breaker.should_allow_request()
            && self.idle_count() > 0
    }

    pub fn should_create_connection(&self) -> bool {
        self.total_connections() < self.config.max_connections
            && self.idle_count() == 0
            && !self.is_shutting_down
    }
}

#[derive(Debug)]
pub struct ConnectionPool {
    pool_id: PoolId,
    state: PoolState,
    nats_urls: Vec<String>,
    connection_semaphore: Arc<Semaphore>,
    wait_semaphore: Arc<Semaphore>,
}

impl ConnectionPool {
    pub fn new(pool_id: PoolId, nats_urls: Vec<String>, config: PoolConfig) -> Self {
        let vo_config: VoPoolConfig = config.clone().into();
        if let Err(e) = validate_config(&vo_config) {
            panic!("Invalid pool config: {:?}", e);
        }

        let state = PoolState::new(pool_id.clone(), config);

        Self {
            pool_id: pool_id.clone(),
            state,
            nats_urls,
            connection_semaphore: Arc::new(Semaphore::new(0)),
            wait_semaphore: Arc::new(Semaphore::new(0)),
        }
    }

    pub fn with_config(pool_id: PoolId, nats_urls: Vec<String>, config: PoolConfig) -> Self {
        Self::new(pool_id, nats_urls, config)
    }

    pub async fn acquire(&mut self, owner_id: ConnectionOwnerId) -> AcquireResult {
        self.acquire_with_timeout(
            std::time::Duration::from_millis(self.state.config.connection_timeout_ms),
            owner_id,
        )
        .await
    }

    #[allow(clippy::unused_async)]
    pub async fn acquire_with_timeout(
        &mut self,
        timeout: std::time::Duration,
        owner_id: ConnectionOwnerId,
    ) -> AcquireResult {
        if self.state.is_shutting_down {
            return AcquireResult::PoolClosing;
        }

        if !self.state.circuit_breaker.should_allow_request() {
            let error = ConnectionPoolError {
                category: ErrorCategory::PoolExhaustion,
                detail: ErrorDetail::CircuitBreakerOpen {
                    consecutive_failures: self.state.circuit_breaker.consecutive_failures(),
                },
                context: ErrorContext {
                    pool_id: self.pool_id.clone(),
                    timestamp: TimestampMs::now(),
                    operation: "acquire",
                    connection_id: None,
                },
            };
            error!("Circuit breaker open: {:?}", error);
            return AcquireResult::PoolExhausted {
                config: self.state.config.clone().into(),
            };
        }

// Try idle connections with health check loop: evict stale/unhealthy
        // and keep trying until a healthy one is found or pool is exhausted.
        while let Some(conn_id) = self.state.idle_connections.pop_front() {
            if !self.health_check_connection(conn_id) {
                debug!(
                    "Evicted unhealthy idle connection {} from pool {}",
                    conn_id, self.pool_id
                );
                continue;
            }

            if let Some(mut conn) = self.state.connections.get_mut(&conn_id) {
                if self.health_check_connection(conn_id) {
                    conn.status = ConnectionStatus::CheckedOut;
                    conn.increment_use_count();
                    self.state.total_acquires += 1;

                    let checkout_id = ConnectionId::new();
                    self.state
                        .checked_out_connections
                        .insert(checkout_id, (conn_id, owner_id));

                    debug!("Acquired connection {} from pool {}", conn_id, self.pool_id);

                    return AcquireResult::Available {
                        connection: conn.clone(),
                    };
                }
            }
        }

        if self.state.total_connections() < self.state.config.max_connections {
            let connection_id = ConnectionId::new();
            let now = TimestampMs::now();

            let pooled =
                PooledConnection::new(connection_id, now).with_status(ConnectionStatus::CheckedOut);

            self.state.connections.insert(connection_id, pooled.clone());

            let checkout_id = ConnectionId::new();
            self.state
                .checked_out_connections
                .insert(checkout_id, (connection_id, owner_id));

            self.state.total_acquires += 1;

            debug!(
                "Created and acquired new connection {} in pool {}",
                connection_id, self.pool_id
            );

            return AcquireResult::Available { connection: pooled };
        }

        if self.state.pending_acquires.len() >= self.state.config.max_pending_acquires as usize {
            let error = ConnectionPoolError {
                category: ErrorCategory::PoolExhaustion,
                detail: ErrorDetail::PendingAcquiresExceeded {
                    max: self.state.config.max_pending_acquires,
                },
                context: ErrorContext {
                    pool_id: self.pool_id.clone(),
                    timestamp: TimestampMs::now(),
                    operation: "acquire",
                    connection_id: None,
                },
            };
            warn!("Pool exhausted: {:?}", error);
            return AcquireResult::PoolExhausted {
                config: self.state.config.clone().into(),
            };
        }

        let wait_handle = WaitHandle {
            request_id: self.state.pending_acquires.len() as u64 + 1,
            enqueued_at: TimestampMs::now(),
            pool_id: self.pool_id.clone(),
        };
        self.state.pending_acquires.push_back(wait_handle.clone());

        AcquireResult::Pending { wait_handle }
    }

    pub fn release(&mut self, connection_id: ConnectionId, owner_id: ConnectionOwnerId) -> ReleaseResult {
        if self.state.is_shutting_down {
            return self.evict_connection(connection_id, EvictionReason::ExplicitEviction);
        }

        let checkout_entry = self
            .state
            .checked_out_connections
            .iter()
            .find(|&(_, (cid, _))| *cid == connection_id)
            .map(|(id, (_, owner))| (*id, owner.clone()));

        let Some((checkout_id, stored_owner)) = checkout_entry else {
            return ReleaseResult::AlreadyClosed;
        };

        if stored_owner != owner_id {
            debug!(
                "Release denied: connection {} owned by {} but released by {}",
                connection_id, stored_owner, owner_id
            );
            return ReleaseResult::NotOwner;
        }

        self.state.checked_out_connections.remove(&checkout_id);

        if let Some(conn) = self.state.connections.get_mut(&connection_id) {
            conn.status = ConnectionStatus::Idle;

            self.state.idle_connections.push_back(connection_id);
            self.state.total_releases += 1;

            debug!(
                "Released connection {} back to pool {}",
                connection_id, self.pool_id
            );
            return ReleaseResult::Returned;
        }

        ReleaseResult::AlreadyClosed
    }

    pub fn evict_connection(
        &mut self,
        connection_id: ConnectionId,
        reason: EvictionReason,
    ) -> ReleaseResult {
        self.state
            .checked_out_connections
            .retain(|_, (cid, _)| *cid != connection_id);

        if let Some(conn) = self.state.connections.get_mut(&connection_id) {
            conn.status = ConnectionStatus::Closed;
            self.state.total_evictions += 1;

            self.state
                .idle_connections
                .retain(|id| *id != connection_id);
            self.state.connections.remove(&connection_id);

            debug!(
                "Evicted connection {} from pool {}: {:?}",
                connection_id, self.pool_id, reason
            );

            if matches!(reason, EvictionReason::HealthCheckFailed(_)) {
                self.state.circuit_breaker.record_failure();
            }

            return ReleaseResult::Evicted { reason };
        }

        ReleaseResult::AlreadyClosed
    }

    pub fn stats(&self) -> PoolStats {
        self.state.stats()
    }

    pub fn health_check_connection(&mut self, connection_id: ConnectionId) -> bool {
        if let Some(conn) = self.state.connections.get(&connection_id) {
            let current_time = TimestampMs::now();
            let result = self.state.health_check.check_connection(
                conn.last_used_at,
                self.state.config.idle_timeout_ms,
                current_time,
            );

            self.state.total_health_checks += 1;

            if result != HealthCheckResult::Healthy {
                self.state.failed_health_checks += 1;
                let evict_reason = match result {
                    HealthCheckResult::Stale => EvictionReason::IdleTimeout,
                    HealthCheckResult::Timeout => {
                        EvictionReason::HealthCheckFailed(HealthCheckResult::Timeout)
                    }
                    HealthCheckResult::Corrupted => {
                        EvictionReason::HealthCheckFailed(HealthCheckResult::Corrupted)
                    }
                    HealthCheckResult::Healthy => EvictionReason::IdleTimeout,
                };
                self.evict_connection(connection_id, evict_reason);
                return false;
            }
        }
        true
    }

    pub fn circuit_breaker_state(&self) -> CircuitBreakerState {
        self.state.circuit_breaker.state()
    }

    pub fn shutdown(&mut self) {
        info!("Shutting down connection pool {}", self.pool_id);
        self.state.is_shutting_down = true;

        self.state.idle_connections.clear();

        let connection_ids: Vec<ConnectionId> = self.state.connections.keys().copied().collect();
        for conn_id in connection_ids {
            self.evict_connection(conn_id, EvictionReason::ExplicitEviction);
        }

        self.state.connections.clear();
        self.state.checked_out_connections.clear();
        self.state.pending_acquires.clear();
    }

    pub fn pool_id(&self) -> &PoolId {
        &self.pool_id
    }

    pub fn is_shutting_down(&self) -> bool {
        self.state.is_shutting_down
    }
}

fn validate_config(config: &VoPoolConfig) -> Result<(), PoolConfigError> {
    if config.min_connections > config.max_connections {
        return Err(PoolConfigError::MinGreaterThanMax);
    }
    if config.max_connections == 0 {
        return Err(PoolConfigError::MaxZero);
    }
    if config.connection_timeout_ms == 0 {
        return Err(PoolConfigError::ConnectionTimeoutZero);
    }
    if config.idle_timeout_ms == 0 {
        return Err(PoolConfigError::IdleTimeoutZero);
    }
    if config.health_check_interval_ms == 0 {
        return Err(PoolConfigError::HealthCheckIntervalZero);
    }
    Ok(())
}

#[cfg(test)]
mod pool_tests {
    use super::*;

    fn create_test_pool() -> ConnectionPool {
        let pool_id = PoolId::new("test-pool");
        let nats_urls = vec!["nats://localhost:4222".to_string()];
        let config = PoolConfig::new(1, 5, 5000, 30000, 10000, 10).unwrap();
        ConnectionPool::new(pool_id, nats_urls, config)
    }

    const TEST_OWNER: ConnectionOwnerId = ConnectionOwnerId::new("test-owner");

    fn owner_id(s: &str) -> ConnectionOwnerId {
        ConnectionOwnerId::new(s)
    }

    #[test]
    fn test_pool_initialization() {
        let pool = create_test_pool();
        assert_eq!(pool.stats().total_connections, 0);
        assert_eq!(pool.stats().idle_connections, 0);
        assert!(!pool.is_shutting_down());
    }

    #[test]
    fn test_acquire_creates_connection() {
        let mut pool = create_test_pool();
        let result = futures::executor::block_on(pool.acquire(TEST_OWNER.clone()));
        match result {
            AcquireResult::Available { connection } => {
                assert!(connection.is_checked_out());
            }
            _ => panic!("Expected Available result"),
        }
    }

    #[test]
    fn test_release_returns_connection_to_pool() {
        let mut pool = create_test_pool();
        let acquire_result = futures::executor::block_on(pool.acquire(TEST_OWNER.clone()));
        let conn_id = match acquire_result {
            AcquireResult::Available { connection } => connection.connection_id,
            _ => panic!("Expected Available result"),
        };

        let release_result = pool.release(conn_id, TEST_OWNER.clone());
        assert_eq!(release_result, ReleaseResult::Returned);
        assert_eq!(pool.stats().idle_connections, 1);
    }

    #[test]
    fn test_release_unknown_connection() {
        let mut pool = create_test_pool();
        let unknown_id = ConnectionId::new();
        let result = pool.release(unknown_id, TEST_OWNER.clone());
        assert_eq!(result, ReleaseResult::AlreadyClosed);
    }

    #[test]
    fn test_pool_stats() {
        let mut pool = create_test_pool();
        let acquire_result = futures::executor::block_on(pool.acquire(TEST_OWNER.clone()));
        let conn_id = match acquire_result {
            AcquireResult::Available { connection } => connection.connection_id,
            _ => panic!("Expected Available result"),
        };

        let stats = pool.stats();
        assert_eq!(stats.total_connections, 1);
        assert_eq!(stats.checked_out_connections, 1);
        assert_eq!(stats.idle_connections, 0);

        pool.release(conn_id, TEST_OWNER.clone());

        let stats = pool.stats();
        assert_eq!(stats.total_connections, 1);
        assert_eq!(stats.checked_out_connections, 0);
        assert_eq!(stats.idle_connections, 1);
    }

    #[test]
    fn test_shutdown_clears_pool() {
        let mut pool = create_test_pool();
        futures::executor::block_on(pool.acquire(TEST_OWNER.clone()));
        pool.shutdown();

        assert!(pool.is_shutting_down());
        assert_eq!(pool.stats().total_connections, 0);
        assert_eq!(pool.stats().idle_connections, 0);
    }

    #[tokio::test]
    async fn test_acquire_respects_circuit_breaker() {
        let pool_id = PoolId::new("cb-test-pool");
        let nats_urls = vec!["nats://localhost:4222".to_string()];
        let config = PoolConfig::new(1, 1, 5000, 30000, 10000, 10).unwrap();
        let mut pool = ConnectionPool::new(pool_id, nats_urls, config);

        pool.state
            .circuit_breaker
            .transition_to(CircuitBreakerState::Open);

        let result = pool.acquire(TEST_OWNER.clone()).await;
        match result {
            AcquireResult::PoolExhausted { .. } => {}
            _ => panic!("Expected PoolExhausted when circuit breaker is open"),
        }
    }

    #[test]
    fn test_release_twice_returns_already_closed() {
        let mut pool = create_test_pool();
        let acquire_result = futures::executor::block_on(pool.acquire(TEST_OWNER.clone()));
        let conn_id = match acquire_result {
            AcquireResult::Available { connection } => connection.connection_id,
            _ => panic!("Expected Available result"),
        };

        let first_release = pool.release(conn_id, TEST_OWNER.clone());
        assert_eq!(first_release, ReleaseResult::Returned);

        let second_release = pool.release(conn_id, TEST_OWNER.clone());
        assert_eq!(second_release, ReleaseResult::AlreadyClosed);
    }

    #[test]
    fn test_release_without_acquire_returns_already_closed() {
        let mut pool = create_test_pool();
        let never_acquired = ConnectionId::new();
        let result = pool.release(never_acquired, TEST_OWNER.clone());
        assert_eq!(result, ReleaseResult::AlreadyClosed);
    }

    // ========================================================================
    // Health Check on Acquire Tests (ve-hypnb)
    // ========================================================================

    /// Given: A pool with a healthy idle connection
    /// When: acquire() is called
    /// Then: The connection passes health check and is returned
    #[test]
    fn test_acquire_healthy_connection() {
        let mut pool = create_test_pool();

        // Create a connection and release it to idle
        let result = futures::executor::block_on(pool.acquire(TEST_OWNER.clone()));
        let conn_id = match result {
            AcquireResult::Available { connection } => connection.connection_id,
            _ => panic!("Expected Available"),
        };
        pool.release(conn_id, TEST_OWNER.clone());
        assert_eq!(pool.stats().idle_connections, 1);

        // Acquire again — connection should be healthy and returned
        let result = futures::executor::block_on(pool.acquire(TEST_OWNER.clone()));
        match result {
            AcquireResult::Available { connection } => {
                assert_eq!(connection.connection_id, conn_id);
            }
            _ => panic!("Expected Available for healthy idle connection"),
        }
    }

    /// Given: A pool with a stale idle connection (last_used_at far in the past)
    /// When: acquire() is called
    /// Then: The stale connection is evicted and a new one is created
    #[test]
    fn test_acquire_stale_connection_evicted() {
        let mut pool = create_test_pool();

        // Create a connection and release it to idle
        let result = futures::executor::block_on(pool.acquire(TEST_OWNER.clone()));
        let conn_id = match result {
            AcquireResult::Available { connection } => connection.connection_id,
            _ => panic!("Expected Available"),
        };
        pool.release(conn_id, TEST_OWNER.clone());
        assert_eq!(pool.stats().idle_connections, 1);

        // Artificially age the connection to make it stale
        // idle_timeout_ms is 30000, so setting last_used_at 60000ms in the past
        let stale_time = TimestampMs::new_unchecked(TimestampMs::now().as_u64().saturating_sub(60_000));
        if let Some(conn) = pool.state.connections.get_mut(&conn_id) {
            conn.last_used_at = stale_time;
        }

        // Acquire should evict the stale connection and create a new one
        let result = futures::executor::block_on(pool.acquire(TEST_OWNER.clone()));
        match result {
            AcquireResult::Available { connection } => {
                // New connection, different from the stale one
                assert_ne!(connection.connection_id, conn_id);
            }
            _ => panic!("Expected Available after evicting stale connection"),
        }
    }

    /// Given: A pool where all connections are stale
    /// When: acquire() is called multiple times
    /// Then: All stale connections are evicted, new ones created up to max
    #[test]
    fn test_acquire_connection_failure_all_stale() {
        let pool_id = PoolId::new("stale-test-pool");
        let nats_urls = vec!["nats://localhost:4222".to_string()];
        // max_connections=2, idle_timeout_ms=30000
        let config = PoolConfig::new(1, 2, 5000, 30000, 10000, 10).unwrap();
        let mut pool = ConnectionPool::new(pool_id, nats_urls, config);

        // Create 2 connections and release them
        let mut conn_ids = Vec::new();
        for _ in 0..2 {
            let result = futures::executor::block_on(pool.acquire(TEST_OWNER.clone()));
            if let AcquireResult::Available { connection } = result {
                conn_ids.push(connection.connection_id);
            }
        }
        for id in &conn_ids {
            pool.release(*id, TEST_OWNER.clone());
        }
        assert_eq!(pool.stats().idle_connections, 2);

        // Age all connections to be stale
        let stale_time = TimestampMs::new_unchecked(TimestampMs::now().as_u64().saturating_sub(60_000));
        for id in &conn_ids {
            if let Some(conn) = pool.state.connections.get_mut(id) {
                conn.last_used_at = stale_time;
            }
        }

        // First acquire: evicts all stale, creates new
        let result = futures::executor::block_on(pool.acquire(TEST_OWNER.clone()));
        match result {
            AcquireResult::Available { connection } => {
                assert!(!conn_ids.contains(&connection.connection_id));
            }
            _ => panic!("Expected Available"),
        }
    }

    // ========================================================================
    // Connection Theft Attack Tests (bh-014)
    // ========================================================================

    /// Given: Actor A acquires a connection
    /// When: Actor B tries to release Actor A's connection
    /// Then: The release is denied with NotOwner result
    #[test]
    fn test_connection_theft_blocked() {
        let mut pool = create_test_pool();
        let actor_a = owner_id("actor-a");
        let actor_b = owner_id("actor-b");

        // Actor A acquires a connection
        let acquire_result = futures::executor::block_on(pool.acquire(actor_a.clone()));
        let conn_id = match acquire_result {
            AcquireResult::Available { connection } => connection.connection_id,
            _ => panic!("Expected Available result"),
        };

        // Actor B tries to release Actor A's connection - should be denied
        let release_result = pool.release(conn_id, actor_b);
        assert_eq!(release_result, ReleaseResult::NotOwner);

        // Verify the connection is still checked out to Actor A
        assert_eq!(pool.stats().checked_out_connections, 1);
        assert_eq!(pool.stats().idle_connections, 0);
    }

    /// Given: Actor A acquires a connection and releases it properly
    /// When: Actor B tries to release the now-idle connection
    /// Then: The release is denied because the connection is not checked out to anyone
    #[test]
    fn test_connection_theft_on_idle_connection() {
        let mut pool = create_test_pool();
        let actor_a = owner_id("actor-a");
        let actor_b = owner_id("actor-b");

        // Actor A acquires and releases properly
        let acquire_result = futures::executor::block_on(pool.acquire(actor_a.clone()));
        let conn_id = match acquire_result {
            AcquireResult::Available { connection } => connection.connection_id,
            _ => panic!("Expected Available result"),
        };
        pool.release(conn_id, actor_a.clone());
        assert_eq!(pool.stats().idle_connections, 1);

        // Actor B tries to release the idle connection - should be denied
        let release_result = pool.release(conn_id, actor_b);
        assert_eq!(release_result, ReleaseResult::AlreadyClosed);

        // Verify the connection is still idle
        assert_eq!(pool.stats().idle_connections, 1);
    }

    /// Given: Multiple actors acquire connections
    /// When: Each actor releases their own connection
    /// Then: All releases succeed and connections return to idle
    #[test]
    fn test_actor_isolation_correct_ownership() {
        let mut pool = create_test_pool();
        let actor_a = owner_id("actor-a");
        let actor_b = owner_id("actor-b");

        // Actor A acquires
        let acquire_a = futures::executor::block_on(pool.acquire(actor_a.clone()));
        let conn_id_a = match acquire_a {
            AcquireResult::Available { connection } => connection.connection_id,
            _ => panic!("Expected Available for actor A"),
        };

        // Actor B acquires
        let acquire_b = futures::executor::block_on(pool.acquire(actor_b.clone()));
        let conn_id_b = match acquire_b {
            AcquireResult::Available { connection } => connection.connection_id,
            _ => panic!("Expected Available for actor B"),
        };

        assert_eq!(pool.stats().checked_out_connections, 2);

        // Actor A releases their own connection - should succeed
        let release_a = pool.release(conn_id_a, actor_a.clone());
        assert_eq!(release_a, ReleaseResult::Returned);

        // Actor B releases their own connection - should succeed
        let release_b = pool.release(conn_id_b, actor_b);
        assert_eq!(release_b, ReleaseResult::Returned);

        assert_eq!(pool.stats().idle_connections, 2);
        assert_eq!(pool.stats().checked_out_connections, 0);
    }

    // ========================================================================
    // BLACKHAT: Connection Exhaustion Attack Tests (bh-013 / ve-cdbov)
    // ========================================================================
    //
    // Attack scenario: A malicious actor acquires connections and never releases
    // them, attempting to exhaust the pool and deny service to all other callers.
    //
    // Invariants under test:
    //   INV-EXH-01: Pool count bounded by max_connections at all times
    //   INV-EXH-02: Once exhausted, new requests MUST return PoolExhausted
    //   INV-EXH-03: Normal acquisition still works when pool has capacity
    //   INV-EXH-04: Exhaustion + pending queue overflow returns PoolExhausted
    //   INV-EXH-05: Released connections become available after exhaustion
    //   INV-EXH-06: Pool stats accurately reflect exhaustion state
    //   INV-EXH-07: Shutdown during exhaustion evicts all held connections
    //   INV-EXH-08: No connection leak when acquiring past max (all PoolExhausted)

    fn create_tiny_pool(max: u32, max_pending: u32) -> ConnectionPool {
        let pool_id = PoolId::new("exhaustion-attack-pool");
        let nats_urls = vec!["nats://localhost:4222".to_string()];
        let config = PoolConfig::new(1, max, 5000, 30000, 10000, max_pending).unwrap();
        ConnectionPool::new(pool_id, nats_urls, config)
    }

    fn attacker_id() -> ConnectionOwnerId {
        ConnectionOwnerId::new("malicious-actor")
    }

    fn victim_id() -> ConnectionOwnerId {
        ConnectionOwnerId::new("legitimate-actor")
    }

    /// INV-EXH-03: Normal acquisition works when pool has capacity.
    /// Given: A pool with max_connections=3
    /// When: Acquiring 3 connections (happy path)
    /// Then: All succeed with AcquireResult::Available
    #[test]
    fn bh_normal_acquire_works_within_capacity() {
        let mut pool = create_tiny_pool(3, 5);
        let mut acquired = Vec::new();

        for i in 0..3 {
            let result = futures::executor::block_on(pool.acquire(attacker_id()));
            match result {
                AcquireResult::Available { connection } => {
                    acquired.push(connection.connection_id);
                }
                other => panic!("Acquire #{} should succeed, got {:?}", i + 1, other),
            }
        }

        assert_eq!(pool.stats().checked_out_connections, 3);
        assert_eq!(pool.stats().total_connections, 3);
        assert_eq!(pool.stats().idle_connections, 0);

        for id in acquired {
            assert_eq!(pool.release(id, attacker_id()), ReleaseResult::Returned);
        }
    }

    /// INV-EXH-01 + INV-EXH-02: Actor holds all connections; next acquire
    /// gets PoolExhausted (after pending queue also fills).
    /// Given: A pool with max_connections=2, max_pending_acquires=2
    /// When: Malicious actor acquires 2 connections, never releases
    ///   Then 2 more acquires go to Pending (queue), then next gets PoolExhausted
    #[test]
    fn bh_exhaustion_by_holding_all_connections() {
        let mut pool = create_tiny_pool(2, 2);

        let mut held = Vec::new();
        for _ in 0..2 {
            let result = futures::executor::block_on(pool.acquire(attacker_id()));
            match result {
                AcquireResult::Available { connection } => {
                    held.push(connection.connection_id);
                }
                other => panic!("Should acquire, got {:?}", other),
            }
        }

        assert_eq!(pool.stats().checked_out_connections, 2);
        assert_eq!(pool.stats().total_connections, 2);

        for i in 0..2 {
            let result = futures::executor::block_on(pool.acquire(victim_id()));
            match result {
                AcquireResult::Pending { .. } => {}
                other => panic!(
                    "Pending acquire #{} should be Pending, got {:?}",
                    i + 1,
                    other
                ),
            }
        }

        assert_eq!(pool.stats().pending_acquires, 2);

        let result = futures::executor::block_on(pool.acquire(victim_id()));
        match result {
            AcquireResult::PoolExhausted { .. } => {}
            other => panic!(
                "Should be PoolExhausted after holding all + pending full, got {:?}",
                other
            ),
        }
    }

    /// INV-EXH-01: Total connections never exceeds max_connections.
    /// Given: Pool with max_connections=3
    /// When: Acquiring 100 connections sequentially (never releasing)
    /// Then: Exactly max_connections are Available, rest are PoolExhausted or Pending
    #[test]
    fn bh_pool_count_never_exceeds_max() {
        let mut pool = create_tiny_pool(3, 3);
        let mut available_count = 0u32;
        let mut pending_count = 0u32;
        let mut exhausted_count = 0u32;

        for _ in 0..100 {
            let result = futures::executor::block_on(pool.acquire(attacker_id()));
            match result {
                AcquireResult::Available { .. } => available_count += 1,
                AcquireResult::Pending { .. } => pending_count += 1,
                AcquireResult::PoolExhausted { .. } => exhausted_count += 1,
                AcquireResult::PoolClosing { .. } => exhausted_count += 1,
                AcquireResult::Timeout { .. } => exhausted_count += 1,
            }

            assert!(
                pool.stats().total_connections <= 3,
                "INV-EXH-01 violated: total_connections = {}",
                pool.stats().total_connections
            );
        }

        assert_eq!(available_count, 3, "Exactly max_connections should be Available");
        assert_eq!(pending_count, 3, "Exactly max_pending_acquires should be Pending");
        assert_eq!(exhausted_count, 94, "Remaining 94 should be PoolExhausted");
        assert_eq!(pool.stats().total_connections, 3);
    }

    /// INV-EXH-04: Pending queue overflow returns PoolExhausted immediately.
    /// Given: Pool max_connections=1, max_pending_acquires=1
    /// When: Acquire 1 (Available), acquire 1 (Pending), acquire 1 (PoolExhausted)
    #[test]
    fn bh_pending_queue_overflow_returns_pool_exhausted() {
        let mut pool = create_tiny_pool(1, 1);

        let r1 = futures::executor::block_on(pool.acquire(attacker_id()));
        assert!(matches!(r1, AcquireResult::Available { .. }));

        let r2 = futures::executor::block_on(pool.acquire(victim_id()));
        assert!(matches!(r2, AcquireResult::Pending { .. }));

        let r3 = futures::executor::block_on(pool.acquire(victim_id()));
        match r3 {
            AcquireResult::PoolExhausted { .. } => {}
            other => panic!(
                "Pending overflow must return PoolExhausted, got {:?}",
                other
            ),
        }

        assert_eq!(pool.stats().pending_acquires, 1);
    }

    /// INV-EXH-05: After exhaustion, releasing a connection makes it available again.
    /// Given: Pool exhausted (all connections held)
    /// When: One connection is released
    /// Then: Next acquire succeeds with Available
    #[test]
    fn bh_release_restores_availability_after_exhaustion() {
        let mut pool = create_tiny_pool(1, 0);

        let result = futures::executor::block_on(pool.acquire(attacker_id()));
        let conn_id = match result {
            AcquireResult::Available { connection } => connection.connection_id,
            other => panic!("Should acquire, got {:?}", other),
        };

        let r2 = futures::executor::block_on(pool.acquire(victim_id()));
        assert!(matches!(r2, AcquireResult::PoolExhausted { .. }));

        assert_eq!(pool.release(conn_id, attacker_id()), ReleaseResult::Returned);
        assert_eq!(pool.stats().idle_connections, 1);

        let r3 = futures::executor::block_on(pool.acquire(victim_id()));
        match r3 {
            AcquireResult::Available { connection } => {
                assert_eq!(connection.connection_id, conn_id);
            }
            other => panic!(
                "Should be Available after release, got {:?}",
                other
            ),
        }
    }

    /// INV-EXH-06: Pool stats accurately reflect exhaustion state.
    /// Given: A pool being exhausted by a malicious actor
    /// When: Checking stats at each stage
    /// Then: Stats show correct checked_out, idle, pending, total counts
    #[test]
    fn bh_stats_accurately_reflect_exhaustion_state() {
        let mut pool = create_tiny_pool(2, 2);

        let s = pool.stats();
        assert_eq!(s.total_connections, 0);
        assert_eq!(s.checked_out_connections, 0);
        assert_eq!(s.idle_connections, 0);
        assert_eq!(s.pending_acquires, 0);

        let r1 = futures::executor::block_on(pool.acquire(attacker_id()));
        let id1 = match r1 {
            AcquireResult::Available { connection } => connection.connection_id,
            _ => panic!("Expected Available"),
        };
        let s = pool.stats();
        assert_eq!(s.total_connections, 1);
        assert_eq!(s.checked_out_connections, 1);
        assert_eq!(s.idle_connections, 0);
        assert_eq!(s.total_acquires, 1);

        let r2 = futures::executor::block_on(pool.acquire(attacker_id()));
        let id2 = match r2 {
            AcquireResult::Available { connection } => connection.connection_id,
            _ => panic!("Expected Available"),
        };
        let s = pool.stats();
        assert_eq!(s.total_connections, 2);
        assert_eq!(s.checked_out_connections, 2);
        assert_eq!(s.idle_connections, 0);

        let _ = futures::executor::block_on(pool.acquire(victim_id()));
        let s = pool.stats();
        assert_eq!(s.pending_acquires, 1);

        let _ = futures::executor::block_on(pool.acquire(victim_id()));
        let s = pool.stats();
        assert_eq!(s.pending_acquires, 2);

        let _ = futures::executor::block_on(pool.acquire(victim_id()));
        let s = pool.stats();
        assert_eq!(s.pending_acquires, 2, "Pending should not grow past max");
        assert_eq!(s.checked_out_connections, 2);

        pool.release(id1, attacker_id());
        let s = pool.stats();
        assert_eq!(s.checked_out_connections, 1);
        assert_eq!(s.idle_connections, 1);
        assert_eq!(s.total_releases, 1);
        assert_eq!(s.total_connections, 2);

        let _ = pool.release(id2, attacker_id());
    }

    /// INV-EXH-07: Shutdown during active exhaustion evicts all held connections.
    /// Given: Malicious actor holds all connections
    /// When: shutdown() is called
    /// Then: Pool clears entirely
    #[test]
    fn bh_shutdown_during_exhaustion_evicts_all() {
        let mut pool = create_tiny_pool(3, 2);

        let mut held = Vec::new();
        for _ in 0..3 {
            let r = futures::executor::block_on(pool.acquire(attacker_id()));
            if let AcquireResult::Available { connection } = r {
                held.push(connection.connection_id);
            }
        }

        for _ in 0..2 {
            let r = futures::executor::block_on(pool.acquire(victim_id()));
            assert!(matches!(r, AcquireResult::Pending { .. }));
        }

        assert_eq!(pool.stats().total_connections, 3);
        assert_eq!(pool.stats().pending_acquires, 2);

        pool.shutdown();

        assert!(pool.is_shutting_down());
        assert_eq!(pool.stats().total_connections, 0);
        assert_eq!(pool.stats().checked_out_connections, 0);
        assert_eq!(pool.stats().idle_connections, 0);
        assert_eq!(pool.stats().pending_acquires, 0);

        let r = futures::executor::block_on(pool.acquire(victim_id()));
        assert!(matches!(r, AcquireResult::PoolClosing));
    }

    /// INV-EXH-08: No connection leak — all acquisitions past max_connections
    /// return PoolExhausted or Pending, never Available.
    /// Given: Pool max_connections=2, max_pending_acquires=1
    /// When: Acquiring 50 times without releasing
    /// Then: Exactly 2 Available, 1 Pending, 47 PoolExhausted; no connection count exceeds 2
    #[test]
    fn bh_no_connection_leak_under_sustained_attack() {
        let mut pool = create_tiny_pool(2, 1);
        let mut counts = (0u32, 0u32, 0u32); // (available, pending, exhausted)

        for _ in 0..50 {
            let result = futures::executor::block_on(pool.acquire(attacker_id()));
            match result {
                AcquireResult::Available { .. } => counts.0 += 1,
                AcquireResult::Pending { .. } => counts.1 += 1,
                AcquireResult::PoolExhausted { .. } => counts.2 += 1,
                _ => counts.2 += 1,
            }

            assert!(
                pool.stats().total_connections <= 2,
                "Connection leak detected: total_connections = {}",
                pool.stats().total_connections
            );
        }

        assert_eq!(counts.0, 2, "Only max_connections should be Available");
        assert_eq!(counts.1, 1, "Only max_pending_acquires should be Pending");
        assert_eq!(counts.2, 47, "All remaining should be PoolExhausted");
    }

    /// Circuit breaker as defense: When circuit breaker is Open due to failures,
    /// even though pool has capacity, acquires return PoolExhausted.
    #[test]
    fn bh_circuit_breaker_blocks_exhaustion_even_with_capacity() {
        let mut pool = create_tiny_pool(5, 5);

        pool.state
            .circuit_breaker
            .transition_to(CircuitBreakerState::Open);

        let result = futures::executor::block_on(pool.acquire(victim_id()));
        match result {
            AcquireResult::PoolExhausted { .. } => {}
            other => panic!(
                "Circuit breaker should block acquire even with capacity, got {:?}",
                other
            ),
        }

        assert_eq!(pool.stats().total_connections, 0);
    }

    /// Multi-actor scenario: Malicious actor holds all connections,
    /// legitimate actor denied then recovers when released.
    /// Given: Pool max_connections=2, max_pending_acquires=1
    /// When: Actor A holds 2 connections, Actor B tries to acquire
    /// Then: Actor B gets Pending (queue has room), then PoolExhausted on second try
    #[test]
    fn bh_legitimate_actor_denied_service_by_malicious_holder() {
        let mut pool = create_tiny_pool(2, 1);

        let mut actor_a_held = Vec::new();
        for _ in 0..2 {
            let r = futures::executor::block_on(pool.acquire(attacker_id()));
            if let AcquireResult::Available { connection } = r {
                actor_a_held.push(connection.connection_id);
            }
        }

        let r1 = futures::executor::block_on(pool.acquire(victim_id()));
        assert!(
            matches!(r1, AcquireResult::Pending { .. }),
            "Legitimate actor should get Pending, got {:?}",
            r1
        );

        let r2 = futures::executor::block_on(pool.acquire(victim_id()));
        assert!(
            matches!(r2, AcquireResult::PoolExhausted { .. }),
            "Legitimate actor should get PoolExhausted, got {:?}",
            r2
        );

        let released_id = actor_a_held.pop().unwrap();
        pool.release(released_id, attacker_id());

        let r3 = futures::executor::block_on(pool.acquire(victim_id()));
        match r3 {
            AcquireResult::Available { connection } => {
                assert_eq!(connection.connection_id, released_id);
            }
            other => panic!(
                "Legitimate actor should recover after release, got {:?}",
                other
            ),
        }
    }

    /// Rapid acquire/release cycle under pressure: Verify pool stays consistent
    /// when a malicious actor rapidly acquires and releases to thrash the pool.
    #[test]
    fn bh_rapid_acquire_release_thrash_stays_bounded() {
        let mut pool = create_tiny_pool(3, 2);
        let actor = attacker_id();

        for i in 0..200 {
            let mut held = Vec::new();
            for _ in 0..3 {
                let r = futures::executor::block_on(pool.acquire(actor.clone()));
                if let AcquireResult::Available { connection } = r {
                    held.push(connection.connection_id);
                }
            }

            assert!(
                pool.stats().total_connections <= 3,
                "Iteration {}: total_connections = {} exceeds max",
                i,
                pool.stats().total_connections
            );

            for id in held {
                assert_eq!(pool.release(id, actor.clone()), ReleaseResult::Returned);
            }

            assert_eq!(pool.stats().checked_out_connections, 0);
            assert_eq!(pool.stats().idle_connections, 3.min(pool.stats().total_connections));
        }
    }

    /// Exhaustion with zero pending queue: Immediate rejection when no buffering.
    /// Given: Pool max_connections=1, max_pending_acquires=0
    /// When: Acquire one connection, then attempt another
    /// Then: Second acquire immediately returns PoolExhausted
    #[test]
    fn bh_zero_pending_queue_immediate_rejection() {
        let mut pool = create_tiny_pool(1, 0);

        let r1 = futures::executor::block_on(pool.acquire(attacker_id()));
        assert!(matches!(r1, AcquireResult::Available { .. }));

        let r2 = futures::executor::block_on(pool.acquire(victim_id()));
        match r2 {
            AcquireResult::PoolExhausted { .. } => {}
            other => panic!(
                "Zero pending queue should immediately reject, got {:?}",
                other
            ),
        }

        assert_eq!(pool.stats().pending_acquires, 0);
    }

    /// Verify that PoolExhausted result carries the correct config snapshot.
    #[test]
    fn bh_pool_exhausted_carries_config_snapshot() {
        let mut pool = create_tiny_pool(1, 0);

        let _ = futures::executor::block_on(pool.acquire(attacker_id()));

        let result = futures::executor::block_on(pool.acquire(victim_id()));
        match result {
            AcquireResult::PoolExhausted { config } => {
                assert_eq!(config.max_connections, 1);
                assert_eq!(config.max_pending_acquires, 0);
                assert_eq!(config.connection_timeout_ms, 5000);
            }
            other => panic!("Expected PoolExhausted with config, got {:?}", other),
        }
    }
}
