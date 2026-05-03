use std::sync::Mutex;
use std::time::Duration;

use vo_common::connection_pool::{
    AcquireResult, ConnectionId, ConnectionPoolError, ConnectionStatus, ErrorCategory,
    ErrorContext, ErrorDetail, EvictionReason, PoolConfig as VoPoolConfig, PoolId, PoolStats,
    PooledConnection, ReleaseResult,
};
use vo_types::TimestampMs;

use crate::connector::{Connector, ConnectorError, ConnectorRegistry};
use crate::pool::{config::PoolConfig, ConnectionPool};

pub struct ManagedPool {
    pool_id: PoolId,
    nats_urls: Vec<String>,
    pool: Mutex<ConnectionPool>,
    connectors: ConnectorRegistry,
}

impl ManagedPool {
    pub fn new(pool_id: PoolId, config: VoPoolConfig) -> Self {
        let worker_config = PoolConfig::new(
            config.min_connections,
            config.max_connections,
            config.connection_timeout_ms,
            config.idle_timeout_ms,
            config.health_check_interval_ms,
            config.max_pending_acquires,
        )
        .expect("pool config validation failed in ManagedPool::new");

        let pool = ConnectionPool::new(
            pool_id.clone(),
            vec!["nats://localhost:4222".to_string()],
            worker_config,
        );

        Self {
            pool_id,
            nats_urls: vec!["nats://localhost:4222".to_string()],
            pool: Mutex::new(pool),
            connectors: ConnectorRegistry::new(),
        }
    }

    pub async fn acquire(&self) -> Result<AcquireResult, ConnectionPoolError> {
        let timeout_ms = self.pool.lock().unwrap().state.config.connection_timeout_ms;
        let mut guard = self
            .pool
            .lock()
            .map_err(|_| ConnectionPoolError {
                category: ErrorCategory::InvalidState,
                detail: ErrorDetail::InvalidRelease {
                    reason: "pool mutex poisoned",
                },
                context: ErrorContext {
                    pool_id: self.pool_id.clone(),
                    timestamp: TimestampMs::now().into(),
                    operation: "acquire",
                    connection_id: None,
                },
            })?;

        match guard.acquire_with_timeout(Duration::from_millis(timeout_ms)).await
        {
            AcquireResult::Available { connection } => Ok(AcquireResult::Available { connection }),
            AcquireResult::Pending { wait_handle } => Ok(AcquireResult::Pending { wait_handle }),
            AcquireResult::PoolExhausted { config } => Ok(AcquireResult::PoolExhausted { config }),
            AcquireResult::PoolClosing => {
                let error = ConnectionPoolError {
                    category: ErrorCategory::ShutdownInProgress,
                    detail: ErrorDetail::AlreadyShutdown,
                    context: ErrorContext {
                        pool_id: self.pool_id.clone(),
                        timestamp: TimestampMs::now().into(),
                        operation: "acquire",
                        connection_id: None,
                    },
                };
                Err(error)
            }
            AcquireResult::Timeout { waited_ms } => {
                let error = ConnectionPoolError {
                    category: ErrorCategory::Timeout,
                    detail: ErrorDetail::AcquireTimeout {
                        waited_ms,
                        timeout_ms: guard.state.config.connection_timeout_ms,
                    },
                    context: ErrorContext {
                        pool_id: self.pool_id.clone(),
                        timestamp: TimestampMs::now().into(),
                        operation: "acquire",
                        connection_id: None,
                    },
                };
                Err(error)
            }
        }
    }

    pub async fn release(
        &self,
        connection_id: ConnectionId,
    ) -> Result<ReleaseResult, ConnectionPoolError> {
        let mut guard = self
            .pool
            .lock()
            .map_err(|_| ConnectionPoolError {
                category: ErrorCategory::InvalidState,
                detail: ErrorDetail::InvalidRelease {
                    reason: "pool mutex poisoned",
                },
                context: ErrorContext {
                    pool_id: self.pool_id.clone(),
                    timestamp: TimestampMs::now().into(),
                    operation: "release",
                    connection_id: None,
                },
            })?;

        match guard.release(connection_id) {
            ReleaseResult::Returned => Ok(ReleaseResult::Returned),
            ReleaseResult::Evicted { reason } => Ok(ReleaseResult::Evicted { reason }),
            ReleaseResult::AlreadyClosed => {
                let error = ConnectionPoolError {
                    category: ErrorCategory::InvalidState,
                    detail: ErrorDetail::InvalidRelease {
                        reason: "connection not acquired or already released",
                    },
                    context: ErrorContext {
                        pool_id: self.pool_id.clone(),
                        timestamp: TimestampMs::now().into(),
                        operation: "release",
                        connection_id: Some(connection_id),
                    },
                };
                Err(error)
            }
        }
    }

    pub fn stats(&self) -> PoolStats {
        self.pool.lock().ok().map(|g| g.stats()).unwrap_or_default()
    }

    pub fn register_connector(&mut self, name: String, connector: Box<dyn Connector>) {
        self.connectors.register(name, connector);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connector::{CommitOutcome, PreparedEffect, ReconcileOutcome};

    #[test]
    fn pool_config_type_accessible_from_vo_worker() {
        let config = VoPoolConfig {
            min_connections: 1,
            max_connections: 10,
            connection_timeout_ms: 5000,
            idle_timeout_ms: 30000,
            health_check_interval_ms: 10000,
            max_pending_acquires: 5,
        };
        let _ = &config;
    }

    #[test]
    fn pool_id_type_accessible_from_vo_worker() {
        let id = PoolId::new("test-pool");
        assert_eq!(id.as_str(), "test-pool");
    }

    #[test]
    fn connection_id_type_accessible_from_vo_worker() {
        let id = ConnectionId::new();
        let _ = &id;
    }

    #[test]
    fn pooled_connection_type_accessible_from_vo_worker() {
        let conn = PooledConnection::new(
            ConnectionId::new(),
            TimestampMs(1000),
        );
        assert!(conn.is_idle());
    }

    #[test]
    fn managed_pool_new_creates_instance() {
        let config = VoPoolConfig {
            min_connections: 1,
            max_connections: 5,
            connection_timeout_ms: 3000,
            idle_timeout_ms: 10000,
            health_check_interval_ms: 5000,
            max_pending_acquires: 3,
        };
        let pool = ManagedPool::new(PoolId::new("test"), config);
        let _ = &pool;
        assert_eq!(pool.pool_id().as_str(), "test");
    }

    #[tokio::test]
    async fn managed_pool_acquire_returns_available() {
        let config = VoPoolConfig {
            min_connections: 1,
            max_connections: 5,
            connection_timeout_ms: 3000,
            idle_timeout_ms: 10000,
            health_check_interval_ms: 5000,
            max_pending_acquires: 3,
        };
        let pool = ManagedPool::new(PoolId::new("test"), config);
        let result = pool.acquire().await;
        assert!(matches!(result, Ok(AcquireResult::Available { .. })));
    }

    #[tokio::test]
    async fn managed_pool_release_returns_result() {
        let config = VoPoolConfig {
            min_connections: 1,
            max_connections: 5,
            connection_timeout_ms: 3000,
            idle_timeout_ms: 10000,
            health_check_interval_ms: 5000,
            max_pending_acquires: 3,
        };
        let pool = ManagedPool::new(PoolId::new("test"), config);
        let acquire = pool.acquire().await.unwrap();
        let conn_id = match acquire {
            AcquireResult::Available { connection } => connection.connection_id,
            _ => panic!("expected Available"),
        };
        let result = pool.release(conn_id).await;
        assert!(matches!(result, Ok(ReleaseResult::Returned)));
    }

    #[tokio::test]
    async fn managed_pool_release_unknown_connection_errors() {
        let config = VoPoolConfig {
            min_connections: 1,
            max_connections: 5,
            connection_timeout_ms: 3000,
            idle_timeout_ms: 10000,
            health_check_interval_ms: 5000,
            max_pending_acquires: 3,
        };
        let pool = ManagedPool::new(PoolId::new("test"), config);
        let unknown_id = ConnectionId::new();
        let result = pool.release(unknown_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn managed_pool_release_twice_errors() {
        let config = VoPoolConfig {
            min_connections: 1,
            max_connections: 5,
            connection_timeout_ms: 3000,
            idle_timeout_ms: 10000,
            health_check_interval_ms: 5000,
            max_pending_acquires: 3,
        };
        let pool = ManagedPool::new(PoolId::new("test"), config);
        let acquire = pool.acquire().await.unwrap();
        let conn_id = match acquire {
            AcquireResult::Available { connection } => connection.connection_id,
            _ => panic!("expected Available"),
        };
        pool.release(conn_id).await.unwrap();
        let result = pool.release(conn_id).await;
        assert!(result.is_err());
    }

    #[test]
    fn managed_pool_stats_returns_stats() {
        let config = VoPoolConfig {
            min_connections: 0,
            max_connections: 5,
            connection_timeout_ms: 3000,
            idle_timeout_ms: 10000,
            health_check_interval_ms: 5000,
            max_pending_acquires: 3,
        };
        let pool = ManagedPool::new(PoolId::new("test"), config);
        let stats = pool.stats();
        assert_eq!(stats.pool_id.as_str(), "test");
    }

    #[test]
    fn managed_pool_stats_reflects_acquisitions() {
        let config = VoPoolConfig {
            min_connections: 1,
            max_connections: 5,
            connection_timeout_ms: 3000,
            idle_timeout_ms: 10000,
            health_check_interval_ms: 5000,
            max_pending_acquires: 3,
        };
        let pool = ManagedPool::new(PoolId::new("stats-test"), config);
        let stats_before = pool.stats();
        assert_eq!(stats_before.total_connections, 0);

        let acquire = futures::executor::block_on(pool.acquire()).unwrap();
        match acquire {
            AcquireResult::Available { .. } => {}
            _ => panic!("expected Available"),
        }

        let stats_after = pool.stats();
        assert_eq!(stats_after.total_connections, 1);
    }

    #[tokio::test]
    async fn managed_pool_register_connector() {
        let config = VoPoolConfig {
            min_connections: 1,
            max_connections: 5,
            connection_timeout_ms: 3000,
            idle_timeout_ms: 10000,
            health_check_interval_ms: 5000,
            max_pending_acquires: 3,
        };
        let mut pool = ManagedPool::new(PoolId::new("test"), config);

        pool.register_connector(
            "mock".to_string(),
            Box::new(MockConnector {
                name: "mock".to_string(),
            }),
        );

        let stats = pool.stats();
        assert_eq!(stats.pool_id.as_str(), "test");
    }

    #[test]
    fn pool_stats_type_accessible() {
        let stats = PoolStats::default();
        assert_eq!(stats.total_connections, 0);
    }

    #[derive(Clone)]
    struct MockConnector {
        name: String,
    }

    #[async_trait::async_trait]
    impl Connector for MockConnector {
        fn connector_type(&self) -> &str {
            &self.name
        }
        fn connector_version(&self) -> &str {
            "1.0.0"
        }
        fn supports_compensation(&self) -> bool {
            false
        }

        async fn prepare(
            &self,
            _effect_intent: serde_json::Value,
            _effect_id: String,
            _fence: u64,
        ) -> Result<PreparedEffect, ConnectorError> {
            Ok(PreparedEffect {
                effect_id: String::new(),
                payload: serde_json::value::Value::Null,
                fence: 0,
            })
        }

        async fn commit(
            &self,
            _prepared: PreparedEffect,
        ) -> Result<CommitOutcome, ConnectorError> {
            Ok(CommitOutcome::Committed {
                receipt: "mock".into(),
            })
        }

        async fn reconcile(
            &self,
            _effect_id: &str,
        ) -> Result<ReconcileOutcome, ConnectorError> {
            Ok(ReconcileOutcome::NotCommitted)
        }
    }
}
