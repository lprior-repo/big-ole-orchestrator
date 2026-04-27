//! Storage monitor — reads filesystem metrics, flush timeouts, and compaction state.

use std::time::Instant;

use crate::storage_watchdog::types::{DiskSpaceMetrics, FlushTimeoutConfig, StorageMetrics};

/// Tracks flush timeout events for the watchdog.
///
/// Maintains a sliding window of flush timeout timestamps to count how many
/// occurred within the configured monitoring window.
#[derive(Debug, Clone)]
pub struct FlushTimeoutTracker {
    /// Timestamps of recent flush timeout events.
    timeouts: Vec<Instant>,
    /// Configuration for the monitoring window.
    config: FlushTimeoutConfig,
}

impl FlushTimeoutTracker {
    /// Creates a new flush timeout tracker with the given config.
    #[must_use]
    pub fn new(config: FlushTimeoutConfig) -> Self {
        Self {
            timeouts: Vec::new(),
            config,
        }
    }

    /// Records a flush timeout event.
    pub fn record_timeout(&mut self) {
        self.timeouts.push(Instant::now());
        self.prune_stale();
    }

    /// Returns the number of flush timeouts in the monitoring window.
    #[must_use]
    pub fn timeout_count(&self) -> u64 {
        self.timeouts.len() as u64
    }

    /// Prunes timestamps older than the monitoring window.
    fn prune_stale(&mut self) {
        let now = Instant::now();
        let window = self.config.window;
        self.timeouts.retain(|t| now.duration_since(*t) <= window);
    }
}

impl Default for FlushTimeoutTracker {
    fn default() -> Self {
        Self::new(FlushTimeoutConfig::default())
    }
}

/// Reads filesystem disk space metrics.
///
/// On Unix, reads from `/proc/stat` or uses libc `statvfs`.
/// On platforms without libc, returns default (all zeros, 100% free).
#[must_use]
pub fn read_disk_space(path: &str) -> DiskSpaceMetrics {
    disk_space_for_path(path)
}

/// Reads disk space for a specific filesystem path.
#[cfg(unix)]
fn disk_space_for_path(path: &str) -> DiskSpaceMetrics {
    use std::ffi::CString;

    match CString::new(path) {
        Ok(c_path) => {
            let mut statvfs = unsafe { std::mem::zeroed() };
            let result = unsafe { libc::statvfs(c_path.as_ptr(), &mut statvfs) };

            if result == 0 {
                let block_size = statvfs.f_bsize as u64;
                let total_bytes = statvfs.f_blocks * block_size;
                let _free_bytes = statvfs.f_bfree * block_size;
                let available_bytes = statvfs.f_bavail * block_size;

                // Use available (non-root) space for accuracy
                let used_bytes = total_bytes.saturating_sub(available_bytes);

                DiskSpaceMetrics::new(total_bytes, used_bytes, available_bytes)
            } else {
                DiskSpaceMetrics::default()
            }
        }
        Err(_) => DiskSpaceMetrics::default(),
    }
}

#[cfg(not(unix))]
fn disk_space_for_path(_path: &str) -> DiskSpaceMetrics {
    // Fallback for non-Unix platforms — cannot read disk space
    DiskSpaceMetrics::default()
}

/// Reads current storage metrics from all sources.
///
/// # Arguments
/// * `data_path` - Path to the storage data directory for disk space monitoring.
/// * `writer_queue_depth` - Current depth of the writer queue.
/// * `commit_latency_ms` - Current batch commit latency in milliseconds.
/// * `blob_queue_depth` - Current depth of the blob queue.
/// * `compaction_backlog` - Current compaction backlog (pending flushes/SSTables).
/// * `compaction_stall_active` - Whether a compaction stall is currently active.
/// * `storage_stall_active` - Whether a storage stall is currently active.
/// * `flush_tracker` - Flush timeout tracker for counting recent timeouts.
///
/// # Returns
/// Aggregated `StorageMetrics` from all monitoring sources.
#[must_use]
pub fn read_storage_metrics(
    data_path: &str,
    writer_queue_depth: u64,
    commit_latency_ms: u64,
    blob_queue_depth: u64,
    compaction_backlog: u64,
    compaction_stall_active: bool,
    storage_stall_active: bool,
    flush_tracker: &FlushTimeoutTracker,
) -> StorageMetrics {
    StorageMetrics {
        disk_space: read_disk_space(data_path),
        writer_queue_depth,
        commit_latency_ms,
        blob_queue_depth,
        flush_timeout_count: flush_tracker.timeout_count(),
        compaction_backlog,
        compaction_stall_active,
        storage_stall_active,
    }
}
