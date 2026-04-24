//! Red Queen adversarial tests for Connection Pool Manager types.
//!
//! bead_id: ve-n1ms
//! phase: Red Queen (adversarial testing)
//!
//! Dimensions attacked:
//!   - contract-violations: Public fields bypass validation, invalid PoolConfig
//!   - invariant-attacks: INV-010 tautology proof, invalid connection counts
//!   - serde-attacks: Deserialize invalid configs, wrong types, boundary values
//!   - display-impl: ConnectionPoolError Display coverage
//!   - edge-cases: Zero timeouts, max values, negative-like values
//!   - state-corruption: Direct field manipulation to create inconsistent states

use vo_common::connection_pool::{
    AcquireResult, CircuitBreakerState, ConnectionId, ConnectionPoolError, ConnectionStatus,
    ErrorCategory, ErrorContext, ErrorDetail, EvictionReason, HealthCheckResult, PoolConfig,
    PoolId, PoolStats, PooledConnection, ReleaseResult, WaitHandle,
};
use vo_common::connection_pool::TimestampMs;

// ===========================================================================
// DIMENSION: contract-violations
// Public fields allow bypassing any validation
// ===========================================================================

/// RQ-01: PoolConfig with min > max is allowed via direct construction
/// Contract says INV-001: min_connections <= max_connections at all times
/// But public fields allow bypassing this invariant
#[test]
fn rq_pool_config_min_gt_max_allowed_via_public_fields() {
    let config = PoolConfig {
        min_connections: 100,
        max_connections: 10,
        connection_timeout_ms: 5000,
        idle_timeout_ms: 30000,
        health_check_interval_ms: 10000,
        max_pending_acquires: 5,
    };
    assert!(
        config.min_connections > config.max_connections,
        "Contract violated: min > max"
    );
}

/// RQ-02: PoolConfig with zero timeouts allowed via public fields
/// Zero timeout may cause immediate timeouts or infinite waits
#[test]
fn rq_pool_config_zero_timeouts_allowed_via_public_fields() {
    let config = PoolConfig {
        min_connections: 1,
        max_connections: 10,
        connection_timeout_ms: 0,
        idle_timeout_ms: 0,
        health_check_interval_ms: 0,
        max_pending_acquires: 5,
    };
    assert_eq!(config.connection_timeout_ms, 0);
    assert_eq!(config.idle_timeout_ms, 0);
    assert_eq!(config.health_check_interval_ms, 0);
}

/// RQ-03: PooledConnection with inconsistent status via with_status()
/// Status can be set to any value regardless of actual state
#[test]
fn rq_pooled_connection_status_independent_of_metadata() {
    let conn_id = ConnectionId::new();
    let timestamp = TimestampMs::new_unchecked(1000);

    // Create connection in CheckedOut state but use_count is 0
    // This is inconsistent - checked out connections should have been used
    let conn = PooledConnection::new(conn_id, timestamp)
        .with_status(ConnectionStatus::CheckedOut)
        .with_use_count(0);

    assert!(conn.is_checked_out());
    assert_eq!(
        conn.use_count, 0,
        "CheckedOut but never used - inconsistent state"
    );
}

/// RQ-04: PooledConnection can have status set to Idle while checked out
/// No enforcement that status transitions are valid
#[test]
fn rq_pooled_connection_can_retransitions_to_idle() {
    let conn_id = ConnectionId::new();
    let timestamp = TimestampMs::new_unchecked(1000);

    let conn = PooledConnection::new(conn_id, timestamp).with_status(ConnectionStatus::CheckedOut);

    // Manually set back to Idle (simulating what direct field access could do)
    let retransitioned = PooledConnection {
        connection_id: conn.connection_id,
        created_at: conn.created_at,
        last_used_at: conn.last_used_at,
        use_count: conn.use_count,
        status: ConnectionStatus::Idle, // Retransitioned to Idle
    };

    assert!(!retransitioned.is_checked_out());
    assert!(retransitioned.is_idle());
}

// ===========================================================================
// DIMENSION: invariant-attacks
// Test the invariants to prove they are not enforced
// ===========================================================================

/// RQ-05: INV-010 is a tautology - proof
/// The test at line 1321-1346 computes:
///   stats.total_connections == stats.total_connections
/// This always passes and provides ZERO validation
#[test]
fn rq_inv_010_is_a_tautology() {
    let stats = PoolStats {
        pool_id: PoolId::new("test-pool"),
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

    // The tautology: total == total
    // This proves nothing about actual pool state
    let tautology_left = stats.total_connections;
    let tautology_right = stats.total_connections;
    assert_eq!(
        tautology_left, tautology_right,
        "Tautology: X == X always passes"
    );

    // A proper invariant would check: idle + checked_out + [other states] == total
    // But PoolStats doesn't track HealthCheck/Closing/Closed counts separately
    // So we cannot verify the actual invariant
}

/// RQ-06: INV-002 cannot be verified without max_connections context
/// The invariant: checked_out + idle + pending <= max + max_pending
/// But PoolStats doesn't store max_connections or max_pending_acquires!
#[test]
fn rq_inv_002_cannot_be_verified_without_config() {
    let stats = PoolStats {
        pool_id: PoolId::new("test-pool"),
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

    // We can compute the left side
    let actual = stats.checked_out_connections + stats.idle_connections + stats.pending_acquires;
    // But we DON'T have max_connections or max_pending_acquires in stats
    // So we CANNOT verify INV-002 without external config context

    // This is a design flaw - stats should include bounds for self-verification
    let _ = actual; // silence unused warning
}

/// RQ-07: Connection status counts don't add up to total
/// INV-002 would require: idle + checked_out + health_check + closing + closed = total
/// But only idle and checked_out are tracked!
#[test]
fn rq_connection_status_counts_incomplete() {
    let stats = PoolStats {
        pool_id: PoolId::new("test-pool"),
        total_connections: 10,
        idle_connections: 5,
        checked_out_connections: 3,
        pending_acquires: 2, // This is NOT a connection state
        total_acquires: 100,
        total_releases: 95,
        total_evictions: 5,
        total_health_checks: 50,
        failed_health_checks: 3,
    };

    // 5 + 3 = 8, but total is 10
    // Missing 2 connections are in HealthCheck/Closing/Closed states
    // But stats don't track these separately!
    let tracked = stats.idle_connections + stats.checked_out_connections;
    let missing = stats.total_connections - tracked;
    assert_eq!(missing, 2, "2 connections in untracked states");
}

/// RQ-08: use_count can be set to any value via with_use_count()
/// INV-004: use_count monotonically increases
/// But with_use_count() allows setting it to any value, breaking monotonicity
#[test]
fn rq_use_count_monotonicity_broken_by_builder() {
    let conn_id = ConnectionId::new();
    let timestamp = TimestampMs::new_unchecked(1000);

    let mut conn = PooledConnection::new(conn_id, timestamp);
    conn.increment_use_count();
    conn.increment_use_count();
    assert_eq!(conn.use_count, 2);

    // Now "reset" to 0 using with_use_count
    let reset_conn = conn.clone().with_use_count(0);
    assert_eq!(
        reset_conn.use_count, 0,
        "Monotonicity violated - use_count reset to 0"
    );
    assert!(
        reset_conn.use_count < conn.use_count,
        "Monotonicity violated"
    );
}

// ===========================================================================
// DIMENSION: serde-attacks
// Deserialization bypasses validation
// ===========================================================================

/// RQ-09: PoolConfig JSON with min > max deserializes successfully
/// This violates INV-001 but serde doesn't care
#[test]
fn rq_pool_config_json_min_gt_max_deserializes() {
    let json = r#"{
        "min_connections": 100,
        "max_connections": 10,
        "connection_timeout_ms": 5000,
        "idle_timeout_ms": 30000,
        "health_check_interval_ms": 10000,
        "max_pending_acquires": 5
    }"#;

    let config: PoolConfig =
        serde_json::from_str(json).expect("Deserialization succeeds but shouldn't");
    assert!(
        config.min_connections > config.max_connections,
        "Contract violated via serde"
    );
}

/// RQ-10: PoolConfig JSON with zero timeouts deserializes successfully
#[test]
fn rq_pool_config_json_zero_timeouts() {
    let json = r#"{
        "min_connections": 1,
        "max_connections": 10,
        "connection_timeout_ms": 0,
        "idle_timeout_ms": 0,
        "health_check_interval_ms": 0,
        "max_pending_acquires": 5
    }"#;

    let config: PoolConfig = serde_json::from_str(json).expect("Zero timeouts deserialize");
    assert_eq!(config.connection_timeout_ms, 0);
}

/// RQ-11: PoolConfig JSON with u32::MAX values deserializes
#[test]
fn rq_pool_config_json_max_u32_values() {
    let json = format!(
        r#"{{
        "min_connections": {},
        "max_connections": {},
        "connection_timeout_ms": {},
        "idle_timeout_ms": {},
        "health_check_interval_ms": {},
        "max_pending_acquires": {}
    }}"#,
        u32::MAX,
        u32::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u32::MAX
    );

    let config: PoolConfig = serde_json::from_str(&json).expect("Max u32 values deserialize");
    assert_eq!(config.min_connections, u32::MAX);
    assert_eq!(config.max_connections, u32::MAX);
}

/// RQ-12: PoolStats JSON round-trip preserves values
#[test]
fn rq_pool_stats_serde_round_trip() {
    let stats = PoolStats {
        pool_id: PoolId::new("test-pool"),
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

    let json = serde_json::to_value(&stats).unwrap();
    let restored: PoolStats = serde_json::from_value(json).unwrap();
    assert_eq!(restored, stats);
}

/// RQ-13: ConnectionId JSON round-trip
#[test]
fn rq_connection_id_serde_round_trip() {
    let id = ConnectionId::new();
    let json = serde_json::to_value(&id).unwrap();
    let restored: ConnectionId = serde_json::from_value(json).unwrap();
    assert_eq!(restored, id);
}

/// RQ-14: PooledConnection JSON round-trip
#[test]
fn rq_pooled_connection_serde_round_trip() {
    let conn = PooledConnection::new(ConnectionId::new(), TimestampMs::new_unchecked(1000))
        .with_status(ConnectionStatus::CheckedOut)
        .with_use_count(42);

    let json = serde_json::to_value(&conn).unwrap();
    let restored: PooledConnection = serde_json::from_value(json).unwrap();
    assert_eq!(restored, conn);
}

// ===========================================================================
// DIMENSION: display-impl
// ConnectionPoolError Display coverage
// ===========================================================================

/// RQ-15: ConnectionPoolError Display impl exists and produces output
#[test]
fn rq_connection_pool_error_display() {
    let error = ConnectionPoolError {
        category: ErrorCategory::PoolExhaustion,
        detail: ErrorDetail::MaxConnectionsReached { max: 10 },
        context: ErrorContext {
            pool_id: PoolId::new("test-pool"),
            timestamp: TimestampMs::new_unchecked(1000),
            operation: "acquire",
            connection_id: Some(ConnectionId::new()),
        },
    };

    let display = format!("{}", error);
    assert!(
        !display.is_empty(),
        "ConnectionPoolError Display should produce output"
    );
    assert!(
        display.contains("PoolExhaustion"),
        "Display should contain category"
    );
}

/// RQ-16: All ErrorDetail variants have Display via to_string()
#[test]
fn rq_error_detail_all_variants_display() {
    let variants = [
        ErrorDetail::MaxConnectionsReached { max: 10 },
        ErrorDetail::PendingAcquiresExceeded { max: 5 },
        ErrorDetail::AcquireTimeout {
            waited_ms: 5000,
            timeout_ms: 10000,
        },
        ErrorDetail::NatsConnectionError {
            connection_id: ConnectionId::new(),
            reason: "connection reset".to_string(),
        },
        ErrorDetail::HealthCheckTimeout {
            connection_id: ConnectionId::new(),
        },
        ErrorDetail::ConnectionCorrupted {
            connection_id: ConnectionId::new(),
        },
        ErrorDetail::InvalidRelease {
            reason: "not from this pool",
        },
        ErrorDetail::PoolNotInitialized,
        ErrorDetail::AlreadyShutdown,
        ErrorDetail::CircuitBreakerOpen {
            consecutive_failures: 5,
        },
    ];

    for variant in variants {
        let display = variant.clone().to_string();
        assert!(
            !display.is_empty(),
            "ErrorDetail variant {:?} should have Display",
            variant
        );
    }
}

/// RQ-17: ErrorCategory Display impl
#[test]
fn rq_error_category_display_all_variants() {
    let variants = [
        ErrorCategory::PoolExhaustion,
        ErrorCategory::Timeout,
        ErrorCategory::ConnectionFailed,
        ErrorCategory::HealthCheckFailed,
        ErrorCategory::InvalidState,
        ErrorCategory::ShutdownInProgress,
        ErrorCategory::ResourceExhaustion,
    ];

    for variant in variants {
        let display = format!("{}", variant);
        assert!(
            !display.is_empty(),
            "ErrorCategory variant {:?} should Display",
            variant
        );
    }
}

// ===========================================================================
// DIMENSION: edge-cases
// Boundary values and edge cases
// ===========================================================================

/// RQ-18: ConnectionId ULID properties - uniqueness
#[test]
fn rq_connection_id_uniqueness() {
    let ids: Vec<ConnectionId> = (0..1000).map(|_| ConnectionId::new()).collect();
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(
        unique.len(),
        1000,
        "All 1000 ConnectionIds should be unique"
    );
}

/// RQ-19: ConnectionId ULID properties - time ordering
#[test]
fn rq_connection_id_time_ordered() {
    let id1 = ConnectionId::new();
    std::thread::sleep(std::time::Duration::from_millis(1));
    let id2 = ConnectionId::new();

    // ULIDs are lexicographically sortable by time
    assert!(
        id1.to_string() < id2.to_string(),
        "Later ULID should be lexicographically greater"
    );
}

/// RQ-20: PooledConnection increment_use_count stress test
#[test]
fn rq_pooled_connection_use_count_stress() {
    let conn_id = ConnectionId::new();
    let mut conn = PooledConnection::new(conn_id, TimestampMs::new_unchecked(1000));

    for _ in 0..1000 {
        conn.increment_use_count();
    }
    assert_eq!(conn.use_count, 1000);
}

/// RQ-21: All ConnectionStatus variants can be constructed
#[test]
fn rq_connection_status_all_variants() {
    let _ = ConnectionStatus::Idle;
    let _ = ConnectionStatus::CheckedOut;
    let _ = ConnectionStatus::HealthCheck;
    let _ = ConnectionStatus::Closing;
    let _ = ConnectionStatus::Closed;
}

/// RQ-22: All HealthCheckResult variants can be constructed
#[test]
fn rq_health_check_result_all_variants() {
    let _ = HealthCheckResult::Healthy;
    let _ = HealthCheckResult::Stale;
    let _ = HealthCheckResult::Corrupted;
    let _ = HealthCheckResult::Timeout;
}

/// RQ-23: All CircuitBreakerState variants
#[test]
fn rq_circuit_breaker_state_all_variants() {
    let _ = CircuitBreakerState::Closed;
    let _ = CircuitBreakerState::Open;
    let _ = CircuitBreakerState::HalfOpen;
}

/// RQ-24: All EvictionReason variants
#[test]
fn rq_eviction_reason_all_variants() {
    let _ = EvictionReason::HealthCheckFailed(HealthCheckResult::Healthy);
    let _ = EvictionReason::ExplicitEviction;
    let _ = EvictionReason::IdleTimeout;
    let _ = EvictionReason::ProtocolError("test".to_string());
}

/// RQ-25: All AcquireResult variants
#[test]
fn rq_acquire_result_all_variants() {
    let config = PoolConfig {
        min_connections: 1,
        max_connections: 10,
        connection_timeout_ms: 5000,
        idle_timeout_ms: 30000,
        health_check_interval_ms: 10000,
        max_pending_acquires: 5,
    };

    let _ = AcquireResult::Available {
        connection: PooledConnection::new(ConnectionId::new(), TimestampMs::new_unchecked(1000)),
    };
    let _ = AcquireResult::Pending {
        wait_handle: WaitHandle {
            request_id: 1,
            enqueued_at: TimestampMs::new_unchecked(1000),
            pool_id: PoolId::new("test"),
        },
    };
    let _ = AcquireResult::PoolExhausted { config };
    let _ = AcquireResult::PoolClosing;
    let _ = AcquireResult::Timeout { waited_ms: 5000 };
}

/// RQ-26: All ReleaseResult variants
#[test]
fn rq_release_result_all_variants() {
    let _ = ReleaseResult::Returned;
    let _ = ReleaseResult::AlreadyClosed;
    let _ = ReleaseResult::Evicted {
        reason: EvictionReason::IdleTimeout,
    };
}

/// RQ-27: PoolStats default is all zeros except pool_id
#[test]
fn rq_pool_stats_default() {
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

/// RQ-28: PooledConnection is_idle, is_checked_out, is_closed
#[test]
fn rq_pooled_connection_status_checkers() {
    let conn_id = ConnectionId::new();
    let timestamp = TimestampMs::new_unchecked(1000);

    let idle = PooledConnection::new(conn_id, timestamp);
    assert!(idle.is_idle());
    assert!(!idle.is_checked_out());
    assert!(!idle.is_closed());

    let checked = idle.with_status(ConnectionStatus::CheckedOut);
    assert!(!checked.is_idle());
    assert!(checked.is_checked_out());
    assert!(!checked.is_closed());

    let closed = checked.with_status(ConnectionStatus::Closed);
    assert!(!closed.is_idle());
    assert!(!closed.is_checked_out());
    assert!(closed.is_closed());
}

// ===========================================================================
// DIMENSION: state-corruption
// Direct field manipulation to create impossible states
// ===========================================================================

/// RQ-29: PooledConnection with future timestamp
#[test]
fn rq_pooled_connection_future_created_at() {
    let conn_id = ConnectionId::new();
    let future = TimestampMs::new_unchecked(u64::MAX);

    let conn = PooledConnection::new(conn_id, future);
    assert!(conn.created_at > conn.last_used_at || conn.created_at == conn.last_used_at);
}

/// RQ-30: PooledConnection last_used_at before created_at (impossible)
#[test]
fn rq_pooled_connection_last_used_before_created() {
    let conn_id = ConnectionId::new();
    let created = TimestampMs::new_unchecked(5000);
    let last_used = TimestampMs::new_unchecked(1000);

    let conn = PooledConnection {
        connection_id: conn_id,
        created_at: created,
        last_used_at: last_used, // Impossible: used before created!
        use_count: 0,
        status: ConnectionStatus::Idle,
    };

    assert!(
        conn.last_used_at < conn.created_at,
        "last_used_at is before created_at"
    );
}

/// RQ-31: PooledConnection with u64::MAX use_count (overflow potential)
#[test]
fn rq_pooled_connection_max_use_count() {
    let conn_id = ConnectionId::new();
    let timestamp = TimestampMs::new_unchecked(1000);

    let conn = PooledConnection {
        connection_id: conn_id,
        created_at: timestamp,
        last_used_at: timestamp,
        use_count: u64::MAX,
        status: ConnectionStatus::Idle,
    };

    assert_eq!(conn.use_count, u64::MAX);
    // increment would overflow
}

/// RQ-32: PoolStats with counts that violate INV-002
#[test]
fn rq_pool_stats_invariant_violation() {
    // idle=5, checked_out=10, pending=5, but total=5
    // 5 + 10 + 5 = 20 > total_connections (5)
    let stats = PoolStats {
        pool_id: PoolId::new("test-pool"),
        total_connections: 5,
        idle_connections: 5,
        checked_out_connections: 10, // More checked out than total!
        pending_acquires: 5,
        total_acquires: 0,
        total_releases: 0,
        total_evictions: 0,
        total_health_checks: 0,
        failed_health_checks: 0,
    };

    assert!(
        stats.checked_out_connections > stats.total_connections,
        "Impossible state: more checked out than total"
    );
}

/// RQ-33: PoolStats with negative-like overflow
#[test]
fn rq_pool_stats_total_mismatch() {
    // idle + checked_out = 3, but total = 10
    // Where are the other 7 connections?
    let stats = PoolStats {
        pool_id: PoolId::new("test-pool"),
        total_connections: 10,
        idle_connections: 2,
        checked_out_connections: 1,
        pending_acquires: 0,
        total_acquires: 0,
        total_releases: 0,
        total_evictions: 0,
        total_health_checks: 0,
        failed_health_checks: 0,
    };

    let visible = stats.idle_connections + stats.checked_out_connections;
    assert!(
        visible < stats.total_connections,
        "7 connections are unaccounted for"
    );
}
