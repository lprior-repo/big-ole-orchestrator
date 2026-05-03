//! Health check implementation for pooled connections.

use vo_common::connection_pool::HealthCheckResult;
use vo_types::TimestampMs;

/// Determines the health check result based on connection state flags.
pub fn determine_health_check_result(
    is_connected: bool,
    is_timeout: bool,
    is_corrupted: bool,
) -> HealthCheckResult {
    if is_timeout {
        HealthCheckResult::Timeout
    } else if is_corrupted {
        HealthCheckResult::Corrupted
    } else if is_connected {
        HealthCheckResult::Healthy
    } else {
        HealthCheckResult::Stale
    }
}

/// Health check configuration and logic for pooled connections.
#[derive(Debug, Clone)]
pub struct HealthCheck {
    timeout_ms: u64,
}

impl HealthCheck {
    pub fn new(timeout_ms: u64) -> Self {
        Self { timeout_ms }
    }

    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    pub fn check_connection(
        &self,
        _last_used: TimestampMs,
        _idle_timeout_ms: u64,
        _now: TimestampMs,
    ) -> HealthCheckResult {
        HealthCheckResult::Healthy
    }
}

/// A future representing a pending health check operation.
pub struct HealthCheckFuture {
    _connection_id: vo_common::connection_pool::ConnectionId,
    _timeout_ms: u64,
    created_at: std::time::Instant,
}

impl HealthCheckFuture {
    pub fn new(
        connection_id: vo_common::connection_pool::ConnectionId,
        timeout_ms: u64,
    ) -> Self {
        Self {
            _connection_id: connection_id,
            _timeout_ms: timeout_ms,
            created_at: std::time::Instant::now(),
        }
    }

    pub fn is_timed_out(&self) -> bool {
        self.created_at.elapsed().as_millis() as u64 >= self._timeout_ms
    }

    pub fn connection_id(&self) -> vo_common::connection_pool::ConnectionId {
        self._connection_id
    }
}

pub trait TimestampLike {
    fn timestamp_ms(self) -> u64;
}

impl TimestampLike for TimestampMs {
    fn timestamp_ms(self) -> u64 {
        self.as_u64()
    }
}

#[cfg(test)]
mod health_check_tests {
    use super::*;

    #[test]
    fn test_healthy_connection() {
        let hc = HealthCheck::new(5000);
        let created = TimestampMs::new_unchecked(1000);
        let now = TimestampMs::new_unchecked(5000);
        let result = hc.check_connection(created, 30000, now);
        assert_eq!(result, HealthCheckResult::Healthy);
    }

    #[test]
    fn test_stale_connection_after_idle_timeout() {
        let hc = HealthCheck::new(5000);
        let last_used = TimestampMs::new_unchecked(1000);
        let now = TimestampMs::new_unchecked(40000);
        let idle_timeout_ms = 30000;
        let result = hc.check_connection(last_used, idle_timeout_ms, now);
        assert_eq!(result, HealthCheckResult::Stale);
    }

    #[test]
    fn test_healthy_just_under_idle_timeout() {
        let hc = HealthCheck::new(5000);
        let last_used = TimestampMs::new_unchecked(1000);
        let now = TimestampMs::new_unchecked(30999);
        let idle_timeout_ms = 30000;
        let result = hc.check_connection(last_used, idle_timeout_ms, now);
        assert_eq!(result, HealthCheckResult::Healthy);
    }

    #[test]
    fn test_determine_health_check_result_healthy() {
        let result = determine_health_check_result(true, false, false);
        assert_eq!(result, HealthCheckResult::Healthy);
    }

    #[test]
    fn test_determine_health_check_result_timeout() {
        let result = determine_health_check_result(true, true, false);
        assert_eq!(result, HealthCheckResult::Timeout);
    }

    #[test]
    fn test_determine_health_check_result_corrupted() {
        let result = determine_health_check_result(true, false, true);
        assert_eq!(result, HealthCheckResult::Corrupted);
    }

    #[test]
    fn test_determine_health_check_result_stale() {
        let result = determine_health_check_result(false, false, false);
        assert_eq!(result, HealthCheckResult::Stale);
    }

    #[test]
    fn test_health_check_future_timeout() {
        let conn_id = vo_common::connection_pool::ConnectionId::new();
        let future = HealthCheckFuture::new(conn_id, 1);
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(future.is_timed_out());
    }

    #[test]
    fn test_health_check_future_not_timed_out_yet() {
        let conn_id = vo_common::connection_pool::ConnectionId::new();
        let future = HealthCheckFuture::new(conn_id, 10000);
        assert!(!future.is_timed_out());
    }

    #[test]
    fn test_health_check_future_connection_id() {
        let conn_id = vo_common::connection_pool::ConnectionId::new();
        let future = HealthCheckFuture::new(conn_id, 5000);
        assert_eq!(future.connection_id(), conn_id);
    }

    #[test]
    fn test_health_check_new_connection_zero_elapsed() {
        let hc = HealthCheck::new(5000);
        let last_used = TimestampMs::new_unchecked(5000);
        let now = TimestampMs::new_unchecked(5000);
        let result = hc.check_connection(last_used, 30000, now);
        assert_eq!(result, HealthCheckResult::Healthy);
    }

    #[test]
    fn test_health_check_zero_idle_timeout_immediately_stale() {
        let hc = HealthCheck::new(5000);
        let last_used = TimestampMs::new_unchecked(1000);
        let now = TimestampMs::new_unchecked(1001);
        let result = hc.check_connection(last_used, 0, now);
        assert_eq!(result, HealthCheckResult::Stale);
    }

    #[test]
    fn test_health_check_max_idle_timeout_never_stale() {
        let hc = HealthCheck::new(5000);
        let last_used = TimestampMs::new_unchecked(1000);
        let now = TimestampMs::new_unchecked(u64::MAX);
        let result = hc.check_connection(last_used, u64::MAX, now);
        assert_eq!(result, HealthCheckResult::Healthy);
    }

    #[test]
    fn test_health_check_current_time_before_last_used_saturating() {
        let hc = HealthCheck::new(5000);
        let last_used = TimestampMs::new_unchecked(5000);
        let now = TimestampMs::new_unchecked(1000);
        let result = hc.check_connection(last_used, 30000, now);
        assert_eq!(result, HealthCheckResult::Healthy);
    }

    #[test]
    fn test_health_check_zero_timeout_ms() {
        let hc = HealthCheck::new(0);
        assert_eq!(hc.timeout_ms(), 0);
    }

    #[test]
    fn test_health_check_max_timeout_ms() {
        let hc = HealthCheck::new(u64::MAX);
        assert_eq!(hc.timeout_ms(), u64::MAX);
    }

    #[test]
    fn test_determine_health_check_result_priority_timeout_first() {
        assert_eq!(
            determine_health_check_result(true, true, true),
            HealthCheckResult::Timeout
        );
        assert_eq!(
            determine_health_check_result(true, true, false),
            HealthCheckResult::Timeout
        );
        assert_eq!(
            determine_health_check_result(false, true, true),
            HealthCheckResult::Timeout
        );
    }

    #[test]
    fn test_determine_health_check_result_priority_corrupted_second() {
        assert_eq!(
            determine_health_check_result(true, false, true),
            HealthCheckResult::Corrupted
        );
        assert_eq!(
            determine_health_check_result(false, false, true),
            HealthCheckResult::Corrupted
        );
    }

    #[test]
    fn test_determine_health_check_result_priority_stale_third() {
        assert_eq!(
            determine_health_check_result(false, false, false),
            HealthCheckResult::Stale
        );
    }

    #[test]
    fn test_health_check_future_zero_timeout_triggers_on_elapsed() {
        let conn_id = vo_common::connection_pool::ConnectionId::new();
        let future = HealthCheckFuture::new(conn_id, 0);
        std::thread::sleep(std::time::Duration::from_millis(1));
        assert!(future.is_timed_out());
    }

    #[test]
    fn test_health_check_future_max_timeout_never() {
        let conn_id = vo_common::connection_pool::ConnectionId::new();
        let future = HealthCheckFuture::new(conn_id, u64::MAX);
        assert!(!future.is_timed_out());
    }
}
