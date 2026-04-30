//! Heartbeat watcher configuration types.
//!
//! Per ADR-012: The heartbeat watcher monitors actor health and handles
//! graceful shutdown when actors become unresponsive.

use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;

use crate::probe::BackoffConfig;

/// Configuration for the heartbeat watcher.
#[derive(Debug, Clone)]
pub struct HeartbeatWatcherConfig {
    /// How often to check all registered probes.
    pub check_interval: Duration,
    /// Number of consecutive failures before triggering shutdown.
    pub failure_threshold: u32,
    /// Backoff configuration for retry intervals.
    pub backoff: BackoffConfig,
    /// Timeout for graceful shutdown.
    pub graceful_shutdown_timeout: Duration,
}

impl Default for HeartbeatWatcherConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(30),
            failure_threshold: 3,
            backoff: BackoffConfig::default(),
            graceful_shutdown_timeout: Duration::from_secs(30),
        }
    }
}

/// Callback for when an actor should be shut down due to health failure.
pub type ShutdownCallback = Box<dyn Fn(InstanceIdOwned) -> Result<(), ShutdownError> + Send + Sync>;

/// Instance ID type alias for heartbeat monitoring.
pub type InstanceIdOwned = String;

#[derive(Debug, Error)]
pub enum ShutdownError {
    #[error("actor not found: {0}")]
    NotFound(String),
    #[error("shutdown failed: {0}")]
    Failed(String),
}
