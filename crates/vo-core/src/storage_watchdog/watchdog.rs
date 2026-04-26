//! Storage watchdog — background tokio task that monitors storage health and triggers degraded mode.
//!
//! Per ADR-013 §2:
//! "A background Tokio task monitors filesystem free space, DbWriterActor commit latency,
//! writer queue depth, flush timeout frequency, and storage stall or compaction-backlog
//! indicators."
//!
//! "If critical thresholds are crossed, the Engine enters Degraded Mode."

use std::sync::Arc;

use tokio::sync::watch;
use tokio::time::{interval, MissedTickBehavior};
use tokio::task::JoinHandle;
use tracing::{error, warn};

use super::monitor::{read_storage_metrics, FlushTimeoutTracker};
use super::types::{FlushTimeoutConfig, StorageHealth, StorageMetrics, StorageWatchdogConfig};
use crate::admission::types::PressureIndicator;

/// Handle for controlling the Storage Watchdog.
#[derive(Debug)]
pub struct StorageWatchdogHandle {
    shutdown_trigger: tokio::sync::watch::Sender<()>,
    task_handle: Option<JoinHandle<()>>,
    health_receiver: watch::Receiver<StorageHealth>,
}

impl StorageWatchdogHandle {
    /// Requests the watchdog to shut down.
    #[tracing::instrument(skip(self))]
    pub async fn shutdown(mut self) {
        let _ = self.shutdown_trigger.send(());
        if let Some(task) = self.task_handle.take() {
            let _ = task.await;
        }
    }

    /// Gets the current health state reported by the watchdog.
    #[must_use]
    pub fn current_health(&self) -> StorageHealth {
        self.health_receiver.borrow().clone()
    }

    /// Returns true if the watchdog considers storage healthy.
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.health_receiver.borrow().is_healthy()
    }

    /// Returns true if the watchdog considers storage degraded or critical.
    #[must_use]
    pub fn is_degraded(&self) -> bool {
        self.health_receiver.borrow().is_degraded()
    }

    /// Returns true if the watchdog reports critical health.
    #[must_use]
    pub fn is_critical(&self) -> bool {
        self.health_receiver.borrow().is_critical()
    }

    /// Returns true if the writer is stalled and the engine should shut down.
    #[must_use]
    pub fn writer_stalled(&self) -> bool {
        self.health_receiver.borrow().writer_stalled()
    }

    /// Returns the degraded-mode triggers.
    #[must_use]
    pub fn triggers(&self) -> Vec<PressureIndicator> {
        self.health_receiver.borrow().indicators()
    }
}

/// Background task that monitors storage health indicators.
///
/// # ADR-013 Compliance
///
/// Implements the storage watchdog from ADR-013 §2:
/// - Monitors filesystem free space via `statvfs`
/// - Tracks commit latency, queue depths, flush timeouts, compaction backlog
/// - Triggers degraded mode when critical thresholds are crossed
/// - Reports writer stall for clean engine shutdown
pub struct StorageWatchdog;

impl StorageWatchdog {
    /// Spawns the storage watchdog as a background tokio task.
    ///
    /// # Arguments
    /// * `config` - Watchdog configuration with thresholds.
    /// * `data_path` - Path to the storage data directory for disk space monitoring.
    /// * `flush_timeout_tx` - Channel sender for flush timeout events.
    /// * `health_sender` - A watch channel sender for broadcasting health state.
    /// * `health_receiver` - A watch channel receiver for the health state.
    /// * `compaction_stall_rx` - Receiver for compaction stall events.
    /// * `storage_stall_rx` - Receiver for storage stall events.
    ///
    /// # Returns
    /// A `StorageWatchdogHandle` for controlling the watchdog.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        config: StorageWatchdogConfig,
        data_path: String,
        flush_timeout_tx: tokio::sync::mpsc::Sender<()>,
        health_sender: watch::Sender<StorageHealth>,
        health_receiver: watch::Receiver<StorageHealth>,
        mut compaction_stall_rx: watch::Receiver<bool>,
        mut storage_stall_rx: watch::Receiver<bool>,
    ) -> StorageWatchdogHandle {
        let (shutdown_trigger, mut shutdown_rx) = watch::channel(());
        let config = Arc::new(config);
        let data_path = Arc::new(data_path);

        let task_handle = tokio::spawn(async move {
            let _ = health_sender.send(StorageHealth::Healthy);

            let mut flush_tracker = FlushTimeoutTracker::new(FlushTimeoutConfig {
                count_threshold: config.flush_timeout_count_threshold,
                window: config.flush_timeout_window,
            });

            let mut poll = interval(config.poll_interval);
            poll.set_missed_tick_behavior(MissedTickBehavior::Skip);

            let mut last_compaction_stall = false;
            let mut last_storage_stall = false;

            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        let _ = health_sender.send(StorageHealth::Healthy);
                        break;
                    }
                    _ = poll.tick() => {
                        if compaction_stall_rx.has_changed().unwrap_or(false) {
                            last_compaction_stall = *compaction_stall_rx.borrow_and_update();
                        }

                        if storage_stall_rx.has_changed().unwrap_or(false) {
                            last_storage_stall = *storage_stall_rx.borrow_and_update();
                        }

                        let metrics = read_storage_metrics(
                            &data_path,
                            0,
                            0,
                            0,
                            0,
                            last_compaction_stall,
                            last_storage_stall,
                            &flush_tracker,
                        );

                        let health = Self::compute_health(&metrics, &config);

                        let current = health_sender.borrow();
                        if *current != health {
                            if health.is_critical() {
                                let triggers = health.indicators();
                                if triggers.len() >= 3 || health.writer_stalled() {
                                    error!(
                                        triggers = ?triggers,
                                        writer_stalled = health.writer_stalled(),
                                        "Storage watchdog: CRITICAL — entering degraded mode"
                                    );
                                } else {
                                    warn!(
                                        triggers = ?triggers,
                                        "Storage watchdog: DEGRADED — restricting non-critical workloads"
                                    );
                                }
                            }

                            let _ = health_sender.send(health.clone());
                        }

                        if health.writer_stalled() {
                            error!("Storage watchdog: writer stalled — engine should shut down cleanly");
                        }
                    }
                }
            }

            let _ = health_sender.send(StorageHealth::Healthy);
        });

        StorageWatchdogHandle {
            shutdown_trigger,
            task_handle: Some(task_handle),
            health_receiver,
        }
    }

    /// Computes storage health from metrics and configuration.
    ///
    /// Returns `StorageHealth::Healthy` if all indicators are within thresholds,
    /// `StorageHealth::Degraded` if 1-2 indicators are exceeded,
    /// or `StorageHealth::Critical` if 3+ indicators are exceeded or writer is stalled.
    #[must_use]
    pub fn compute_health(metrics: &StorageMetrics, config: &StorageWatchdogConfig) -> StorageHealth {
        let indicators = metrics.exceeded_indicators(config);

        if indicators.is_empty() {
            return StorageHealth::Healthy;
        }

        let writer_stalled = metrics.commit_latency_ms > config.commit_latency_ms_threshold * 5
            || metrics.writer_queue_depth > config.writer_queue_depth_threshold * 3
            || metrics.disk_space.is_critical(config.disk_space_critical_percent / 2.0);

        if indicators.len() >= 3 || writer_stalled {
            StorageHealth::Critical {
                indicators,
                writer_stalled,
            }
        } else {
            StorageHealth::Degraded { indicators }
        }
    }

    /// Informs the watchdog of a flush timeout event.
    ///
    /// This is called by the DbWriterActor when a flush operation times out.
    pub fn record_flush_timeout(flush_tx: &tokio::sync::mpsc::Sender<()>) {
        let _ = flush_tx.blocking_send(());
    }
}
