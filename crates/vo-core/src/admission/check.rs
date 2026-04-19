//! Admission check functions.
//!
//! Implements degraded-mode admission coupling to write pressure indicators.

#![allow(unexpected_cfgs)]

#[allow(unused_imports)]
use crate::admission::types::{
    AdmissionError, AdmissionThresholds, PressureIndicator, WritePressureState,
};

/// Admit a new write under degraded-mode pressure coupling.
///
/// # Preconditions
/// - `state` must have all fields initialized (non-negative values)
/// - Operation must be within a single actor context
///
/// # Postconditions
/// - Returns `Ok(())` if admission is granted
/// - Returns `Err(AdmissionError)` with specific indicator if pressure threshold exceeded
///
/// # Errors
/// - `AdmissionError::MetricsUnavailable` if pressure metrics cannot be read
/// - `AdmissionError::InvalidAdmissionContext` if operation spans multiple actors
/// - `AdmissionError::WriterQueueDepthExceeded` if write queue over threshold
/// - `AdmissionError::BatchCommitLatencyExceeded` if commit latency over threshold
/// - `AdmissionError::BlobQueueDepthExceeded` if blob queue over threshold
/// - `AdmissionError::CompactionStallActive` if compaction is stalled
/// - `AdmissionError::StorageStallActive` if storage is stalled
/// - `AdmissionError::MultiplePressureIndicators` if multiple indicators exceeded
pub fn check_admission(state: &WritePressureState) -> Result<(), AdmissionError> {
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: 100,
        batch_commit_latency_ms_threshold: 1000,
        blob_queue_depth_threshold: 50,
        max_queued_memory_bytes: 512 * 1024 * 1024,
    };
    check_admission_with_thresholds(state, &thresholds)
}

/// Check admission with explicit thresholds (for testing and configuration).
pub fn check_admission_with_thresholds(
    state: &WritePressureState,
    thresholds: &AdmissionThresholds,
) -> Result<(), AdmissionError> {
    let mut indicators = Vec::new();

    if state.writer_queue_depth > thresholds.writer_queue_depth_threshold {
        indicators.push(PressureIndicator::WriterQueueDepth);
    }
    if state.batch_commit_latency_ms > thresholds.batch_commit_latency_ms_threshold {
        indicators.push(PressureIndicator::BatchCommitLatency);
    }
    if state.blob_queue_depth > thresholds.blob_queue_depth_threshold {
        indicators.push(PressureIndicator::BlobQueueDepth);
    }
    if state.compaction_stall_active {
        indicators.push(PressureIndicator::CompactionStall);
    }
    if state.storage_stall_active {
        indicators.push(PressureIndicator::StorageStall);
    }

    match indicators.len() {
        0 => Ok(()),
        1 => match indicators[0] {
            PressureIndicator::WriterQueueDepth => Err(AdmissionError::WriterQueueDepthExceeded {
                current_depth: state.writer_queue_depth,
                threshold: thresholds.writer_queue_depth_threshold,
            }),
            PressureIndicator::BatchCommitLatency => {
                Err(AdmissionError::BatchCommitLatencyExceeded {
                    current_latency_ms: state.batch_commit_latency_ms,
                    threshold_ms: thresholds.batch_commit_latency_ms_threshold,
                })
            }
            PressureIndicator::BlobQueueDepth => Err(AdmissionError::BlobQueueDepthExceeded {
                current_depth: state.blob_queue_depth,
                threshold: thresholds.blob_queue_depth_threshold,
            }),
            PressureIndicator::CompactionStall => Err(AdmissionError::CompactionStallActive),
            PressureIndicator::StorageStall => Err(AdmissionError::StorageStallActive),
            PressureIndicator::MemoryLimit => Err(AdmissionError::MemoryLimitExceeded {
                current_bytes: 0,
                max_bytes: thresholds.max_queued_memory_bytes,
            }),
        },
        _ => Err(AdmissionError::MultiplePressureIndicators { indicators }),
    }
}
