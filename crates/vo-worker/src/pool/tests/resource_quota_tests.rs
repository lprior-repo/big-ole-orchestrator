use vo_types::connection_pool::{ConnectionId, ConnectionStatus, PoolId, PooledConnection};
use vo_types::integer_types::TimestampMs;

use crate::pool::{PoolConfig, PoolState, QuotaError, QuotaStatus, ResourceQuota};

fn test_pool_with_quota() -> PoolState {
    let mut pool = PoolState::new(PoolId::new("quota-test"), PoolConfig::with_defaults());
    pool.resource_quota = Some(
        ResourceQuota::new(5, 10, 1000).unwrap(),
    );
    pool
}

fn add_connections(pool: &mut PoolState, count: u32) {
    for _ in 0..count {
        let id = ConnectionId::new();
        let conn =
            PooledConnection::new(id, TimestampMs::now()).with_status(ConnectionStatus::CheckedOut);
        pool.connections.insert(id, conn);
        pool.checked_out_connections.insert(ConnectionId::new(), id);
    }
}

#[test]
fn resource_quota_can_be_created() {
    let quota = ResourceQuota::new(5, 10, 1000).unwrap();
    assert_eq!(quota.soft_limit, 5);
    assert_eq!(quota.hard_limit, 10);
}

#[test]
fn no_quota_configured_always_allows() {
    let mut pool = PoolState::new(PoolId::new("no-quota"), PoolConfig::with_defaults());
    assert!(pool.resource_quota.is_none());
    assert_eq!(pool.check_quota().unwrap(), QuotaStatus::Ok);
}

#[test]
fn below_soft_limit_returns_ok() {
    let mut pool = test_pool_with_quota();
    add_connections(&mut pool, 3);
    assert_eq!(pool.check_quota().unwrap(), QuotaStatus::Ok);
}

#[test]
fn at_soft_limit_triggers_warning() {
    let mut pool = test_pool_with_quota();
    add_connections(&mut pool, 5);
    assert_eq!(pool.check_quota().unwrap(), QuotaStatus::SoftLimitReached);
}

#[test]
fn above_soft_limit_triggers_warning() {
    let mut pool = test_pool_with_quota();
    add_connections(&mut pool, 7);
    assert_eq!(pool.check_quota().unwrap(), QuotaStatus::SoftLimitReached);
}

#[test]
fn at_hard_limit_is_denied() {
    let mut pool = test_pool_with_quota();
    add_connections(&mut pool, 10);
    assert_eq!(pool.check_quota().unwrap_err(), QuotaError::HardLimitExceeded);
}

#[test]
fn above_hard_limit_is_denied() {
    let mut pool = test_pool_with_quota();
    add_connections(&mut pool, 12);
    assert_eq!(pool.check_quota().unwrap_err(), QuotaError::HardLimitExceeded);
}

#[test]
fn warning_frequency_respects_interval() {
    let mut pool = test_pool_with_quota();
    pool.resource_quota = Some(ResourceQuota::new(5, 10, 5000).unwrap());
    add_connections(&mut pool, 7);

    // First check: should warn
    assert_eq!(pool.check_quota().unwrap(), QuotaStatus::SoftLimitReached);
    assert!(pool.last_quota_warning_at.is_some());

    // Immediate re-check: suppressed (within interval)
    let first_warn_time = pool.last_quota_warning_at;
    assert_eq!(pool.check_quota().unwrap(), QuotaStatus::Ok);
    assert_eq!(pool.last_quota_warning_at, first_warn_time);

    // After interval elapsed: should warn again
    pool.last_quota_warning_at = first_warn_time.map(|t| {
        TimestampMs::new_unchecked(t.as_u64().saturating_sub(6000))
    });
    assert_eq!(pool.check_quota().unwrap(), QuotaStatus::SoftLimitReached);
}

#[test]
fn soft_limit_must_be_less_than_hard_limit() {
    assert_eq!(ResourceQuota::new(10, 10, 1000).unwrap_err(), QuotaError::SoftNotLessThanHard);
    assert_eq!(ResourceQuota::new(11, 10, 1000).unwrap_err(), QuotaError::SoftNotLessThanHard);
    assert!(ResourceQuota::new(9, 10, 1000).is_ok());
}

#[test]
fn zero_limits_rejected() {
    assert_eq!(ResourceQuota::new(0, 10, 1000).unwrap_err(), QuotaError::ZeroLimit);
    assert_eq!(ResourceQuota::new(5, 0, 1000).unwrap_err(), QuotaError::ZeroLimit);
}

#[test]
fn hard_limit_rejects_regardless_of_warning_interval() {
    let mut pool = test_pool_with_quota();
    pool.resource_quota = Some(ResourceQuota::new(5, 10, 5000).unwrap());
    add_connections(&mut pool, 10);

    // Even if we just warned
    pool.last_quota_warning_at = Some(TimestampMs::now());
    assert_eq!(pool.check_quota().unwrap_err(), QuotaError::HardLimitExceeded);
}
