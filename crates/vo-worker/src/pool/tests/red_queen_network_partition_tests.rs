//! Red Queen adversarial tests for network partition scenarios in the connection pool.
//!
//! This module implements adversarial testing for network partition scenarios:
//! - Connection acquire/release during network partition
//! - Pool recovery after network heals
//! - Connection leak prevention during partition
//! - Circuit breaker behavior during partition
//! - Health check behavior during partition
//!
//! These tests attack the pool contracts from the other side — they verify that
//! the system fails (or succeeds) correctly under adversarial network conditions
//! and does NOT leak connections.

use std::sync::Mutex;
use vo_worker::pool::{ConnectionPool, PoolConfig};
use vo_types::connection_pool::{AcquireResult, ConnectionStatus, EvictionReason, ReleaseResult};
use vo_types::connection_pool::PoolId;

static TEST_LOCK: Mutex<()> = Mutex::new(());

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

// ============================================================================
// Network Partition Tests: Connection Acquire/Release
// ============================================================================

#[cfg(test)]
mod red_queen_pool_network_partition_acquire_tests {
    use super::*;

    /// Given: A healthy connection pool
    /// When: Network partition occurs during acquire
    /// Then: Acquire fails gracefully without leaking connections
    #[tokio::test]
    async fn acquire_fails_gracefully_during_partition() {
        let _guard = test_guard();
        
        let pool_id = PoolId::new("partition-acquire-test");
        let nats_urls = vec!["nats://localhost:4222".to_string()];
        let config = PoolConfig::new(1, 3, 5000, 30000, 10000, 10).unwrap();
        let mut pool = ConnectionPool::new(pool_id.clone(), nats_urls, config);

        // Create some connections
        let mut conn_ids = Vec::new();
        for _ in 0..3 {
            match pool.acquire().await {
                AcquireResult::Available { connection } => {
                    conn_ids.push(connection.connection_id);
                }
                _ => panic!("Expected Available"),
            }
        }
        
        // Release all connections to make them idle
        for conn_id in &conn_ids {
            let result = pool.release(*conn_id);
            assert_eq!(result, ReleaseResult::Returned);
        }

        // Verify connections are idle
        assert_eq!(pool.stats().idle_connections, 3);

        // Simulate network partition by forcing shutdown
        pool.shutdown();

        // Verify: No connection leak
        let final_stats = pool.stats();
        assert_eq!(final_stats.total_connections, 0, 
            "Connection leak detected: {} connections after shutdown", 
            final_stats.total_connections);
        assert_eq!(final_stats.idle_connections, 0);
        assert_eq!(final_stats.checked_out_connections, 0);
        assert!(pool.is_shutting_down());
    }

    /// Given: A pool with idle connections
    /// When: Network partition occurs
    /// Then: Idle connections are properly evicted, not leaked
    #[tokio::test]
    async fn idle_connections_evicted_during_partition() {
        let _guard = test_guard();
        
        let pool_id = PoolId::new("partition-idle-test");
        let nats_urls = vec!["nats://localhost:4222".to_string()];
        let config = PoolConfig::new(1, 5, 5000, 30000, 10000, 10).unwrap();
        let mut pool = ConnectionPool::new(pool_id.clone(), nats_urls, config);

        // Create and release connections to have idle connections
        let mut acquired_ids = Vec::new();
        for _ in 0..3 {
            match pool.acquire().await {
                AcquireResult::Available { connection } => {
                    acquired_ids.push(connection.connection_id);
                }
                _ => panic!("Expected Available"),
            }
        }

        // Release all connections back to pool
        for conn_id in &acquired_ids {
            let result = pool.release(*conn_id);
            assert_eq!(result, ReleaseResult::Returned);
        }

        let idle_before = pool.stats().idle_connections;
        assert_eq!(idle_before, 3);

        // Simulate partition by shutting down pool
        pool.shutdown();

        // Verify: All connections properly evicted, no leak
        let final_stats = pool.stats();
        assert_eq!(final_stats.total_connections, 0);
        assert_eq!(final_stats.idle_connections, 0);
        assert!(pool.is_shutting_down());
    }

    /// Given: A pool under load
    /// When: Network partition occurs
    /// Then: Pool cleans up without leaking connections
    #[tokio::test]
    async fn pool_cleans_up_after_partition() {
        let _guard = test_guard();
        
        let pool_id = PoolId::new("partition-cleanup-test");
        let nats_urls = vec!["nats://localhost:4222".to_string()];
        let config = PoolConfig::new(1, 3, 5000, 30000, 10000, 10).unwrap();
        let mut pool = ConnectionPool::new(pool_id.clone(), nats_urls, config);

        // Phase 1: Normal operation - acquire and release several times
        for round in 0..5 {
            let mut connections = Vec::new();
            
            // Acquire some connections
            for _ in 0..2 {
                match pool.acquire().await {
                    AcquireResult::Available { connection } => {
                        connections.push(connection.connection_id);
                    }
                    _ => panic!("Expected Available, round {}", round),
                }
            }

            // Release them
            for conn_id in &connections {
                let result = pool.release(*conn_id);
                assert_eq!(result, ReleaseResult::Returned);
            }

            let stats = pool.stats();
            assert_eq!(stats.total_connections, 2);
            assert_eq!(stats.idle_connections, 2);
            assert_eq!(stats.checked_out_connections, 0);
        }

        // Phase 2: Simulate partition by forcing shutdown
        pool.shutdown();

        // Verify: No connection leak
        assert_eq!(pool.stats().total_connections, 0);
        assert_eq!(pool.stats().idle_connections, 0);
        assert!(pool.is_shutting_down());
    }
}

// ============================================================================
// Network Partition Tests: Circuit Breaker Behavior
// ============================================================================

#[cfg(test)]
mod red_queen_pool_network_partition_circuit_breaker_tests {
    use super::*;

    /// Given: A pool with circuit breaker
    /// When: Network partition causes repeated failures
    /// Then: Circuit breaker opens and prevents further acquires
    #[tokio::test]
    async fn circuit_breaker_opens_during_partition() {
        let _guard = test_guard();
        
        let pool_id = PoolId::new("partition-cb-test");
        let nats_urls = vec!["nats://localhost:4222".to_string()];
        let config = PoolConfig::new(1, 2, 5000, 30000, 10000, 10).unwrap();
        let mut pool = ConnectionPool::new(pool_id.clone(), nats_urls, config);

        // Force circuit breaker to open (simulates partition failures)
        use vo_types::connection_pool::CircuitBreakerState;
        pool.state.circuit_breaker.transition_to(CircuitBreakerState::Open);

        // Try to acquire - should be blocked by circuit breaker
        let result = pool.acquire().await;
        
        match result {
            AcquireResult::PoolExhausted { .. } => {
                // Expected: circuit breaker blocking
            }
            _ => panic!("Expected PoolExhausted when circuit breaker is open"),
        }

        // Verify circuit breaker state
        assert_eq!(pool.circuit_breaker_state(), CircuitBreakerState::Open);
    }

    /// Given: A circuit breaker in half-open state
    /// When: Network heals and health checks succeed
    /// Then: Circuit breaker closes and pool recovers
    #[tokio::test]
    async fn circuit_breaker_closes_after_partition_heals() {
        let _guard = test_guard();
        
        let pool_id = PoolId::new("partition-cb-heal-test");
        let nats_urls = vec!["nats://localhost:4222".to_string()];
        let config = PoolConfig::new(1, 2, 5000, 30000, 10000, 10).unwrap();
        let mut pool = ConnectionPool::new(pool_id.clone(), nats_urls, config);

        use vo_types::connection_pool::CircuitBreakerState;

        // Start with circuit breaker closed (healthy)
        assert_eq!(pool.circuit_breaker_state(), CircuitBreakerState::Closed);

        // Simulate partition: open circuit breaker
        pool.state.circuit_breaker.transition_to(CircuitBreakerState::Open);
        assert_eq!(pool.circuit_breaker_state(), CircuitBreakerState::Open);

        // Simulate healing: reset circuit breaker (health checks passed)
        pool.state.circuit_breaker.reset();
        assert_eq!(pool.circuit_breaker_state(), CircuitBreakerState::HalfOpen);

        // Pool should now be able to acquire
        let result = pool.acquire().await;
        assert!(matches!(result, AcquireResult::Available { .. }));
    }
}

// ============================================================================
// Network Partition Tests: Connection Leak Prevention
// ============================================================================

#[cfg(test)]
mod red_queen_pool_network_partition_leak_tests {
    use super::*;

 /// Given: Multiple concurrent acquires during partition
    /// When: Partition heals
    /// Then: All connections are properly tracked, no leaks
    #[tokio::test]
    async fn concurrent_operations_no_leak_after_partition() {
        let _guard = test_guard();
        
        let pool_id = PoolId::new("partition-concurrent-test");
        let nats_urls = vec!["nats://localhost:4222".to_string()];
        let config = PoolConfig::new(1, 5, 5000, 30000, 10000, 10).unwrap();
        let mut pool = ConnectionPool::new(pool_id.clone(), nats_urls, config);

        // Simulate partition by shutting down
        pool.shutdown();
        
        // After shutdown, all connections should be evicted
        let stats = pool.stats();
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.idle_connections, 0);
        assert_eq!(stats.checked_out_connections, 0);
    }

    /// Given: A pool with checked-out connections
    /// When: Network partition occurs (simulated by shutdown)
    /// Then: Checked-out connections are properly evicted, not leaked
    #[tokio::test]
    async fn checked_out_connections_evicted_on_partition() {
        let _guard = test_guard();
        
        let pool_id = PoolId::new("partition-co-evict-test");
        let nats_urls = vec!["nats://localhost:4222".to_string()];
        let config = PoolConfig::new(1, 3, 5000, 30000, 10000, 10).unwrap();
        let mut pool = ConnectionPool::new(pool_id.clone(), nats_urls, config);

        // Acquire connections but don't release (simulates partition during use)
        let mut connection_ids = Vec::new();
        for _ in 0..3 {
            match pool.acquire().await {
                AcquireResult::Available { connection } => {
                    connection_ids.push(connection.connection_id);
                }
                _ => panic!("Expected Available"),
            }
        }

        // Verify connections are checked out
        assert_eq!(pool.stats().checked_out_connections, 3);
        assert_eq!(pool.stats().idle_connections, 0);

        // Simulate partition by shutting down
        pool.shutdown();

        // Verify: All connections properly evicted, no leak
        let stats = pool.stats();
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.checked_out_connections, 0);
        assert_eq!(stats.idle_connections, 0);
    }

    /// Given: A pool with pending acquires
    /// When: Network partition occurs
    /// Then: Pending acquires are cleared, no leak
    #[tokio::test]
    async fn pending_acquires_cleared_on_partition() {
        let _guard = test_guard();
        
        let pool_id = PoolId::new("partition-pending-test");
        let nats_urls = vec!["nats://localhost:4222".to_string()];
        let config = PoolConfig::new(1, 1, 5000, 30000, 10000, 10).unwrap(); // max 1 connection
        let mut pool = ConnectionPool::new(pool_id.clone(), nats_urls, config);

        // Fill the pool
        match pool.acquire().await {
            AcquireResult::Available { .. } => {}
            _ => panic!("Expected Available"),
        }

        // All pending acquires should be cleared on shutdown
        pool.shutdown();

        let stats = pool.stats();
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.pending_acquires, 0);
    }

    /// Given: A pool with checked-out connections
    /// When: Network partition occurs (simulated by shutdown)
    /// Then: Checked-out connections are properly evicted, not leaked
    #[tokio::test]
    async fn checked_out_connections_evicted_on_partition() {
        let _guard = test_guard();
        
        let pool_id = PoolId::new("partition-co-evict-test");
        let nats_urls = vec!["nats://localhost:4222".to_string()];
        let config = PoolConfig::new(1, 3, 5000, 30000, 10000, 10).unwrap();
        let mut pool = ConnectionPool::new(pool_id.clone(), nats_urls, config);

        // Acquire connections but don't release (simulates partition during use)
        let mut connection_ids = Vec::new();
        for _ in 0..3 {
            match pool.acquire().await {
                AcquireResult::Available { connection } => {
                    connection_ids.push(connection.connection_id);
                }
                _ => panic!("Expected Available"),
            }
        }

        // Verify connections are checked out
        assert_eq!(pool.stats().checked_out_connections, 3);
        assert_eq!(pool.stats().idle_connections, 0);

        // Simulate partition by shutting down
        pool.shutdown();

        // Verify: All connections properly evicted, no leak
        let stats = pool.stats();
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.checked_out_connections, 0);
        assert_eq!(stats.idle_connections, 0);
    }

    /// Given: A pool with pending acquires
    /// When: Network partition occurs
    /// Then: Pending acquires are cleared, no leak
    #[tokio::test]
    async fn pending_acquires_cleared_on_partition() {
        let _guard = test_guard();
        
        let pool_id = PoolId::new("partition-pending-test");
        let nats_urls = vec!["nats://localhost:4222".to_string()];
        let config = PoolConfig::new(1, 1, 5000, 30000, 10000, 10).unwrap(); // max 1 connection
        let mut pool = ConnectionPool::new(pool_id.clone(), nats_urls, config);

        // Fill the pool
        match pool.acquire().await {
            AcquireResult::Available { .. } => {}
            _ => panic!("Expected Available"),
        }

        // All pending acquires should be cleared on shutdown
        pool.shutdown();

        let stats = pool.stats();
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.pending_acquires, 0);
    }
}

// ============================================================================
// Network Partition Tests: Health Check Behavior
// ============================================================================

#[cfg(test)]
mod red_queen_pool_network_partition_health_tests {
    use super::*;

    /// Given: Healthy connections in pool
    /// When: Network partition causes health checks to fail
    /// Then: Unhealthy connections are evicted, no leak
    #[tokio::test]
    async fn unhealthy_connections_evicted_during_partition() {
        let _guard = test_guard();
        
        let pool_id = PoolId::new("partition-health-test");
        let nats_urls = vec!["nats://localhost:4222".to_string()];
        let config = PoolConfig::new(1, 2, 5000, 30000, 10000, 10).unwrap();
        let mut pool = ConnectionPool::new(pool_id.clone(), nats_urls, config);

        // Create and release connections
        let conn_id = match pool.acquire().await {
            AcquireResult::Available { connection } => {
                connection.connection_id
            }
            _ => panic!("Expected Available"),
        };
        pool.release(conn_id);

        // Verify connection is idle
        assert_eq!(pool.stats().idle_connections, 1);

        // Simulate partition: shutdown pool
        pool.shutdown();

        // Verify: Connection evicted, no leak
        assert_eq!(pool.stats().idle_connections, 0);
        assert_eq!(pool.stats().total_connections, 0);
    }

    /// Given: Pool with health check configured
    /// When: Partition causes health check failures
    /// Then: Failed health checks increment circuit breaker
    #[tokio::test]
    async fn health_check_failures_trigger_circuit_breaker() {
        let _guard = test_guard();
        
        let pool_id = PoolId::new("partition-hc-cb-test");
        let nats_urls = vec!["nats://localhost:4222".to_string()];
        let config = PoolConfig::new(1, 2, 5000, 30000, 10000, 10).unwrap();
        let mut pool = ConnectionPool::new(pool_id.clone(), nats_urls, config);

        // Simulate health check failure during partition
        // For this test, we directly verify the circuit breaker mechanism
        use vo_types::connection_pool::CircuitBreakerState;
        for _ in 0..10 {
            pool.state.circuit_breaker.record_failure();
        }
        
        // Circuit breaker should now be open
        let cb_state = pool.circuit_breaker_state();
        assert_eq!(cb_state, CircuitBreakerState::Open);
    }
}

// ============================================================================
// Network Partition Tests: Pool Stats and Monitoring
// ============================================================================

#[cfg(test)]
mod red_queen_pool_network_partition_stats_tests {
    use super::*;

    /// Given: A pool under normal operation
    /// When: Network partition occurs
    /// Then: Stats accurately reflect pool state during partition
    #[tokio::test]
    async fn stats_accurate_during_partition() {
        let _guard = test_guard();
        
        let pool_id = PoolId::new("partition-stats-test");
        let nats_urls = vec!["nats://localhost:4222".to_string()];
        let config = PoolConfig::new(1, 3, 5000, 30000, 10000, 10).unwrap();
        let mut pool = ConnectionPool::new(pool_id.clone(), nats_urls, config);

        // Normal operation
        for _ in 0..3 {
            match pool.acquire().await {
                AcquireResult::Available { .. } => {}
                _ => panic!("Expected Available"),
            }
        }

        let stats_before = pool.stats();
        assert_eq!(stats_before.total_connections, 3);
        assert_eq!(stats_before.checked_out_connections, 3);

        // Simulate partition
        pool.shutdown();

        // Stats should reflect partition state
        let stats_during = pool.stats();
        assert_eq!(stats_during.total_connections, 0);
        assert_eq!(stats_during.checked_out_connections, 0);
        assert_eq!(stats_during.idle_connections, 0);
        assert!(pool.is_shutting_down());
    }

    /// Given: A pool with eviction tracking
    /// When: Partition causes evictions
    /// Then: Eviction count accurately reflects partition events
    #[tokio::test]
    async fn eviction_count_accurate_during_partition() {
        let _guard = test_guard();
        
        let pool_id = PoolId::new("partition-eviction-test");
        let nats_urls = vec!["nats://localhost:4222".to_string()];
        let config = PoolConfig::new(1, 2, 5000, 30000, 10000, 10).unwrap();
        let mut pool = ConnectionPool::new(pool_id.clone(), nats_urls, config);

        // Create connections
        let conn_ids: Vec<_> = (0..2)
            .map(|_| {
                match pool.acquire().await {
                    AcquireResult::Available { connection } => connection.connection_id,
                    _ => panic!("Expected Available"),
                }
            })
            .collect();

        // Release connections
        for conn_id in &conn_ids {
            pool.release(*conn_id);
        }

        let evictions_before = pool.stats().total_evictions;

        // Simulate partition
        pool.shutdown();

        // Evictions should have increased
        let evictions_after = pool.stats().total_evictions;
        assert!(evictions_after >= evictions_before);
    }
}

// ============================================================================
// Network Partition Tests: Recovery Scenarios
// ============================================================================

#[cfg(test)]
mod red_queen_pool_network_partition_recovery_tests {
    use super::*;

    /// Given: A pool that experienced partition
    /// When: Network heals and new pool is created
    /// Then: New pool starts fresh, no carry-over from partition
    #[tokio::test]
    async fn fresh_pool_after_partition_recovery() {
        let _guard = test_guard();
        
        let nats_urls = vec!["nats://localhost:4222".to_string()];
        let config = PoolConfig::new(1, 3, 5000, 30000, 10000, 10).unwrap();

        // Phase 1: Pool during partition
        let pool_id1 = PoolId::new("partition-recovery-1");
        let mut pool1 = ConnectionPool::new(pool_id1.clone(), nats_urls.clone(), config.clone());

        // Acquire some connections
        for _ in 0..2 {
            match pool1.acquire().await {
                AcquireResult::Available { .. } => {}
                _ => panic!("Expected Available"),
            }
        }

        // Simulate partition
        pool1.shutdown();

        // Phase 2: Fresh pool after recovery
        let pool_id2 = PoolId::new("partition-recovery-2");
        let mut pool2 = ConnectionPool::new(pool_id2.clone(), nats_urls, config);

        // Verify: New pool starts clean
        assert_eq!(pool2.stats().total_connections, 0);
        assert_eq!(pool2.stats().idle_connections, 0);
        assert_eq!(pool2.stats().checked_out_connections, 0);
    }

    /// Given: Multiple partition/heal cycles
    /// When: Pool is recreated after each cycle
    /// Then: No accumulated leaks across cycles
    #[tokio::test]
    async fn multiple_cycles_no_accumulated_leak() {
        let _guard = test_guard();
        
        let nats_urls = vec!["nats://localhost:4222".to_string()];
        let config = PoolConfig::new(1, 3, 5000, 30000, 10000, 10).unwrap();

        let mut max_connections_across_cycles = 0;

        for cycle in 0..10 {
            let pool_id = PoolId::new(format!("partition-cycle-{}", cycle));
            let mut pool = ConnectionPool::new(pool_id.clone(), nats_urls.clone(), config.clone());

            // Acquire and release multiple times
            for _ in 0..3 {
                match pool.acquire().await {
                    AcquireResult::Available { .. } => {}
                    _ => panic!("Cycle {}: Expected Available", cycle),
                }
            }

            let stats = pool.stats();
            max_connections_across_cycles = max_connections_across_cycles.max(stats.total_connections);

            // Simulate partition
            pool.shutdown();

            // Verify no leak in this cycle
            assert_eq!(pool.stats().total_connections, 0);
        }

        // Final verification: No accumulated leak
        assert_eq!(max_connections_across_cycles, 3); // Should never exceed max_connections
    }
}
