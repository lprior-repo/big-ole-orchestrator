use std::sync::Arc;
use tokio::sync::Mutex;

use vo_common::connection_pool::{
    AcquireResult, ConnectionId, ConnectionPoolError, PoolConfig as VoPoolConfig, PoolId, PoolStats,
    PooledConnection, ReleaseResult,
};

use super::config::PoolConfig;
use super::ConnectionPool;
use crate::connector::{Connector, ConnectorRegistry};

pub struct ManagedPool {
    pool_id: PoolId,
    pool: Arc<Mutex<ConnectionPool>>,
    connector_registry: ConnectorRegistry,
}

impl ManagedPool {
    pub fn new(pool_id: PoolId, nats_urls: Vec<String>, config: PoolConfig) -> Self {
        let vo_config: VoPoolConfig = config.clone().into();
        let pool = ConnectionPool::new(pool_id.clone(), nats_urls, config);
        Self {
            pool_id,
            pool: Arc::new(Mutex::new(pool)),
            connector_registry: ConnectorRegistry::new(),
        }
    }

    pub async fn acquire(&self) -> Result<AcquireResult, ConnectionPoolError> {
        let mut pool = self.pool.lock().await;
        Ok(pool.acquire().await)
    }

    pub async fn release(
        &self,
        connection_id: ConnectionId,
    ) -> Result<ReleaseResult, ConnectionPoolError> {
        let mut pool = self.pool.lock().await;
        Ok(pool.release(connection_id))
    }

    pub fn stats(&self) -> PoolStats {
        self.pool.blocking_lock().stats()
    }

    pub fn register_connector(&mut self, name: String, connector: Box<dyn Connector>) {
        self.connector_registry.register(name, connector);
    }

    pub fn get_connector(&self, name: &str) -> Option<Arc<dyn Connector>> {
        self.connector_registry.get(name)
    }

    pub fn pool_id(&self) -> &PoolId {
        &self.pool_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_config_type_accessible_from_vo_worker() {
        let config = PoolConfig::new(1, 10, 5000, 30000, 10000, 5).unwrap();
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
            vo_types::TimestampMs::try_from(1000u64).unwrap(),
        );
        assert!(conn.is_idle());
    }

    #[test]
    fn managed_pool_new_creates_instance() {
        let config = PoolConfig::new(1, 5, 3000, 10000, 5000, 3).unwrap();
        let pool = ManagedPool::new(PoolId::new("test"), vec!["nats://localhost:4222".to_string()], config);
        let _ = &pool;
        assert_eq!(pool.pool_id().as_str(), "test");
    }

    #[tokio::test]
    async fn managed_pool_acquire_returns_result() {
        let config = PoolConfig::new(1, 5, 3000, 10000, 5000, 3).unwrap();
        let pool = ManagedPool::new(PoolId::new("test"), vec!["nats://localhost:4222".to_string()], config);
        let result = pool.acquire().await;
        assert!(matches!(result, AcquireResult::Available { .. }));
    }

    #[test]
    fn managed_pool_stats_returns_stats() {
        let config = PoolConfig::new(0, 5, 3000, 10000, 5000, 3).unwrap();
        let pool = ManagedPool::new(PoolId::new("test"), vec!["nats://localhost:4222".to_string()], config);
        let stats = pool.stats();
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.pool_id.as_str(), "test");
    }

    #[test]
    fn pool_stats_type_accessible() {
        let stats = PoolStats::default();
        assert_eq!(stats.total_connections, 0);
    }
}
