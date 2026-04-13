//! Health check implementation for pooled connections.

use vo_types::connection_pool::HealthCheckResult;
use vo_types::integer_types::TimestampMs;

#[derive(Debug, Clone)]
pub struct HealthCheck {
    timeout_ms: u64,
}

impl HealthCheck {
    pub fn new(timeout_ms: u64) -> Self {
        Self { timeout_ms }
    }

    pub fn check_connection(
        &self,
        last_used_at: TimestampMs,
        idle_timeout_ms: u64,
        current_time: TimestampMs,
    ) -> HealthCheckResult {
        let elapsed = current_time.as_u64().saturating_sub(last_used_at.as_u64());

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
    connection_id: vo_types::connection_pool::ConnectionId,
    started_at: TimestampMs,
    timeout_ms: u64,
}

impl HealthCheckFuture {
    pub fn new(connection_id: vo_types::connection_pool::ConnectionId, timeout_ms: u64) -> Self {
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

    pub fn connection_id(&self) -> vo_types::connection_pool::ConnectionId {
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
        let now = TimestampMs::new_unchecked(35000);
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
        let conn_id = vo_types::connection_pool::ConnectionId::new();
        let future = HealthCheckFuture::new(conn_id, 1);
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(future.is_timed_out());
    }

    #[test]
    fn test_health_check_future_not_timed_out_yet() {
        let conn_id = vo_types::connection_pool::ConnectionId::new();
        let future = HealthCheckFuture::new(conn_id, 10000);
        assert!(!future.is_timed_out());
    }

    #[test]
    fn test_health_check_future_connection_id() {
        let conn_id = vo_types::connection_pool::ConnectionId::new();
        let future = HealthCheckFuture::new(conn_id, 5000);
        assert_eq!(future.connection_id(), conn_id);
    }
}
