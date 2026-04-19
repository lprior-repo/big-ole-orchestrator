//! Write pressure state and admission types.
//!
//! Defines the core data structures for degraded-mode admission coupling.

use serde::{Deserialize, Serialize};
use vo_types::InstanceId;

/// Represents current write pressure state.
///
/// All u64 fields represent current gauge values and must be non-negative.
/// Boolean fields indicate whether critical stall indicators are active.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WritePressureState {
    /// Current depth of the writer queue.
    pub writer_queue_depth: u64,
    /// Current batch commit latency in milliseconds.
    pub batch_commit_latency_ms: u64,
    /// Current depth of the blob queue.
    pub blob_queue_depth: u64,
    /// Whether a compaction stall is currently active.
    pub compaction_stall_active: bool,
    /// Whether a storage stall is currently active.
    pub storage_stall_active: bool,
}

/// Pressure indicator enum for composite errors.
///
/// Enumerates all possible pressure indicators that can contribute to
/// a degraded-mode admission rejection.
#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize, Hash)]
pub enum PressureIndicator {
    /// Writer queue depth exceeded threshold.
    WriterQueueDepth,
    /// Batch commit latency exceeded threshold.
    BatchCommitLatency,
    /// Blob queue depth exceeded threshold.
    BlobQueueDepth,
    /// Compaction stall indicator is active.
    CompactionStall,
    /// Storage stall indicator is active.
    StorageStall,
    /// Queued memory exceeded limit.
    MemoryLimit,
}

/// Errors for degraded-mode admission coupling violations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionError {
    /// Writer queue depth exceeded threshold.
    WriterQueueDepthExceeded {
        /// Current queue depth.
        current_depth: u64,
        /// Threshold that was exceeded.
        threshold: u64,
    },
    /// Batch commit latency exceeded threshold.
    BatchCommitLatencyExceeded {
        /// Current latency in milliseconds.
        current_latency_ms: u64,
        /// Threshold in milliseconds that was exceeded.
        threshold_ms: u64,
    },
    /// Blob queue depth exceeded threshold.
    BlobQueueDepthExceeded {
        /// Current queue depth.
        current_depth: u64,
        /// Threshold that was exceeded.
        threshold: u64,
    },
    /// Compaction stall indicator is active.
    CompactionStallActive,
    /// Storage stall indicator is active.
    StorageStallActive,
    /// Queued memory limit exceeded.
    MemoryLimitExceeded {
        /// Current queued memory in bytes.
        current_bytes: u64,
        /// Maximum allowed queued memory in bytes.
        max_bytes: u64,
    },
    /// Multiple pressure indicators exceeded (composite).
    MultiplePressureIndicators {
        /// List of pressure indicators that exceeded thresholds.
        indicators: Vec<PressureIndicator>,
    },
    /// Precondition violated: metrics unavailable.
    MetricsUnavailable,
    /// Precondition violated: context not bounded to single actor.
    InvalidAdmissionContext,
    /// Command is a duplicate of an already-admitted command.
    Duplicate {
        /// The instance ID of the original command.
        original_instance_id: InstanceId,
    },
    /// A generic admission policy violation with a human-readable message.
    PolicyViolation(String),
}

/// Configurable thresholds for admission decisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmissionThresholds {
    /// Threshold for writer queue depth.
    pub writer_queue_depth_threshold: u64,
    /// Threshold for batch commit latency in milliseconds.
    pub batch_commit_latency_ms_threshold: u64,
    /// Threshold for blob queue depth.
    pub blob_queue_depth_threshold: u64,
    /// Maximum queued memory in bytes (default: 512 MiB).
    pub max_queued_memory_bytes: u64,
}

impl Default for AdmissionThresholds {
    fn default() -> Self {
        Self {
            writer_queue_depth_threshold: 100,
            batch_commit_latency_ms_threshold: 1000,
            blob_queue_depth_threshold: 50,
            max_queued_memory_bytes: 512 * 1024 * 1024,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── WritePressureState Tests ─────────────────────────────────────────────────

    #[test]
    fn write_pressure_state_default_is_all_zero() {
        let state = WritePressureState::default();
        assert_eq!(state.writer_queue_depth, 0);
        assert_eq!(state.batch_commit_latency_ms, 0);
        assert_eq!(state.blob_queue_depth, 0);
        assert!(!state.compaction_stall_active);
        assert!(!state.storage_stall_active);
    }

    #[test]
    fn write_pressure_state_accepts_all_zero_values() {
        let state = WritePressureState {
            writer_queue_depth: 0,
            batch_commit_latency_ms: 0,
            blob_queue_depth: 0,
            compaction_stall_active: false,
            storage_stall_active: false,
        };
        assert_eq!(state.writer_queue_depth, 0);
        assert_eq!(state.batch_commit_latency_ms, 0);
        assert_eq!(state.blob_queue_depth, 0);
        assert!(!state.compaction_stall_active);
        assert!(!state.storage_stall_active);
    }

    #[test]
    fn write_pressure_state_accepts_max_u64_values() {
        let state = WritePressureState {
            writer_queue_depth: u64::MAX,
            batch_commit_latency_ms: u64::MAX,
            blob_queue_depth: u64::MAX,
            compaction_stall_active: true,
            storage_stall_active: true,
        };
        assert_eq!(state.writer_queue_depth, u64::MAX);
        assert_eq!(state.batch_commit_latency_ms, u64::MAX);
        assert_eq!(state.blob_queue_depth, u64::MAX);
    }

    // ── AdmissionThresholds Tests ───────────────────────────────────────────────

    #[test]
    fn admission_thresholds_default_produces_sensible_values() {
        let thresholds = AdmissionThresholds::default();
        assert_eq!(thresholds.writer_queue_depth_threshold, 100);
        assert_eq!(thresholds.batch_commit_latency_ms_threshold, 1000);
        assert_eq!(thresholds.blob_queue_depth_threshold, 50);
    }

    #[test]
    fn admission_thresholds_can_be_constructed_with_custom_values() {
        let thresholds = AdmissionThresholds {
            writer_queue_depth_threshold: 100,
            batch_commit_latency_ms_threshold: 1000,
            blob_queue_depth_threshold: 50,
            max_queued_memory_bytes: 512 * 1024 * 1024,
        };
        assert_eq!(thresholds.writer_queue_depth_threshold, 100);
        assert_eq!(thresholds.batch_commit_latency_ms_threshold, 1000);
        assert_eq!(thresholds.blob_queue_depth_threshold, 50);
    }

    // ── PressureIndicator Tests ─────────────────────────────────────────────────

    #[test]
    fn pressure_indicator_all_variants_exist() {
        let indicators = [
            PressureIndicator::WriterQueueDepth,
            PressureIndicator::BatchCommitLatency,
            PressureIndicator::BlobQueueDepth,
            PressureIndicator::CompactionStall,
            PressureIndicator::StorageStall,
            PressureIndicator::MemoryLimit,
        ];
        assert_eq!(indicators.len(), 6);
    }

    #[test]
    fn pressure_indicator_equality() {
        assert_eq!(
            PressureIndicator::WriterQueueDepth,
            PressureIndicator::WriterQueueDepth
        );
        assert_eq!(
            PressureIndicator::BatchCommitLatency,
            PressureIndicator::BatchCommitLatency
        );
        assert_eq!(
            PressureIndicator::BlobQueueDepth,
            PressureIndicator::BlobQueueDepth
        );
        assert_eq!(
            PressureIndicator::CompactionStall,
            PressureIndicator::CompactionStall
        );
        assert_eq!(
            PressureIndicator::StorageStall,
            PressureIndicator::StorageStall
        );
        assert_eq!(
            PressureIndicator::MemoryLimit,
            PressureIndicator::MemoryLimit
        );
    }

    #[test]
    fn pressure_indicator_inequality() {
        assert_ne!(
            PressureIndicator::WriterQueueDepth,
            PressureIndicator::BatchCommitLatency
        );
        assert_ne!(
            PressureIndicator::BlobQueueDepth,
            PressureIndicator::CompactionStall
        );
        assert_ne!(
            PressureIndicator::StorageStall,
            PressureIndicator::WriterQueueDepth
        );
        assert_ne!(
            PressureIndicator::MemoryLimit,
            PressureIndicator::WriterQueueDepth
        );
    }

    // ── AdmissionError Tests ───────────────────────────────────────────────────

    #[test]
    fn admission_error_writer_queue_depth_exceeded() {
        let err = AdmissionError::WriterQueueDepthExceeded {
            current_depth: 150,
            threshold: 100,
        };
        assert_eq!(err.clone(), err);
    }

    #[test]
    fn admission_error_batch_commit_latency_exceeded() {
        let err = AdmissionError::BatchCommitLatencyExceeded {
            current_latency_ms: 1500,
            threshold_ms: 1000,
        };
        assert_eq!(err.clone(), err);
    }

    #[test]
    fn admission_error_blob_queue_depth_exceeded() {
        let err = AdmissionError::BlobQueueDepthExceeded {
            current_depth: 100,
            threshold: 50,
        };
        assert_eq!(err.clone(), err);
    }

    #[test]
    fn admission_error_compaction_stall_active() {
        let err = AdmissionError::CompactionStallActive;
        assert_eq!(err.clone(), err);
    }

    #[test]
    fn admission_error_storage_stall_active() {
        let err = AdmissionError::StorageStallActive;
        assert_eq!(err.clone(), err);
    }

    #[test]
    fn admission_error_multiple_pressure_indicators() {
        let indicators = vec![
            PressureIndicator::WriterQueueDepth,
            PressureIndicator::BatchCommitLatency,
        ];
        let err = AdmissionError::MultiplePressureIndicators {
            indicators: indicators.clone(),
        };
        match err {
            AdmissionError::MultiplePressureIndicators { indicators: i } => {
                assert_eq!(i.len(), 2);
            }
            _ => panic!("Expected MultiplePressureIndicators"),
        }
    }

    #[test]
    fn admission_error_metrics_unavailable() {
        let err = AdmissionError::MetricsUnavailable;
        assert_eq!(err.clone(), err);
    }

    #[test]
    fn admission_error_invalid_admission_context() {
        let err = AdmissionError::InvalidAdmissionContext;
        assert_eq!(err.clone(), err);
    }

    #[test]
    fn admission_error_duplicate() {
        let original_id = InstanceId::from_bytes([1u8; 16]);
        let err = AdmissionError::Duplicate {
            original_instance_id: original_id.clone(),
        };
        assert_eq!(err.clone(), err);
    }

    #[test]
    fn admission_error_policy_violation() {
        let err = AdmissionError::PolicyViolation("rate limit exceeded".to_string());
        assert_eq!(err.clone(), err);
    }

    #[test]
    fn admission_error_memory_limit_exceeded() {
        let err = AdmissionError::MemoryLimitExceeded {
            current_bytes: 600_000_000,
            max_bytes: 512 * 1024 * 1024,
        };
        assert_eq!(err.clone(), err);
    }
}
