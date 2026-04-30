//! Heartbeat watcher internal methods.
//!
//! Contains private helper methods for probe checking, health state recording,
//! and failure threshold management.

use std::time::Duration;

use tracing::{error, info, warn};

use crate::probe::{Probe, ProbeConfig, ProbeError, ProbeId, ProbeResult, ProbeStatus};

use super::config::HeartbeatWatcherConfig;
use super::detector::ActorHealthState;
use super::runner::HeartbeatWatcher;

impl HeartbeatWatcher {
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
