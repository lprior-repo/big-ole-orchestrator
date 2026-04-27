//! Storage watchdog types.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::admission::types::PressureIndicator;

/// Configuration for the storage watchdog background task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StorageWatchdogConfig {
    /// How often the watchdog runs its health check cycle.
    pub check_interval: Duration,
    /// Filesystem free space percentage below which critical degraded mode is triggered.
    pub disk_space_critical_percent: f64,
    /// Filesystem free space percentage below which degraded mode is triggered.
    pub disk_space_warn_percent: f64,
    /// Writer queue depth threshold for degraded mode.
    pub writer_queue_depth_threshold: u64,
    /// Batch commit latency threshold (ms) for degraded mode.
    pub commit_latency_ms_threshold: u64,
    /// Blob queue depth threshold for degraded mode.
    pub blob_queue_depth_threshold: u64,
    /// Maximum flush timeout count within the monitoring window before triggering.
    pub flush_timeout_count_threshold: u64,
    /// Duration of the flush timeout monitoring window.
    pub flush_timeout_window: Duration,
    /// Compaction backlog size threshold (number of SSTables or memtable flushes pending).
    pub compaction_backlog_threshold: u64,
    /// Whether a compaction stall is currently active.
    pub compaction_stall_active: bool,
    /// Whether a storage stall is currently active.
    pub storage_stall_active: bool,
    /// Watchdog task poll interval.
    pub poll_interval: Duration,
}

impl Default for StorageWatchdogConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(10),
            disk_space_critical_percent: 5.0,
            disk_space_warn_percent: 15.0,
            writer_queue_depth_threshold: 500,
            commit_latency_ms_threshold: 2000,
            blob_queue_depth_threshold: 200,
            flush_timeout_count_threshold: 3,
            flush_timeout_window: Duration::from_secs(60),
            compaction_backlog_threshold: 1000,
            compaction_stall_active: false,
            storage_stall_active: false,
            poll_interval: Duration::from_secs(5),
        }
    }
}

/// Current filesystem disk space metrics.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DiskSpaceMetrics {
    /// Total disk space in bytes.
    pub total_bytes: u64,
    /// Used disk space in bytes.
    pub used_bytes: u64,
    /// Free disk space in bytes.
    pub free_bytes: u64,
    /// Free space as a percentage of total.
    pub free_percent: f64,
}

impl DiskSpaceMetrics {
    /// Creates new disk space metrics.
    #[must_use]
    pub fn new(total_bytes: u64, used_bytes: u64, free_bytes: u64) -> Self {
        let free_percent = if total_bytes > 0 {
            (free_bytes as f64 / total_bytes as f64) * 100.0
        } else {
            100.0
        };
        Self {
            total_bytes,
            used_bytes,
            free_bytes,
            free_percent,
        }
    }

    /// Returns true if free space is below the warn threshold.
    #[must_use]
    pub fn is_warn(&self, threshold_percent: f64) -> bool {
        self.free_percent < threshold_percent
    }

    /// Returns true if free space is below the critical threshold.
    #[must_use]
    pub fn is_critical(&self, threshold_percent: f64) -> bool {
        self.free_percent < threshold_percent
    }
}

impl Default for DiskSpaceMetrics {
    fn default() -> Self {
        Self {
            total_bytes: 0,
            used_bytes: 0,
            free_bytes: 0,
            free_percent: 100.0,
        }
    }
}

/// Flush timeout tracking configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlushTimeoutConfig {
    /// Maximum flush timeout count within the monitoring window before triggering degraded mode.
    pub count_threshold: u64,
    /// Duration of the flush timeout monitoring window.
    pub window: Duration,
}

impl Default for FlushTimeoutConfig {
    fn default() -> Self {
        Self {
            count_threshold: 3,
            window: Duration::from_secs(60),
        }
    }
}

/// Aggregated storage health metrics from all monitoring sources.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StorageMetrics {
    /// Disk space metrics from filesystem.
    pub disk_space: DiskSpaceMetrics,
    /// Current writer queue depth.
    pub writer_queue_depth: u64,
    /// Current batch commit latency in milliseconds.
    pub commit_latency_ms: u64,
    /// Current blob queue depth.
    pub blob_queue_depth: u64,
    /// Number of flush timeouts in the monitoring window.
    pub flush_timeout_count: u64,
    /// Current compaction backlog (pending flushes/SSTables).
    pub compaction_backlog: u64,
    /// Whether a compaction stall is currently active.
    pub compaction_stall_active: bool,
    /// Whether a storage stall is currently active.
    pub storage_stall_active: bool,
}

impl StorageMetrics {
    /// Converts this into a WritePressureState for admission coupling.
    #[must_use]
    pub fn to_write_pressure_state(&self) -> crate::admission::types::WritePressureState {
        crate::admission::types::WritePressureState {
            writer_queue_depth: self.writer_queue_depth,
            batch_commit_latency_ms: self.commit_latency_ms,
            blob_queue_depth: self.blob_queue_depth,
            compaction_stall_active: self.compaction_stall_active,
            storage_stall_active: self.storage_stall_active,
        }
    }

    /// Returns all pressure indicators that are currently exceeded.
    pub fn exceeded_indicators(&self, config: &StorageWatchdogConfig) -> Vec<PressureIndicator> {
        let mut indicators = Vec::new();

        if self.writer_queue_depth > config.writer_queue_depth_threshold {
            indicators.push(PressureIndicator::WriterQueueDepth);
        }
        if self.commit_latency_ms > config.commit_latency_ms_threshold {
            indicators.push(PressureIndicator::BatchCommitLatency);
        }
        if self.blob_queue_depth > config.blob_queue_depth_threshold {
            indicators.push(PressureIndicator::BlobQueueDepth);
        }
        if self.compaction_stall_active {
            indicators.push(PressureIndicator::CompactionStall);
        }
        if self.storage_stall_active {
            indicators.push(PressureIndicator::StorageStall);
        }
        if self
            .disk_space
            .is_critical(config.disk_space_critical_percent)
        {
            indicators.push(PressureIndicator::StorageStall);
        } else if self.disk_space.is_warn(config.disk_space_warn_percent) {
            indicators.push(PressureIndicator::WriterQueueDepth);
        }
        if self.flush_timeout_count >= config.flush_timeout_count_threshold {
            indicators.push(PressureIndicator::BatchCommitLatency);
        }
        if self.compaction_backlog > config.compaction_backlog_threshold {
            indicators.push(PressureIndicator::CompactionStall);
        }

        indicators
    }
}

/// Overall storage health status from the watchdog's perspective.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageHealth {
    /// All storage indicators within normal thresholds.
    Healthy,
    /// Some indicators degraded but not critical.
    Degraded {
        /// Which indicators are degraded.
        indicators: Vec<PressureIndicator>,
    },
    /// Critical thresholds exceeded — system may shut down if writer stalls.
    Critical {
        /// Which indicators are critical.
        indicators: Vec<PressureIndicator>,
        /// Whether the writer is unable to make forward progress.
        writer_stalled: bool,
    },
}

impl StorageHealth {
    /// Returns true if health is Healthy.
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        matches!(self, StorageHealth::Healthy)
    }

    /// Returns true if health is Degraded or Critical.
    #[must_use]
    pub fn is_degraded(&self) -> bool {
        matches!(
            self,
            StorageHealth::Degraded { .. } | StorageHealth::Critical { .. }
        )
    }

    /// Returns true if health is Critical.
    #[must_use]
    pub fn is_critical(&self) -> bool {
        matches!(self, StorageHealth::Critical { .. })
    }

    /// Whether the writer is stalled and the system should shut down.
    #[must_use]
    pub fn writer_stalled(&self) -> bool {
        matches!(
            self,
            StorageHealth::Critical {
                writer_stalled: true,
                ..
            }
        )
    }

    /// Returns the indicators for this health state.
    #[must_use]
    pub fn indicators(&self) -> Vec<PressureIndicator> {
        match self {
            StorageHealth::Healthy => Vec::new(),
            StorageHealth::Degraded { indicators } => indicators.clone(),
            StorageHealth::Critical { indicators, .. } => indicators.clone(),
        }
    }
}
