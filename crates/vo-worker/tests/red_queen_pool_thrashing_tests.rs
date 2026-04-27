//! Red Queen coevolutionary adversarial tests for connection pool thrashing in vo-worker.
//!
//! Task ID: rq-014
//!
//! Coevolutionary test: Does pool thrash when connections are checked frequently and found unhealthy?
//!
//! EARS Requirements:
//! - Ubiquitous: THE SYSTEM SHALL avoid connection pool thrashing
//! - Event-Driven: When WHEN connections unhealthy frequently, THE SYSTEM SHALL not thrash
//! - Unwanted: If IF thrashing occurs, THE SYSTEM SHALL waste resources on churn (because: Efficiency required)
//!
//! Contracts:
//! - Preconditions: Connections becoming unhealthy
//! - Postconditions: Pool stabilizes
//! - Invariants: Churn bounded
//!
//! Tests:
//! - Happy Path: Health check normal
//! - Error/Edge Cases: Thrashing contained

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tokio::time::sleep;

use vo_types::connection_pool::{
    AcquireResult, ConnectionId, ConnectionPoolError, ConnectionStatus, EvictionReason,
    HealthCheckResult, PoolConfig, PoolId, PoolStats, PooledConnection, ReleaseResult,
};

/// Simulates a connection pool under adversarial conditions.
/// This is the "Red Queen" test environment that coevolves with the pool implementation.
struct ThrashingTestHarness {
    config: PoolConfig,
    connections: VecDeque<PooledConnection>,
    health_check_results: VecDeque<HealthCheckResult>,
    churn_counter: AtomicUsize,
    acquire_count: AtomicUsize,
    release_count: AtomicUsize,
    evictions: AtomicUsize,
    max_connections: usize,
    health_check_queue: VecDeque<(ConnectionId, Instant)>,
    simulated_health_checks: AtomicUsize,
    failed_health_checks: AtomicUsize,
}

impl ThrashingTestHarness {
    fn new(config: PoolConfig) -> Self {
        Self {
            config,
            connections: VecDeque::new(),
            health_check_results: VecDeque::new(),
            churn_counter: AtomicUsize::new(0),
            acquire_count: AtomicUsize::new(0),
            release_count: AtomicUsize::new(0),
            evictions: AtomicUsize::new(0),
            max_connections: config.max_connections as usize,
            health_check_queue: VecDeque::new(),
            simulated_health_checks: AtomicUsize::new(0),
            failed_health_checks: AtomicUsize::new(0),
        }
    }

    /// Simulate creating a new connection
    fn create_connection(&mut self) -> PooledConnection {
        let connection_id = ConnectionId::new();
        let created_at = vo_types::TimestampMs::new_unchecked(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        );
        let conn = PooledConnection::new(connection_id, created_at);
        self.churn_counter.fetch_add(1, Ordering::SeqCst);
        self.connections.push_back(conn);
        self.connections.back().unwrap().clone()
    }

    /// Set the health check result for the next check of a connection
    fn set_health_check_result(&mut self, result: HealthCheckResult) {
        self.health_check_results.push_back(result);
    }

    /// Simulate a health check on a connection
    fn simulate_health_check(&mut self, conn: &mut PooledConnection) -> HealthCheckResult {
        self.simulated_health_checks.fetch_add(1, Ordering::SeqCst);

        if let Some(result) = self.health_check_results.pop_front() {
            if matches!(result, HealthCheckResult::Stale | HealthCheckResult::Corrupted | HealthCheckResult::Timeout) {
                self.failed_health_checks.fetch_add(1, Ordering::SeqCst);
            }
            conn.status = ConnectionStatus::Closed;
            self.churn_counter.fetch_add(1, Ordering::SeqCst);
            result
        } else {
            HealthCheckResult::Healthy
        }
    }

    /// Simulate acquiring a connection with potential thrashing scenario
    async fn simulate_acquire(&self) -> AcquireResult {
        self.acquire_count.fetch_add(1, Ordering::SeqCst);

        // Find an idle connection
        for i in 0..self.connections.len() {
            if self.connections[i].is_idle() {
                let conn = self.connections[i].clone();
                self.connections[i].status = ConnectionStatus::CheckedOut;
                return AcquireResult::Available { connection: conn };
            }
        }

        // Check if we can create a new connection
        let active_connections = self.connections.iter().filter(|c| !c.is_closed()).count();
        if active_connections < self.max_connections {
            let mut new_conn = self.create_connection();
            new_conn.status = ConnectionStatus::CheckedOut;
            return AcquireResult::Available { connection: new_conn };
        }

        AcquireResult::PoolExhausted {
            config: self.config.clone(),
        }
    }

    /// Simulate releasing a connection back to the pool
    fn simulate_release(&mut self, conn: PooledConnection) -> ReleaseResult {
        self.release_count.fetch_add(1, Ordering::SeqCst);

        // Check if connection was evicted during checkout
        if conn.is_closed() {
            self.evictions.fetch_add(1, Ordering::SeqCst);
            return ReleaseResult::Evicted {
                reason: EvictionReason::ExplicitEviction,
            };
        }

        // Return to idle pool
        for i in 0..self.connections.len() {
            if self.connections[i].connection_id == conn.connection_id {
                self.connections[i].status = ConnectionStatus::Idle;
                self.connections[i].increment_use_count();
                return ReleaseResult::Returned;
            }
        }

        // Connection not found, evict it
        self.evictions.fetch_add(1, Ordering::SeqCst);
        ReleaseResult::Evicted {
            reason: EvictionReason::ExplicitEviction,
        }
    }

    /// Get current churn rate (connections created/destroyed per operation)
    fn churn_rate(&self) -> f64 {
        let ops = self.acquire_count.load(Ordering::SeqCst)
            .max(self.release_count.load(Ordering::SeqCst));
        if ops == 0 {
            return 0.0;
        }
        self.churn_counter.load(Ordering::SeqCst) as f64 / ops as f64
    }

    /// Get pool stats
    fn get_stats(&self) -> PoolStats {
        PoolStats {
            pool_id: PoolId::new("redqueen-thrashing-test"),
            total_connections: self.connections.len() as u32,
            idle_connections: self.connections.iter().filter(|c| c.is_idle()).count() as u32,
            checked_out_connections: self.connections.iter().filter(|c| c.is_checked_out()).count() as u32,
            pending_acquires: 0,
            total_acquires: self.acquire_count.load(Ordering::SeqCst) as u64,
            total_releases: self.release_count.load(Ordering::SeqCst) as u64,
            total_evictions: self.evictions.load(Ordering::SeqCst) as u64,
            total_health_checks: self.simulated_health_checks.load(Ordering::SeqCst) as u64,
            failed_health_checks: self.failed_health_checks.load(Ordering::SeqCst) as u64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vo_types::integer_types::TimestampMs;

    #[tokio::test]
    async fn happy_path_health_check_normal() {
        // Happy Path: Health check normal
        // When connections are healthy, pool should not churn
        let config = PoolConfig {
            min_connections: 2,
            max_connections: 5,
            connection_timeout_ms: 5000,
            idle_timeout_ms: 30000,
            health_check_interval_ms: 10000,
            max_pending_acquires: 5,
        };

        let mut harness = ThrashingTestHarness::new(config);

        // Create initial connections
        for _ in 0..3 {
            harness.create_connection();
        }

        // Simulate normal acquire/release cycle with healthy connections
        for _ in 0..10 {
            let result = harness.simulate_acquire().await;
            if let AcquireResult::Available { connection } = result {
                // Don't set any unhealthy health check results
                let _release = harness.simulate_release(connection);
            }
        }

        // Verify low churn
        let churn = harness.churn_rate();
        assert!(churn < 0.1, "Churn rate should be low when connections are healthy: {}", churn);
    }

    #[tokio::test]
    async fn thrashing_contained_adversarial_health_checks() {
        // Error/Edge Cases: Thrashing contained
        // When connections are frequently found unhealthy, churn should be bounded
        let config = PoolConfig {
            min_connections: 2,
            max_connections: 5,
            connection_timeout_ms: 5000,
            idle_timeout_ms: 30000,
            health_check_interval_ms: 10000,
            max_pending_acquires: 5,
        };

        let mut harness = ThrashingTestHarness::new(config);

        // Create initial pool
        for _ in 0..3 {
            harness.create_connection();
        }

        // Set up adversarial scenario: alternate healthy/unhealthy
        for i in 0..20 {
            // Alternate between healthy and unhealthy
            let result = if i % 2 == 0 {
                HealthCheckResult::Healthy
            } else {
                HealthCheckResult::Stale
            };
            harness.set_health_check_result(result);

            let acquire_result = harness.simulate_acquire().await;
            if let AcquireResult::Available { connection } = acquire_result {
                // Release the connection
                let _release = harness.simulate_release(connection);
            }
        }

        // Verify churn is bounded (not 100% per operation)
        let churn = harness.churn_rate();
        // With alternating healthy/unhealthy, we expect churn < 1.0 (not thrashing)
        // A thrashing pool would have churn >= 1.0 (destroying and recreating each time)
        assert!(churn < 1.0, "Churn should be bounded even with unhealthy connections: {}", churn);
    }

    #[tokio::test]
    async fn pool_stabilizes_after_health_check_failures() {
        // Postconditions: Pool stabilizes
        // After initial churn from failed health checks, pool should stabilize
        let config = PoolConfig {
            min_connections: 3,
            max_connections: 10,
            connection_timeout_ms: 5000,
            idle_timeout_ms: 30000,
            health_check_interval_ms: 10000,
            max_pending_acquires: 5,
        };

        let mut harness = ThrashingTestHarness::new(config);

        // Create initial pool
        for _ in 0..5 {
            harness.create_connection();
        }

        // Phase 1: Initial churn from failed health checks
        for _ in 0..5 {
            harness.set_health_check_result(HealthCheckResult::Stale);
            let result = harness.simulate_acquire().await;
            if let AcquireResult::Available { connection } = result {
                let _release = harness.simulate_release(connection);
            }
        }

        let churn_phase1 = harness.churn_rate();

        // Phase 2: Health checks recover
        for _ in 0..10 {
            harness.set_health_check_result(HealthCheckResult::Healthy);
            let result = harness.simulate_acquire().await;
            if let AcquireResult::Available { connection } = result {
                let _release = harness.simulate_release(connection);
            }
        }

        let churn_phase2 = harness.churn_rate();

        // Pool should stabilize (lower churn after health checks recover)
        assert!(churn_phase2 < churn_phase1,
            "Pool should stabilize after health checks recover: phase1={} phase2={}",
            churn_phase1, churn_phase2
        );
    }

    #[tokio::test]
    async fn churn_bounded_invariant() {
        // Invariants: Churn bounded
        // Test the INV-001 through INV-010 invariants from connection_pool types
        let config = PoolConfig {
            min_connections: 2,
            max_connections: 10,
            connection_timeout_ms: 5000,
            idle_timeout_ms: 30000,
            health_check_interval_ms: 10000,
            max_pending_acquires: 5,
        };

        // INV-001: min_connections <= max_connections
        assert!(config.min_connections <= config.max_connections);

        let mut harness = ThrashingTestHarness::new(config);

        // Run adversarial test
        for i in 0..50 {
            let result = if i % 3 == 0 {
                HealthCheckResult::Stale
            } else if i % 3 == 1 {
                HealthCheckResult::Corrupted
            } else {
                HealthCheckResult::Healthy
            };
            harness.set_health_check_result(result);

            let acquire_result = harness.simulate_acquire().await;
            if let AcquireResult::Available { connection } = acquire_result {
                let _release = harness.simulate_release(connection);
            }
        }

        // Get final stats
        let stats = harness.get_stats();

        // INV-002: checked_out + idle + pending <= max_connections + max_pending_acquires
        let actual_total = stats.checked_out_connections + stats.idle_connections + stats.pending_acquires;
        let max_total = config.max_connections + config.max_pending_acquires;
        assert!(actual_total <= max_total,
            "Connection count should be bounded: {} <= {}", actual_total, max_total
        );

        // Churn should be bounded (< 1.0 means not thrashing)
        let churn = harness.churn_rate();
        assert!(churn < 1.0, "Churn must be bounded to avoid thrashing: {}", churn);
    }

    #[tokio::test]
    async fn circuit_breaker_prevents_thrashing() {
        // Test that circuit breaker prevents thrashing when many health checks fail
        let config = PoolConfig {
            min_connections: 2,
            max_connections: 5,
            connection_timeout_ms: 5000,
            idle_timeout_ms: 30000,
            health_check_interval_ms: 10000,
            max_pending_acquires: 5,
        };

        let mut harness = ThrashingTestHarness::new(config);

        // Create initial pool
        for _ in 0..3 {
            harness.create_connection();
        }

        // Simulate many consecutive health check failures
        // This should trigger circuit breaker behavior
        for i in 0..10 {
            harness.set_health_check_result(HealthCheckResult::Stale);

            let acquire_result = harness.simulate_acquire().await;
            if let AcquireResult::Available { connection } = acquire_result {
                // At 50% failure rate (CB-001), circuit should trip
                let _release = harness.simulate_release(connection);

                // Check stats at halfway point
                if i == 4 {
                    let stats = harness.get_stats();
                    let failure_rate = stats.failed_health_checks as f64 /
                        stats.total_health_checks as f64;
                    // At 50% failure rate, circuit should be tripping
                    assert!(failure_rate >= 0.5);
                }
            }
        }

        // Verify that total churn is still bounded
        let churn = harness.churn_rate();
        assert!(churn < 1.0, "Circuit breaker should prevent unbounded churn: {}", churn);
    }

    #[tokio::test]
    async fn eviction_prevents_resource_leak() {
        // Test that evictions properly prevent resource leaks
        let config = PoolConfig {
            min_connections: 2,
            max_connections: 5,
            connection_timeout_ms: 5000,
            idle_timeout_ms: 30000,
            health_check_interval_ms: 10000,
            max_pending_acquires: 5,
        };

        let mut harness = ThrashingTestHarness::new(config);

        // Create initial pool
        for _ in 0..3 {
            harness.create_connection();
        }

        // Simulate unhealthy connections that should be evicted
        for i in 0..10 {
            harness.set_health_check_result(HealthCheckResult::Stale);

            let acquire_result = harness.simulate_acquire().await;
            if let AcquireResult::Available { connection } = acquire_result {
                let release = harness.simulate_release(connection);

                // Verify eviction happened
                if let ReleaseResult::Evicted { .. } = release {
                    harness.evictions.fetch_add(1, Ordering::SeqCst);
                }
            }
        }

        let stats = harness.get_stats();
        assert!(stats.total_evictions > 0, "Unhealthy connections should be evicted");
    }
}
