//! Connection Pool Implementation
//!
//! This module provides the core connection pool implementation for managing
//! NATS client connections in the veloxide distributed worker system.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, OwnedSemaphorePermit, Semaphore};
use tracing::{debug, error, info, warn};

use vo_common::connection_pool::{
    AcquireResult, CircuitBreakerState, ConnectionId, ConnectionPoolError, ConnectionStatus,
    ErrorCategory, ErrorContext, ErrorDetail, EvictionReason, HealthCheckResult,
    PoolConfig as VoPoolConfig, PoolId, PoolStats, PooledConnection, ReleaseResult, WaitHandle,
};
use vo_types::TimestampMs;

use super::circuit_breaker::CircuitBreaker;
use super::config::{PoolConfig, PoolConfigError};
use super::health_check::{determine_health_check_result, HealthCheck};

/// Demand signal indicating current pool pressure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemandSignal {
    /// High demand — pool is under pressure, should grow.
    High,
    /// Normal demand — pool is balanced.
    Normal,
    /// Low demand — pool has excess capacity, should shrink.
    Low,
}

/// Result of a scale decision made by the pool scaler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScaleResult {
    /// Pool scaled up; new_target is the updated connection target.
    ScaledUp { new_target: u32 },
    /// Pool scaled down; new_target is the updated connection target.
    ScaledDown { new_target: u32 },
    /// Cooldown period is still active; no scaling performed.
    CooldownRemaining,
    /// No scaling action needed at this time.
    NoAction,
}

/// Adaptive pool scaler that manages connection count based on demand.
#[derive(Debug, Clone)]
pub struct PoolScaler {
    last_scale_at: Instant,
    scale_cooldown: Duration,
    current_target: u32,
}

impl PoolScaler {
    pub fn new(min_connections: u32, scale_cooldown: Duration) -> Self {
        Self {
            last_scale_at: Instant::now(),
            scale_cooldown,
            current_target: min_connections,
        }
    }

    pub fn current_target(&self) -> u32 {
        self.current_target
    }

    pub fn last_scale_at(&self) -> Instant {
        self.last_scale_at
    }

    pub fn scale_cooldown(&self) -> Duration {
        self.scale_cooldown
    }
}

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

#[derive(Debug)]
pub struct PoolState {
    pub pool_id: PoolId,
    pub connections: HashMap<ConnectionId, PooledConnection>,
    pub idle_connections: VecDeque<ConnectionId>,
    pub checked_out_connections: HashMap<ConnectionId, ConnectionId>,
    pub pending_acquires: VecDeque<WaitHandle>,
    pub config: PoolConfig,
    pub circuit_breaker: CircuitBreaker,
    pub health_check: HealthCheck,
    pub scaler: PoolScaler,
    pub total_acquires: u64,
    pub total_releases: u64,
    pub total_evictions: u64,
    pub total_health_checks: u64,
    pub failed_health_checks: u64,
    pub is_shutting_down: bool,
}

impl PoolState {
    pub fn new(pool_id: PoolId, config: PoolConfig) -> Self {
        let scaler = PoolScaler::new(config.min_connections, Duration::from_secs(5));
        Self {
            pool_id,
            connections: HashMap::new(),
            idle_connections: VecDeque::new(),
            checked_out_connections: HashMap::new(),
            pending_acquires: VecDeque::new(),
            circuit_breaker: CircuitBreaker::new(),
            health_check: HealthCheck::new(config.health_check_interval_ms),
            config,
            scaler,
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

    pub fn scaler(&self) -> &PoolScaler {
        &self.scaler
    }

    pub fn scaler_mut(&mut self) -> &mut PoolScaler {
        &mut self.scaler
    }
}

#[derive(Debug)]
pub struct ConnectionPool {
    pool_id: PoolId,
    pub(crate) state: PoolState,
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

    pub async fn acquire(&mut self) -> AcquireResult {
        self.acquire_with_timeout(Duration::from_millis(
            self.state.config.connection_timeout_ms,
        ))
        .await
    }

    #[allow(clippy::unused_async)]
    pub async fn acquire_with_timeout(&mut self, timeout: Duration) -> AcquireResult {
        self.try_scale(DemandSignal::High);

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
                    timestamp: TimestampMs::now().into(),
                    operation: "acquire",
                    connection_id: None,
                },
            };
            error!("Circuit breaker open: {:?}", error);
            return AcquireResult::PoolExhausted {
                config: self.state.config.clone().into(),
            };
        }

        if let Some(conn_id) = self.state.idle_connections.pop_front() {
            if let Some(mut conn) = self.state.connections.get_mut(&conn_id) {
                conn.status = ConnectionStatus::CheckedOut;
                conn.increment_use_count();
                self.state.total_acquires += 1;

                let checkout_id = ConnectionId::new();
                self.state
                    .checked_out_connections
                    .insert(checkout_id, conn_id);

                debug!("Acquired connection {} from pool {}", conn_id, self.pool_id);

                return AcquireResult::Available {
                    connection: conn.clone(),
                };
            }
        }

        if self.state.total_connections() < self.state.config.max_connections {
            let connection_id = ConnectionId::new();
            let now = TimestampMs::now();

            let pooled =
                PooledConnection::new(connection_id, now.as_u64().into()).with_status(ConnectionStatus::CheckedOut);

            self.state.connections.insert(connection_id, pooled.clone());

            let checkout_id = ConnectionId::new();
            self.state
                .checked_out_connections
                .insert(checkout_id, connection_id);

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
                    timestamp: TimestampMs::now().into(),
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
            enqueued_at: TimestampMs::now().into(),
            pool_id: self.pool_id.clone(),
        };
        self.state.pending_acquires.push_back(wait_handle.clone());

        AcquireResult::Pending { wait_handle }
    }

    pub fn release(&mut self, connection_id: ConnectionId) -> ReleaseResult {
        if self.state.is_shutting_down {
            return self.evict_connection(connection_id, EvictionReason::ExplicitEviction);
        }

        let checkout_id = self
            .state
            .checked_out_connections
            .iter()
            .find(|&(_, cid)| *cid == connection_id)
            .map(|(id, _)| *id);

        let Some(checkout_id) = checkout_id else {
            return ReleaseResult::AlreadyClosed;
        };

        self.state.checked_out_connections.remove(&checkout_id);

        if let Some(conn) = self.state.connections.get_mut(&connection_id) {
            if let Some(waiter) = self.state.pending_acquires.pop_front() {
                conn.status = ConnectionStatus::CheckedOut;
                conn.increment_use_count();

                let waiter_checkout_id = ConnectionId::new();
                self.state
                    .checked_out_connections
                    .insert(waiter_checkout_id, connection_id);

                self.connection_semaphore.add_permits(1);

                debug!(
                    "Released connection {} to waiting request {} in pool {}",
                    connection_id, waiter.request_id, self.pool_id
                );
                return ReleaseResult::Returned;
            }

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
            .retain(|_, cid| *cid != connection_id);

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
                TimestampMs::new_unchecked(conn.last_used_at),
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

    /// Attempt to create a new connection in the pool.
    /// Returns true if a new connection was created, false if pool is at max capacity.
    pub fn create_connection(&mut self) -> bool {
        if self.state.total_connections() >= self.state.config.max_connections {
            return false;
        }

        if self.state.is_shutting_down {
            return false;
        }

        let connection_id = ConnectionId::new();
        let now = TimestampMs::now();
        let pooled = PooledConnection::new(connection_id, now.as_u64().into()).with_status(ConnectionStatus::Idle);

        self.state.connections.insert(connection_id, pooled);
        self.state.idle_connections.push_back(connection_id);

        debug!(
            "Scaler created new idle connection {} in pool {}",
            connection_id, self.pool_id
        );
        true
    }

    /// Adaptive scaling decision based on demand signal.
    ///
    /// Grows the pool toward max_connections during high demand and shrinks
    /// toward min_connections during idle periods, respecting a configurable
    /// cooldown between scaling decisions.
    pub fn try_scale(&mut self, demand_signal: DemandSignal) -> ScaleResult {
        let scaler = &self.state.scaler;
        let now = Instant::now();

        let elapsed = now.duration_since(scaler.last_scale_at());
        if elapsed < scaler.scale_cooldown() {
            return ScaleResult::CooldownRemaining;
        }

        let idle_count = self.state.idle_count();
        let pending_count = self.state.pending_acquires.len() as u32;

        match demand_signal {
            DemandSignal::High => {
                let current_target = scaler.current_target();
                if current_target >= self.state.config.max_connections {
                    return ScaleResult::NoAction;
                }

                let demand_threshold = (current_target as f64 * 0.8) as u32;
                let demand = idle_count + pending_count;

                if demand > demand_threshold {
                    let new_target = current_target + 1;
                    self.state.scaler_mut().current_target = new_target;
                    self.state.scaler_mut().last_scale_at = now;

                    let created = self.create_connection();
                    if created {
                        debug!(
                            "Pool {} scaled up to target {} (demand: {}, idle: {}, pending: {})",
                            self.pool_id, new_target, demand, idle_count, pending_count
                        );
                    }
                    ScaleResult::ScaledUp { new_target }
                } else {
                    ScaleResult::NoAction
                }
            }
            DemandSignal::Low => {
                let current_target = scaler.current_target();
                if current_target <= self.state.config.min_connections {
                    return ScaleResult::NoAction;
                }

                let idle_threshold = (current_target as f64 * 0.5) as u32;

                if idle_count > idle_threshold {
                    let new_target = current_target - 1;
                    self.state.scaler_mut().current_target = new_target;
                    self.state.scaler_mut().last_scale_at = now;

                    debug!(
                        "Pool {} scaled down to target {} (idle: {}, threshold: {})",
                        self.pool_id, new_target, idle_count, idle_threshold
                    );
                    ScaleResult::ScaledDown { new_target }
                } else {
                    ScaleResult::NoAction
                }
            }
            DemandSignal::Normal => ScaleResult::NoAction,
        }
    }

    /// Spawn a background task that periodically checks pool demand and
    /// triggers scale-down during idle periods every 30 seconds.
    pub fn start_scaling_task(&self) -> tokio::task::JoinHandle<()> {
        let pool_id = self.pool_id.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;

                debug!(
                    "Scaling tick for pool {} — pool not yet fully wired for runtime scaling",
                    pool_id
                );
            }
        })
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
        let result = futures::executor::block_on(pool.acquire());
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
        let acquire_result = futures::executor::block_on(pool.acquire());
        let conn_id = match acquire_result {
            AcquireResult::Available { connection } => connection.connection_id,
            _ => panic!("Expected Available result"),
        };

        let release_result = pool.release(conn_id);
        assert_eq!(release_result, ReleaseResult::Returned);
        assert_eq!(pool.stats().idle_connections, 1);
    }

    #[test]
    fn test_release_unknown_connection() {
        let mut pool = create_test_pool();
        let unknown_id = ConnectionId::new();
        let result = pool.release(unknown_id);
        assert_eq!(result, ReleaseResult::AlreadyClosed);
    }

    #[test]
    fn test_pool_stats() {
        let mut pool = create_test_pool();
        let acquire_result = futures::executor::block_on(pool.acquire());
        let conn_id = match acquire_result {
            AcquireResult::Available { connection } => connection.connection_id,
            _ => panic!("Expected Available result"),
        };

        let stats = pool.stats();
        assert_eq!(stats.total_connections, 1);
        assert_eq!(stats.checked_out_connections, 1);
        assert_eq!(stats.idle_connections, 0);

        pool.release(conn_id);

        let stats = pool.stats();
        assert_eq!(stats.total_connections, 1);
        assert_eq!(stats.checked_out_connections, 0);
        assert_eq!(stats.idle_connections, 1);
    }

    #[test]
    fn test_shutdown_clears_pool() {
        let mut pool = create_test_pool();
        futures::executor::block_on(pool.acquire());
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

        let result = pool.acquire().await;
        match result {
            AcquireResult::PoolExhausted { .. } => {}
            _ => panic!("Expected PoolExhausted when circuit breaker is open"),
        }
    }

    #[test]
    fn test_release_twice_returns_already_closed() {
        let mut pool = create_test_pool();
        let acquire_result = futures::executor::block_on(pool.acquire());
        let conn_id = match acquire_result {
            AcquireResult::Available { connection } => connection.connection_id,
            _ => panic!("Expected Available result"),
        };

        let first_release = pool.release(conn_id);
        assert_eq!(first_release, ReleaseResult::Returned);

        let second_release = pool.release(conn_id);
        assert_eq!(second_release, ReleaseResult::AlreadyClosed);
    }

    #[test]
    fn test_release_without_acquire_returns_already_closed() {
        let mut pool = create_test_pool();
        let never_acquired = ConnectionId::new();
        let result = pool.release(never_acquired);
        assert_eq!(result, ReleaseResult::AlreadyClosed);
    }

    #[tokio::test]
    async fn test_release_fulfills_pending_acquire() {
        let pool_id = PoolId::new("pending-test-pool");
        let nats_urls = vec!["nats://localhost:4222".to_string()];
        let config = PoolConfig::new(0, 1, 5000, 30000, 10000, 5).unwrap();
        let mut pool = ConnectionPool::new(pool_id, nats_urls, config);

        let result1 = pool.acquire().await;
        let conn_id = match result1 {
            AcquireResult::Available { connection } => connection.connection_id,
            _ => panic!("Expected Available result"),
        };
        assert_eq!(pool.stats().checked_out_connections, 1);
        assert_eq!(pool.stats().idle_connections, 0);
        assert_eq!(pool.stats().pending_acquires, 0);

        let pending_result = pool.acquire().await;
        let pending_handle = match pending_result {
            AcquireResult::Pending { wait_handle } => wait_handle,
            _ => panic!("Expected Pending result"),
        };
        assert_eq!(pool.stats().pending_acquires, 1);

        pool.release(conn_id);

        assert_eq!(pool.stats().pending_acquires, 0);
        assert_eq!(pool.stats().checked_out_connections, 1);
        assert_eq!(pool.stats().idle_connections, 0);

        let _ = pending_handle;
    }

    // ========================================================================
    // Pool Scaler Tests (vel-9kxy)
    // ========================================================================

    fn create_scaler_pool(min: u32, max: u32, cooldown_secs: u64) -> ConnectionPool {
        let pool_id = PoolId::new("scaler-test-pool");
        let nats_urls = vec!["nats://localhost:4222".to_string()];
        let config = PoolConfig::new(min, max, 5000, 30000, 10000, 5).unwrap();
        let mut pool = ConnectionPool::new(pool_id, nats_urls, config);

        pool.state.scaler = PoolScaler::new(min, Duration::from_secs(cooldown_secs));

        pool
    }

    /// Acquire N connections and release them, filling the pool with N idle connections.
    fn fill_pool_with_idle(pool: &mut ConnectionPool, count: u32) {
        let mut conn_ids = Vec::new();
        for _ in 0..count {
            let result = futures::executor::block_on(pool.acquire());
            let conn_id = match result {
                AcquireResult::Available { connection } => connection.connection_id,
                _ => panic!("Expected Available result"),
            };
            conn_ids.push(conn_id);
        }
        // Release all connections back to the pool
        for conn_id in conn_ids {
            pool.release(conn_id);
        }
    }

    #[test]
    fn test_scale_up_on_high_demand() {
        let mut pool = create_scaler_pool(2, 10, 1);
        assert_eq!(pool.state.scaler.current_target(), 2);

        fill_pool_with_idle(&mut pool, 4);

        assert_eq!(pool.state.idle_connections.len() as u32, 4);
        assert_eq!(pool.state.checked_out_connections.len() as u32, 0);

        pool.state.scaler.last_scale_at = Instant::now() - Duration::from_secs(10);

        let result = pool.try_scale(DemandSignal::High);
        match result {
            ScaleResult::ScaledUp { new_target } => {
                assert_eq!(new_target, 3);
                assert_eq!(pool.state.scaler.current_target(), 3);
            }
            other => panic!("Expected ScaledUp, got {:?}", other),
        }
    }

    #[test]
    fn test_scale_down_on_idle() {
        let mut pool = create_scaler_pool(2, 10, 1);
        pool.state.scaler.current_target = 5;

        fill_pool_with_idle(&mut pool, 5);

        let idle_count = pool.state.idle_connections.len() as u32;
        assert_eq!(idle_count, 5);

        pool.state.scaler.last_scale_at = Instant::now() - Duration::from_secs(10);

        let result = pool.try_scale(DemandSignal::Low);
        match result {
            ScaleResult::ScaledDown { new_target } => {
                assert_eq!(new_target, 4);
                assert_eq!(pool.state.scaler.current_target(), 4);
            }
            other => panic!("Expected ScaledDown, got {:?}", other),
        }
    }

    #[test]
    fn test_scale_cooldown_enforced() {
        let mut pool = create_scaler_pool(4, 10, 5);

        fill_pool_with_idle(&mut pool, 4);

        assert_eq!(pool.state.idle_connections.len() as u32, 4);

        pool.state.scaler.last_scale_at = Instant::now() - Duration::from_secs(10);

        let first_result = pool.try_scale(DemandSignal::High);
        match first_result {
            ScaleResult::ScaledUp { .. } => {}
            other => panic!("Expected ScaledUp on first call, got {:?}", other),
        }

        let second_result = pool.try_scale(DemandSignal::High);
        assert_eq!(second_result, ScaleResult::CooldownRemaining);
    }

    #[test]
    fn test_scale_respects_max_connections() {
        let mut pool = create_scaler_pool(2, 5, 1);
        pool.state.scaler.current_target = 5;

        fill_pool_with_idle(&mut pool, 5);

        pool.state.scaler.last_scale_at = Instant::now() - Duration::from_secs(10);

        let result = pool.try_scale(DemandSignal::High);
        assert_eq!(result, ScaleResult::NoAction);
    }

    #[test]
    fn test_scale_respects_min_connections() {
        let mut pool = create_scaler_pool(3, 10, 1);
        pool.state.scaler.current_target = 3;

        fill_pool_with_idle(&mut pool, 5);

        pool.state.scaler.last_scale_at = Instant::now() - Duration::from_secs(10);

        let result = pool.try_scale(DemandSignal::Low);
        assert_eq!(result, ScaleResult::NoAction);
    }

    #[test]
    fn test_scale_normal_signal_no_action() {
        let mut pool = create_scaler_pool(2, 10, 1);

        fill_pool_with_idle(&mut pool, 5);

        pool.state.scaler.last_scale_at = Instant::now() - Duration::from_secs(10);

        let result = pool.try_scale(DemandSignal::Normal);
        assert_eq!(result, ScaleResult::NoAction);
    }
}
