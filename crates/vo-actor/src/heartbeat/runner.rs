//! Heartbeat watcher struct and public API.
//!
//! Contains the HeartbeatWatcher struct definition, public methods,
//! the HeartbeatError type, and the convenience run_heartbeat_watcher function.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{error, info};

use crate::probe::{Probe, ProbeDefinition, ProbeError, ProbeId, ProbeRegistry};

use super::config::{HeartbeatWatcherConfig, ShutdownCallback};
use super::detector::ActorHealthState;

/// Heartbeat watcher that monitors actor health via probes.
///
/// The watcher:
/// 1. Periodically checks all registered probes
/// 2. Tracks consecutive failures per actor
/// 3. Triggers graceful shutdown when threshold is exceeded
/// 4. Emits structured tracing events
pub struct HeartbeatWatcher {
    pub(crate) config: HeartbeatWatcherConfig,
    pub(crate) probe_registry: Arc<RwLock<ProbeRegistry>>,
    pub(crate) actor_states: Arc<RwLock<HashMap<String, ActorHealthState>>>,
    pub(crate) shutdown_callback: Option<ShutdownCallback>,
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
            config: crate::probe::ProbeConfig::Http {
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
        let id = registry.register(definition);
        id
    }

    /// Registers an actor with a probe configuration without creating the probe.
    /// This allows manual probe management.
    pub async fn register_actor(&self, actor_id: String) {
        let mut states = self.actor_states.write().await;
        if !states.contains_key(&actor_id) {
            states.insert(actor_id, ActorHealthState::new());
        }
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
}

/// Errors that can occur during heartbeat watcher operations.
///
/// Wraps either a [`ProbeError`] from a failed health probe or a generic
/// registry access error.
#[derive(Debug, thiserror::Error)]
pub enum HeartbeatError {
    /// A probe check failed (e.g., HTTP connection refused, TCP timeout).
    #[error("probe error: {0}")]
    Probe(#[from] ProbeError),
    /// A registry operation failed (e.g., probe registry access error).
    #[error("registry error: {0}")]
    Registry(String),
}

/// Helper to run the heartbeat watcher with lifecycle integration.
///
/// Creates a [`HeartbeatWatcher`] with default configuration and starts the
/// monitoring loop. This function runs indefinitely until the process is terminated.
///
/// # Panics
///
/// This function will panic if the watcher encounters an error during `run()`.
/// For production use, prefer constructing a [`HeartbeatWatcher`] directly and
/// handling errors gracefully via [`HeartbeatWatcher::run`] or
/// [`HeartbeatWatcher::run_until_shutdown`].
pub async fn run_heartbeat_watcher() {
    let _config = HeartbeatWatcherConfig::default();
    let watcher = HeartbeatWatcher::with_defaults();

    info!("Heartbeat watcher started");
    watcher.run().await.expect("heartbeat watcher error");
}
