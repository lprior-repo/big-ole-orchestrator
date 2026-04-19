//! Heartbeat watcher for actor health monitoring.
//!
//! Per ADR-012: The heartbeat watcher monitors actor health and handles
//! graceful shutdown when actors become unresponsive.
//!
//! # Architecture
//!
//! The watcher periodically checks actor health via configured probes.
//! When consecutive failures exceed the threshold, it triggers graceful
//! shutdown via the lifecycle module.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tokio::time::{interval, Instant};
use tracing::{error, info, warn};

use crate::probe::{
    BackoffConfig, Probe, ProbeConfig, ProbeDefinition, ProbeError, ProbeId, ProbeRegistry,
    ProbeResult, ProbeStatus,
};

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

use thiserror::Error;

type InstanceIdOwned = String;

#[derive(Debug, Error)]
pub enum ShutdownError {
    #[error("actor not found: {0}")]
    NotFound(String),
    #[error("shutdown failed: {0}")]
    Failed(String),
}

/// Actor tracking state for heartbeat monitoring.
#[derive(Debug, Clone)]
struct ActorHealthState {
    consecutive_failures: u32,
    last_check: Option<Instant>,
    last_healthy: Option<Instant>,
}

impl ActorHealthState {
    fn new() -> Self {
        Self {
            consecutive_failures: 0,
            last_check: None,
            last_healthy: None,
        }
    }

    fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        self.last_check = Some(Instant::now());
    }

    fn record_healthy(&mut self) {
        self.consecutive_failures = 0;
        self.last_check = Some(Instant::now());
        self.last_healthy = Some(Instant::now());
    }
}

impl Default for ActorHealthState {
    fn default() -> Self {
        Self::new()
    }
}

/// Heartbeat watcher that monitors actor health via probes.
///
/// The watcher:
/// 1. Periodically checks all registered probes
/// 2. Tracks consecutive failures per actor
/// 3. Triggers graceful shutdown when threshold is exceeded
/// 4. Emits structured tracing events
pub struct HeartbeatWatcher {
    config: HeartbeatWatcherConfig,
    probe_registry: Arc<RwLock<ProbeRegistry>>,
    actor_states: Arc<RwLock<HashMap<String, ActorHealthState>>>,
    shutdown_callback: Option<ShutdownCallback>,
}

impl HeartbeatWatcher {
    /// Creates a new heartbeat watcher with the given configuration.
    #[must_use]
    pub fn new(config: HeartbeatWatcherConfig) -> Self {
        Self {
            config,
            probe_registry: Arc::new(RwLock::new(ProbeRegistry::new())),
            actor_states: Arc::new(RwLock::new(HashMap::new())),
            shutdown_callback: None,
        }
    }

    /// Creates a heartbeat watcher with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(HeartbeatWatcherConfig::default())
    }

    /// Sets the shutdown callback invoked when an actor exceeds failure threshold.
    pub fn with_shutdown_callback(mut self, callback: ShutdownCallback) -> Self {
        self.shutdown_callback = Some(callback);
        self
    }

    /// Registers a probe to monitor an actor.
    pub async fn register_probe(&self, actor_id: String, probe: Box<dyn Probe>) -> ProbeId {
        let definition = ProbeDefinition {
            id: probe.probe_id(),
            name: actor_id.clone(),
            config: ProbeConfig::Http {
                url: format!("http://{actor_id}/health"),
                expected_status: Some(200),
                timeout_ms: 5000,
            },
            interval: self.config.check_interval,
            backoff: self.config.backoff,
            failure_threshold: self.config.failure_threshold,
            success_threshold: 1,
        };

        let mut registry = self.probe_registry.write().await;

        registry.register(definition)
    }

    /// Registers an actor with a probe configuration without creating the probe.
    /// This allows manual probe management.
    pub async fn register_actor(&self, actor_id: String) {
        let mut states = self.actor_states.write().await;
        states.entry(actor_id).or_insert_with(ActorHealthState::new);
    }

    /// Unregisters an actor from heartbeat monitoring.
    pub async fn unregister_actor(&self, actor_id: &str) {
        let mut states = self.actor_states.write().await;
        states.remove(actor_id);
    }

    /// Runs the heartbeat watcher loop.
    ///
    /// This is an infinite loop that periodically checks all probes and
    /// triggers shutdown for actors that exceed the failure threshold.
    ///
    /// # Errors
    /// Returns an error if the probe registry access fails.
    pub async fn run(&self) -> Result<(), HeartbeatError> {
        info!(
            check_interval = ?self.config.check_interval,
            failure_threshold = self.config.failure_threshold,
            "Starting heartbeat watcher"
        );

        let mut check_interval = interval(self.config.check_interval);

        loop {
            check_interval.tick().await;
            if let Err(e) = self.check_all_probes().await {
                error!(error = %e, "Error checking probes");
            }
        }
    }

    /// Runs the heartbeat watcher with a shutdown signal.
    ///
    /// The watcher runs until `shutdown_signal` fires.
    pub async fn run_until_shutdown<F>(&self, shutdown_signal: F)
    where
        F: std::future::Future<Output = ()>,
    {
        tokio::select! {
            result = self.run() => {
                if let Err(e) = result {
                    error!(error = %e, "Heartbeat watcher error");
                }
            }
            _ = shutdown_signal => {
                info!("Heartbeat watcher shutting down");
            }
        }
    }

    async fn check_all_probes(&self) -> Result<(), HeartbeatError> {
        let definitions: Vec<_> = {
            let registry = self.probe_registry.read().await;
            registry
                .list()
                .iter()
                .map(|d| (d.id, d.name.clone(), d.config.clone()))
                .collect()
        };

        for (probe_id, actor_id, config) in definitions {
            let result = self.check_probe_by_config(probe_id, &config).await;

            match result {
                Ok(probe_result) => {
                    self.handle_probe_result(&actor_id, &probe_result).await;
                }
                Err(e) => {
                    warn!(
                        actor_id = %actor_id,
                        error = %e,
                        "Probe check failed"
                    );
                    self.record_failure(&actor_id).await;
                }
            }
        }

        Ok(())
    }

    async fn check_probe_by_config(
        &self,
        probe_id: ProbeId,
        config: &ProbeConfig,
    ) -> Result<ProbeResult, ProbeError> {
        let probe: Box<dyn Probe> = match config {
            ProbeConfig::Http {
                url,
                expected_status,
                timeout_ms,
            } => Box::new(
                crate::probe::HttpProbe::new(url.clone())
                    .with_expected_status(expected_status.unwrap_or(200))
                    .with_timeout(Duration::from_millis(*timeout_ms)),
            ),
            ProbeConfig::Tcp {
                address,
                port,
                timeout_ms,
            } => {
                use std::net::SocketAddr;
                let addr: SocketAddr = format!("{}:{}", address, port).parse().map_err(|_| {
                    ProbeError::Tcp(format!("invalid address {}:{}", address, port))
                })?;
                Box::new(
                    crate::probe::TcpProbe::new(addr)
                        .with_timeout(Duration::from_millis(*timeout_ms)),
                )
            }
            ProbeConfig::Exec {
                command,
                args,
                expected_exit_code,
                timeout_ms,
            } => Box::new(
                crate::probe::ExecProbe::new(command.clone(), args.clone())
                    .with_expected_exit_code(expected_exit_code.unwrap_or(0))
                    .with_timeout(Duration::from_millis(*timeout_ms)),
            ),
        };

        let mut result = probe.check().await?;
        result.probe_id = probe_id;
        Ok(result)
    }

    async fn handle_probe_result(&self, actor_id: &str, result: &ProbeResult) {
        match result.status {
            ProbeStatus::Healthy => {
                info!(
                    actor_id = %actor_id,
                    latency_ms = result.latency_ms,
                    "Actor health check passed"
                );
                self.record_healthy(actor_id).await;
            }
            ProbeStatus::Unhealthy => {
                warn!(
                    actor_id = %actor_id,
                    consecutive_failures = result.consecutive_failures,
                    message = ?result.message,
                    "Actor health check failed"
                );
                self.record_failure(actor_id).await;
            }
            ProbeStatus::Unknown => {
                warn!(
                    actor_id = %actor_id,
                    "Actor health status unknown"
                );
                self.record_failure(actor_id).await;
            }
        }
    }

    async fn record_healthy(&self, actor_id: &str) {
        let mut states = self.actor_states.write().await;
        if let Some(state) = states.get_mut(actor_id) {
            state.record_healthy();
        }
    }

    async fn record_failure(&self, actor_id: &str) {
        let mut states = self.actor_states.write().await;
        let state = states.entry(actor_id.to_string()).or_default();
        state.record_failure();

        if state.consecutive_failures >= self.config.failure_threshold {
            warn!(
                actor_id = %actor_id,
                consecutive_failures = state.consecutive_failures,
                threshold = self.config.failure_threshold,
                "Actor exceeded failure threshold, triggering shutdown"
            );
            drop(states);
            self.trigger_shutdown(actor_id);
        }
    }

    fn trigger_shutdown(&self, actor_id: &str) {
        if let Some(ref callback) = self.shutdown_callback {
            match callback(actor_id.to_string()) {
                Ok(()) => {
                    info!(actor_id = %actor_id, "Graceful shutdown initiated for unresponsive actor");
                }
                Err(e) => {
                    error!(
                        actor_id = %actor_id,
                        error = %e,
                        "Failed to trigger graceful shutdown"
                    );
                }
            }
        } else {
            warn!(
                actor_id = %actor_id,
                "No shutdown callback configured, actor may remain unresponsive"
            );
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HeartbeatError {
    #[error("probe error: {0}")]
    Probe(#[from] ProbeError),
    #[error("registry error: {0}")]
    Registry(String),
}

/// Helper to run the heartbeat watcher with lifecycle integration.
pub async fn run_heartbeat_watcher() {
    let _config = HeartbeatWatcherConfig::default();
    let watcher = HeartbeatWatcher::with_defaults();

    info!("Heartbeat watcher started");
    watcher.run().await.expect("heartbeat watcher error");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::Probe;

    struct MockProbe {
        id: ProbeId,
        healthy: bool,
    }

    impl MockProbe {
        fn new(healthy: bool) -> Self {
            Self {
                id: ProbeId::new(),
                healthy,
            }
        }
    }

    #[async_trait::async_trait]
    impl Probe for MockProbe {
        async fn check(&self) -> Result<ProbeResult, ProbeError> {
            if self.healthy {
                Ok(ProbeResult {
                    probe_id: self.id,
                    status: ProbeStatus::Healthy,
                    latency_ms: 10,
                    consecutive_failures: 0,
                    last_check_ms: 0,
                    message: Some("healthy".to_string()),
                })
            } else {
                Ok(ProbeResult {
                    probe_id: self.id,
                    status: ProbeStatus::Unhealthy,
                    latency_ms: 0,
                    consecutive_failures: 1,
                    last_check_ms: 0,
                    message: Some("unhealthy".to_string()),
                })
            }
        }

        fn probe_id(&self) -> ProbeId {
            self.id
        }
    }

    #[tokio::test]
    async fn test_heartbeat_watcher_registers_actor() {
        let watcher = HeartbeatWatcher::with_defaults();
        watcher.register_actor("actor-1".to_string()).await;

        let states = watcher.actor_states.read().await;
        assert!(states.contains_key("actor-1"));
    }

    #[tokio::test]
    async fn test_heartbeat_watcher_unregisters_actor() {
        let watcher = HeartbeatWatcher::with_defaults();
        watcher.register_actor("actor-1".to_string()).await;
        watcher.unregister_actor("actor-1").await;

        let states = watcher.actor_states.read().await;
        assert!(!states.contains_key("actor-1"));
    }

    #[tokio::test]
    async fn test_actor_health_state_records_healthy() {
        let mut state = ActorHealthState::new();
        state.record_healthy();
        assert_eq!(state.consecutive_failures, 0);
        assert!(state.last_healthy.is_some());
    }

    #[tokio::test]
    async fn test_actor_health_state_records_failure() {
        let mut state = ActorHealthState::new();
        state.record_failure();
        assert_eq!(state.consecutive_failures, 1);
        state.record_failure();
        assert_eq!(state.consecutive_failures, 2);
        assert!(state.last_check.is_some());
    }

    #[tokio::test]
    async fn test_actor_health_state_healthy_resets_failures() {
        let mut state = ActorHealthState::new();
        state.record_failure();
        state.record_failure();
        assert_eq!(state.consecutive_failures, 2);
        state.record_healthy();
        assert_eq!(state.consecutive_failures, 0);
    }

    #[tokio::test]
    async fn test_shutdown_callback_invoked_on_threshold() {
        let config = HeartbeatWatcherConfig {
            failure_threshold: 2,
            ..Default::default()
        };

        let shutdown_called = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let shutdown_called_clone = shutdown_called.clone();

        let callback: ShutdownCallback = Box::new(move |actor_id| {
            assert_eq!(actor_id, "test-actor");
            shutdown_called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        });

        let watcher = HeartbeatWatcher::new(config.clone()).with_shutdown_callback(callback);

        watcher.register_actor("test-actor".to_string()).await;

        watcher.record_failure("test-actor").await;
        assert!(!shutdown_called.load(std::sync::atomic::Ordering::SeqCst));

        watcher.record_failure("test-actor").await;
        assert!(shutdown_called.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_config_default_values() {
        let config = HeartbeatWatcherConfig::default();
        assert_eq!(config.check_interval, Duration::from_secs(30));
        assert_eq!(config.failure_threshold, 3);
        assert_eq!(config.graceful_shutdown_timeout, Duration::from_secs(30));
    }
}
