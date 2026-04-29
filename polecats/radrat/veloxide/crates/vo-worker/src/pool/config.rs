//! Pool configuration validation and construction.

use vo_types::connection_pool::PoolConfig as VoPoolConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolConfig {
    pub min_connections: u32,
    pub max_connections: u32,
    pub connection_timeout_ms: u64,
    pub idle_timeout_ms: u64,
    pub health_check_interval_ms: u64,
    pub max_pending_acquires: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolConfigError {
    MinGreaterThanMax,
    MaxZero,
    ConnectionTimeoutZero,
    IdleTimeoutZero,
    HealthCheckIntervalZero,
}

impl PoolConfig {
    pub fn new(
        min_connections: u32,
        max_connections: u32,
        connection_timeout_ms: u64,
        idle_timeout_ms: u64,
        health_check_interval_ms: u64,
        max_pending_acquires: u32,
    ) -> Result<Self, PoolConfigError> {
        if min_connections > max_connections {
            return Err(PoolConfigError::MinGreaterThanMax);
        }
        if max_connections == 0 {
            return Err(PoolConfigError::MaxZero);
        }
        if connection_timeout_ms == 0 {
            return Err(PoolConfigError::ConnectionTimeoutZero);
        }
        if idle_timeout_ms == 0 {
            return Err(PoolConfigError::IdleTimeoutZero);
        }
        if health_check_interval_ms == 0 {
            return Err(PoolConfigError::HealthCheckIntervalZero);
        }

        Ok(Self {
            min_connections,
            max_connections,
            connection_timeout_ms,
            idle_timeout_ms,
            health_check_interval_ms,
            max_pending_acquires,
        })
    }

    pub fn with_defaults() -> Self {
        Self {
            min_connections: 2,
            max_connections: 10,
            connection_timeout_ms: 5000,
            idle_timeout_ms: 30000,
            health_check_interval_ms: 10000,
            max_pending_acquires: 5,
        }
    }
}

impl From<PoolConfig> for VoPoolConfig {
    fn from(config: PoolConfig) -> Self {
        VoPoolConfig {
            min_connections: config.min_connections,
            max_connections: config.max_connections,
            connection_timeout_ms: config.connection_timeout_ms,
            idle_timeout_ms: config.idle_timeout_ms,
            health_check_interval_ms: config.health_check_interval_ms,
            max_pending_acquires: config.max_pending_acquires,
        }
    }
}

impl From<VoPoolConfig> for PoolConfig {
    fn from(config: VoPoolConfig) -> Self {
        Self {
            min_connections: config.min_connections,
            max_connections: config.max_connections,
            connection_timeout_ms: config.connection_timeout_ms,
            idle_timeout_ms: config.idle_timeout_ms,
            health_check_interval_ms: config.health_check_interval_ms,
            max_pending_acquires: config.max_pending_acquires,
        }
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;

    #[test]
    fn test_valid_config() {
        let config = PoolConfig::new(2, 10, 5000, 30000, 10000, 5);
        assert!(config.is_ok());
        let config = config.unwrap();
        assert_eq!(config.min_connections, 2);
        assert_eq!(config.max_connections, 10);
    }

    #[test]
    fn test_min_equals_max() {
        let config = PoolConfig::new(5, 5, 5000, 30000, 10000, 5);
        assert!(config.is_ok());
    }

    #[test]
    fn test_min_greater_than_max() {
        let config = PoolConfig::new(10, 5, 5000, 30000, 10000, 5);
        assert_eq!(config.unwrap_err(), PoolConfigError::MinGreaterThanMax);
    }

    #[test]
    fn test_max_zero() {
        let config = PoolConfig::new(0, 0, 5000, 30000, 10000, 5);
        assert_eq!(config.unwrap_err(), PoolConfigError::MaxZero);
    }

    #[test]
    fn test_connection_timeout_zero() {
        let config = PoolConfig::new(1, 10, 0, 30000, 10000, 5);
        assert_eq!(config.unwrap_err(), PoolConfigError::ConnectionTimeoutZero);
    }

    #[test]
    fn test_idle_timeout_zero() {
        let config = PoolConfig::new(1, 10, 5000, 0, 10000, 5);
        assert_eq!(config.unwrap_err(), PoolConfigError::IdleTimeoutZero);
    }

    #[test]
    fn test_health_check_interval_zero() {
        let config = PoolConfig::new(1, 10, 5000, 30000, 0, 5);
        assert_eq!(
            config.unwrap_err(),
            PoolConfigError::HealthCheckIntervalZero
        );
    }

    #[test]
    fn test_max_pending_acquires_zero_is_ok() {
        let config = PoolConfig::new(1, 10, 5000, 30000, 10000, 0);
        assert!(config.is_ok());
    }

    #[test]
    fn test_with_defaults() {
        let config = PoolConfig::with_defaults();
        assert_eq!(config.min_connections, 2);
        assert_eq!(config.max_connections, 10);
        assert_eq!(config.connection_timeout_ms, 5000);
    }

    #[test]
    fn test_convert_to_vo_pool_config() {
        let config = PoolConfig::new(2, 10, 5000, 30000, 10000, 5).unwrap();
        let vo_config: VoPoolConfig = config.into();
        assert_eq!(vo_config.min_connections, 2);
        assert_eq!(vo_config.max_connections, 10);
    }

    #[test]
    fn test_convert_from_vo_pool_config() {
        let vo_config = VoPoolConfig {
            min_connections: 3,
            max_connections: 15,
            connection_timeout_ms: 3000,
            idle_timeout_ms: 60000,
            health_check_interval_ms: 5000,
            max_pending_acquires: 10,
        };
        let config = PoolConfig::from(vo_config);
        assert_eq!(config.min_connections, 3);
        assert_eq!(config.max_connections, 15);
    }
}
