//! Tests for connection lifecycle cleanup on pool close (ve-wf7a0).
//!
//! Covers: graceful close with idle connections, force close with checked-out
//! connections, leak detection (no connections remain after shutdown),
//! shutdown idempotency, acquire rejection after shutdown, pending acquire
//! cancellation, and release-during-shutdown eviction behavior.

use super::super::config::PoolConfig;
use super::super::pool::{ConnectionPool, PoolState};
use vo_types::connection_pool::{
    AcquireResult, ConnectionId, ConnectionStatus, EvictionReason, PoolId, ReleaseResult,
};

fn create_test_pool(max_conn: u32, max_pending: u32) -> ConnectionPool {
    let pool_id = PoolId::new("lifecycle-test");
    let nats_urls = vec!["nats://localhost:4222".to_string()];
    let config = PoolConfig::new(1, max_conn, 5000, 30000, 10000, max_pending).unwrap();
    ConnectionPool::new(pool_id, nats_urls, config)
}

/// Helper: acquire a connection and return its id.
fn acquire_conn(pool: &mut ConnectionPool) -> ConnectionId {
    match futures::executor::block_on(pool.acquire()) {
        AcquireResult::Available { connection } => connection.connection_id,
        other => panic!("Expected Available, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Graceful close: all idle connections cleaned up
// ---------------------------------------------------------------------------

#[test]
fn graceful_close_clears_idle_connections() {
    let mut pool = create_test_pool(5, 0);

    // Acquire and release 3 connections → they go idle
    let c1 = acquire_conn(&mut pool);
    let c2 = acquire_conn(&mut pool);
    let c3 = acquire_conn(&mut pool);
    pool.release(c1);
    pool.release(c2);
    pool.release(c3);
    assert_eq!(pool.stats().idle_connections, 3);

    pool.shutdown();

    assert_eq!(pool.stats().total_connections, 0);
    assert_eq!(pool.stats().idle_connections, 0);
    assert!(pool.is_shutting_down());
}

#[test]
fn graceful_close_allows_zero_connections() {
    let mut pool = create_test_pool(3, 0);
    // No connections ever acquired — shutdown should be safe
    pool.shutdown();
    assert_eq!(pool.stats().total_connections, 0);
    assert!(pool.is_shutting_down());
}

// ---------------------------------------------------------------------------
// Force close: checked-out connections evicted
// ---------------------------------------------------------------------------

#[test]
fn force_close_evicts_checked_out_connections() {
    let mut pool = create_test_pool(5, 0);

    // Acquire 3 connections but don't release (simulating checked-out on close)
    let _c1 = acquire_conn(&mut pool);
    let _c2 = acquire_conn(&mut pool);
    let _c3 = acquire_conn(&mut pool);
    assert_eq!(pool.stats().checked_out_connections, 3);

    pool.shutdown();

    assert_eq!(pool.stats().total_connections, 0);
    assert_eq!(pool.stats().checked_out_connections, 0);
}

#[test]
fn force_close_mixed_idle_and_checked_out() {
    let mut pool = create_test_pool(5, 0);

    let c1 = acquire_conn(&mut pool);
    let c2 = acquire_conn(&mut pool);
    let _c3 = acquire_conn(&mut pool);
    let _c4 = acquire_conn(&mut pool);
    pool.release(c1);
    pool.release(c2);

    assert_eq!(pool.stats().idle_connections, 2);
    assert_eq!(pool.stats().checked_out_connections, 2);

    pool.shutdown();

    assert_eq!(pool.stats().total_connections, 0);
    assert_eq!(pool.stats().idle_connections, 0);
    assert_eq!(pool.stats().checked_out_connections, 0);
}

// ---------------------------------------------------------------------------
// Leak detection: no connections survive shutdown
// ---------------------------------------------------------------------------

#[test]
fn shutdown_no_connection_leaks() {
    let mut pool = create_test_pool(10, 5);

    // Create various connection states
    let c1 = acquire_conn(&mut pool);
    let c2 = acquire_conn(&mut pool);
    let c3 = acquire_conn(&mut pool);
    let _c4 = acquire_conn(&mut pool);
    pool.release(c1);
    pool.release(c2);
    pool.release(c3);
    // c4 is still checked out

    assert_eq!(pool.stats().total_connections, 4);

    pool.shutdown();

    let stats = pool.stats();
    assert_eq!(stats.total_connections, 0, "no connections should remain after shutdown");
    assert_eq!(stats.idle_connections, 0, "no idle connections should remain");
    assert_eq!(stats.checked_out_connections, 0, "no checked-out connections should remain");
    assert_eq!(stats.pending_acquires, 0, "no pending acquires should remain");
}

#[test]
fn shutdown_at_max_capacity_no_leaks() {
    let mut pool = create_test_pool(3, 0);

    let _c1 = acquire_conn(&mut pool);
    let _c2 = acquire_conn(&mut pool);
    let _c3 = acquire_conn(&mut pool);
    assert_eq!(pool.stats().total_connections, 3);

    pool.shutdown();
    assert_eq!(pool.stats().total_connections, 0);
}

// ---------------------------------------------------------------------------
// Shutdown rejects new acquires
// ---------------------------------------------------------------------------

#[test]
fn acquire_after_shutdown_returns_pool_closing() {
    let mut pool = create_test_pool(3, 0);
    pool.shutdown();

    let result = futures::executor::block_on(pool.acquire());
    assert!(
        matches!(result, AcquireResult::PoolClosing),
        "expected PoolClosing after shutdown, got {:?}",
        result
    );
}

#[test]
fn acquire_with_timeout_after_shutdown_returns_pool_closing() {
    let mut pool = create_test_pool(3, 0);
    pool.shutdown();

    let result = futures::executor::block_on(
        pool.acquire_with_timeout(std::time::Duration::from_millis(100)),
    );
    assert!(matches!(result, AcquireResult::PoolClosing));
}

// ---------------------------------------------------------------------------
// Pending acquires cancelled on shutdown
// ---------------------------------------------------------------------------

#[test]
fn shutdown_cancels_pending_acquires() {
    let mut pool = create_test_pool(1, 5);

    // Fill the pool
    let _c1 = acquire_conn(&mut pool);
    // The second acquire should be pending (max_pending=5)
    let result = futures::executor::block_on(pool.acquire());
    assert!(matches!(result, AcquireResult::Pending { .. }));
    assert_eq!(pool.stats().pending_acquires, 1);

    pool.shutdown();
    assert_eq!(pool.stats().pending_acquires, 0, "pending acquires should be cleared");
}

// ---------------------------------------------------------------------------
// Shutdown idempotency
// ---------------------------------------------------------------------------

#[test]
fn shutdown_is_idempotent() {
    let mut pool = create_test_pool(3, 0);
    let _c = acquire_conn(&mut pool);

    pool.shutdown();
    pool.shutdown(); // Second shutdown should not panic
    pool.shutdown(); // Third shutdown should not panic

    assert!(pool.is_shutting_down());
    assert_eq!(pool.stats().total_connections, 0);
}

// ---------------------------------------------------------------------------
// Release during shutdown evicts instead of returning
// ---------------------------------------------------------------------------

#[test]
fn release_after_shutdown_returns_already_closed() {
    let mut pool = create_test_pool(3, 0);
    let conn_id = acquire_conn(&mut pool);

    pool.shutdown();
    // Connection was already cleaned up during shutdown — release returns AlreadyClosed
    let result = pool.release(conn_id);
    assert_eq!(result, ReleaseResult::AlreadyClosed);
    assert_eq!(pool.stats().total_connections, 0);
}

// ---------------------------------------------------------------------------
// Eviction during shutdown
// ---------------------------------------------------------------------------

#[test]
fn evict_connection_during_shutdown() {
    let mut pool = create_test_pool(3, 0);
    let conn_id = acquire_conn(&mut pool);

    pool.shutdown();

    let result = pool.evict_connection(conn_id, EvictionReason::ExplicitEviction);
    // Already evicted during shutdown
    assert_eq!(result, ReleaseResult::AlreadyClosed);
}

// ---------------------------------------------------------------------------
// Stats consistency after shutdown
// ---------------------------------------------------------------------------

#[test]
fn stats_consistent_after_shutdown_with_prior_activity() {
    let mut pool = create_test_pool(5, 5);

    // Build up some stats
    let c1 = acquire_conn(&mut pool);
    let c2 = acquire_conn(&mut pool);
    pool.release(c1);
    let _c3 = acquire_conn(&mut pool);
    pool.evict_connection(c2, EvictionReason::ExplicitEviction);

    let pre_shutdown_acquires = pool.stats().total_acquires;
    let pre_shutdown_releases = pool.stats().total_releases;
    let pre_shutdown_evictions = pool.stats().total_evictions;

    pool.shutdown();

    let stats = pool.stats();
    // Counters should not decrease
    assert!(stats.total_acquires >= pre_shutdown_acquires);
    assert!(stats.total_releases >= pre_shutdown_releases);
    assert!(stats.total_evictions >= pre_shutdown_evictions);
    // All connection pools empty
    assert_eq!(stats.total_connections, 0);
    assert_eq!(stats.idle_connections, 0);
    assert_eq!(stats.checked_out_connections, 0);
    assert_eq!(stats.pending_acquires, 0);
}
