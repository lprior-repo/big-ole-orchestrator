//! Health check implementation for pooled connections.

use vo_common::connection_pool::HealthCheckResult;
use vo_common::types::TimestampMs;

pub trait TimestampLike {
    fn timestamp_ms(self) -> u64;
}

impl TimestampLike for TimestampMs {
    fn timestamp_ms(self) -> u64 {
        self.as_u64()
    }
}

impl TimestampLike for vo_types::TimestampMs {
    fn timestamp_ms(self) -> u64 {
        self.as_u64()
    }
}

#[derive(Debug, Clone)]
pub struct HealthCheck {
    timeout_ms: u64,
}

impl HealthCheck {
    pub fn new(timeout_ms: u64) -> Self {
        Self { timeout_ms }
    }

    pub fn check_connection<T>(
        &self,
        last_used_at: T,
        idle_timeout_ms: u64,
        current_time: T,
    ) -> HealthCheckResult
    where
        T: TimestampLike,
    {
        let elapsed = current_time
            .timestamp_ms()
            .saturating_sub(last_used_at.timestamp_ms());

        if elapsed > idle_timeout_ms {
            return HealthCheckResult::Stale;
        }

        HealthCheckResult::Healthy
    }

    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }
}

pub struct HealthCheckFuture {
    connection_id: vo_common::connection_pool::ConnectionId,
    started_at: TimestampMs,
    timeout_ms: u64,
}

impl HealthCheckFuture {
    pub fn new(connection_id: vo_common::connection_pool::ConnectionId, timeout_ms: u64) -> Self {
        Self {
            connection_id,
            started_at: TimestampMs::now(),
            timeout_ms,
        }
    }

    pub fn is_timed_out(&self) -> bool {
        let elapsed = TimestampMs::now()
            .as_u64()
            .saturating_sub(self.started_at.as_u64());
        elapsed > self.timeout_ms
    }

    pub fn connection_id(&self) -> vo_common::connection_pool::ConnectionId {
        self.connection_id
    }
}

pub fn determine_health_check_result(
    is_connected: bool,
    is_timed_out: bool,
    is_corrupted: bool,
) -> HealthCheckResult {
    if is_timed_out {
        return HealthCheckResult::Timeout;
    }
    if is_corrupted {
        return HealthCheckResult::Corrupted;
    }
    if !is_connected {
        return HealthCheckResult::Stale;
    }
    HealthCheckResult::Healthy
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
