//! Health check types for connection pool.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthCheckResult {
    Healthy,
    Stale,
    Corrupted,
    Timeout,
}

impl fmt::Display for HealthCheckResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HealthCheckResult::Healthy => write!(f, "Healthy"),
            HealthCheckResult::Stale => write!(f, "Stale"),
            HealthCheckResult::Corrupted => write!(f, "Corrupted"),
            HealthCheckResult::Timeout => write!(f, "Timeout"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_check_result_all_values() {
        let results = [
            HealthCheckResult::Healthy,
            HealthCheckResult::Stale,
            HealthCheckResult::Corrupted,
            HealthCheckResult::Timeout,
        ];
        assert_eq!(results.len(), 4);
    }

    #[test]
    fn test_health_check_result_equality() {
        assert_eq!(HealthCheckResult::Healthy, HealthCheckResult::Healthy);
        assert_ne!(HealthCheckResult::Healthy, HealthCheckResult::Stale);
        assert_ne!(HealthCheckResult::Stale, HealthCheckResult::Corrupted);
    }

    #[test]
    fn test_health_check_result_display() {
        assert_eq!(format!("{}", HealthCheckResult::Healthy), "Healthy");
        assert_eq!(format!("{}", HealthCheckResult::Stale), "Stale");
    }
}