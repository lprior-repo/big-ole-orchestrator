//! Tests for admission check functions.

use super::*;
use crate::admission::types::{
    AdmissionError, AdmissionThresholds, PressureIndicator, WritePressureState,
};

// ── check_admission: Happy Path ─────────────────────────────────────────────

#[test]
fn check_admission_returns_ok_when_all_indicators_within_thresholds() {
    let state = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let result = check_admission(&state);
    assert_eq!(result, Ok(()));
}

#[test]
fn check_admission_returns_ok_with_all_zero_values() {
    let state = WritePressureState {
        writer_queue_depth: 0,
        batch_commit_latency_ms: 0,
        blob_queue_depth: 0,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let result = check_admission(&state);
    assert_eq!(result, Ok(()));
}

#[test]
fn check_admission_returns_ok_at_exact_threshold() {
    // Threshold boundary test - depth == threshold means within limits, not exceeded
    let state = WritePressureState {
        writer_queue_depth: 100, // exactly at default threshold
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let result = check_admission(&state);
    assert_eq!(result, Ok(()));
}

// ── check_admission: Writer Queue Depth Exceeded ───────────────────────────

#[test]
fn check_admission_returns_writer_queue_depth_exceeded_when_over_threshold() {
    let state = WritePressureState {
        writer_queue_depth: 150,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let result = check_admission(&state);
    assert_eq!(
        result,
        Err(AdmissionError::WriterQueueDepthExceeded {
            current_depth: 150,
            threshold: 100,
        })
    );
}

#[test]
fn check_admission_returns_error_just_over_threshold() {
    // Just over threshold (101 > 100)
    let state = WritePressureState {
        writer_queue_depth: 101,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let result = check_admission(&state);
    assert_eq!(
        result,
        Err(AdmissionError::WriterQueueDepthExceeded {
            current_depth: 101,
            threshold: 100,
        })
    );
}

// ── check_admission: Batch Commit Latency Exceeded ─────────────────────────

#[test]
fn check_admission_returns_batch_commit_latency_exceeded_when_over_threshold() {
    let state = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 1500,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let result = check_admission(&state);
    assert_eq!(
        result,
        Err(AdmissionError::BatchCommitLatencyExceeded {
            current_latency_ms: 1500,
            threshold_ms: 1000,
        })
    );
}

#[test]
fn check_admission_returns_batch_commit_latency_exceeded_just_over_threshold() {
    // Just over threshold (1001 > 1000)
    let state = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 1001,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let result = check_admission(&state);
    assert_eq!(
        result,
        Err(AdmissionError::BatchCommitLatencyExceeded {
            current_latency_ms: 1001,
            threshold_ms: 1000,
        })
    );
}

#[test]
fn check_admission_returns_ok_at_exact_latency_threshold() {
    // Latency exactly at threshold (1000 == 1000)
    let state = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 1000,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let result = check_admission(&state);
    assert_eq!(result, Ok(()));
}

// ── check_admission: Blob Queue Depth Exceeded ─────────────────────────────

#[test]
fn check_admission_returns_blob_queue_depth_exceeded_when_over_threshold() {
    let state = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 100,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let result = check_admission(&state);
    assert_eq!(
        result,
        Err(AdmissionError::BlobQueueDepthExceeded {
            current_depth: 100,
            threshold: 50,
        })
    );
}

#[test]
fn check_admission_returns_blob_queue_depth_exceeded_just_over_threshold() {
    // Just over threshold (51 > 50)
    let state = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 51,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let result = check_admission(&state);
    assert_eq!(
        result,
        Err(AdmissionError::BlobQueueDepthExceeded {
            current_depth: 51,
            threshold: 50,
        })
    );
}

#[test]
fn check_admission_returns_ok_at_exact_blob_threshold() {
    // Blob depth exactly at threshold (50 == 50)
    let state = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 50,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let result = check_admission(&state);
    assert_eq!(result, Ok(()));
}

// ── check_admission: Stall Indicators ──────────────────────────────────────

#[test]
fn check_admission_returns_compaction_stall_active_when_indicator_true() {
    let state = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: true,
        storage_stall_active: false,
    };
    let result = check_admission(&state);
    assert_eq!(result, Err(AdmissionError::CompactionStallActive));
}

#[test]
fn check_admission_returns_ok_when_compaction_stall_false() {
    let state = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let result = check_admission(&state);
    assert_eq!(result, Ok(()));
}

#[test]
fn check_admission_returns_storage_stall_active_when_indicator_true() {
    let state = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: true,
    };
    let result = check_admission(&state);
    assert_eq!(result, Err(AdmissionError::StorageStallActive));
}

#[test]
fn check_admission_returns_ok_when_storage_stall_false() {
    let state = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let result = check_admission(&state);
    assert_eq!(result, Ok(()));
}

// ── check_admission: Multiple Pressure Indicators ───────────────────────────

#[test]
fn check_admission_returns_multiple_pressure_indicators_when_multiple_exceeded() {
    let state = WritePressureState {
        writer_queue_depth: 150,       // over threshold (100)
        batch_commit_latency_ms: 1500, // over threshold (1000)
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let result = check_admission(&state);
    match result {
        Err(AdmissionError::MultiplePressureIndicators { indicators }) => {
            assert!(indicators.contains(&PressureIndicator::WriterQueueDepth));
            assert!(indicators.contains(&PressureIndicator::BatchCommitLatency));
            assert_eq!(indicators.len(), 2);
        }
        other => panic!("Expected MultiplePressureIndicators, got {:?}", other),
    }
}

#[test]
fn check_admission_collects_all_failed_indicators_not_just_first() {
    let state = WritePressureState {
        writer_queue_depth: 150,       // over threshold
        batch_commit_latency_ms: 1500, // also over threshold
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let result = check_admission(&state);
    match result {
        Err(AdmissionError::MultiplePressureIndicators { indicators }) => {
            assert!(indicators.contains(&PressureIndicator::WriterQueueDepth));
            assert!(indicators.contains(&PressureIndicator::BatchCommitLatency));
        }
        _ => panic!("Expected MultiplePressureIndicators, got {:?}", result),
    }
}

#[test]
fn check_admission_returns_multiple_pressure_indicators_all_five_exceeded() {
    let state = WritePressureState {
        writer_queue_depth: 150,
        batch_commit_latency_ms: 1500,
        blob_queue_depth: 100,
        compaction_stall_active: true,
        storage_stall_active: true,
    };
    let result = check_admission(&state);
    match result {
        Err(AdmissionError::MultiplePressureIndicators { indicators }) => {
            assert!(indicators.contains(&PressureIndicator::WriterQueueDepth));
            assert!(indicators.contains(&PressureIndicator::BatchCommitLatency));
            assert!(indicators.contains(&PressureIndicator::BlobQueueDepth));
            assert!(indicators.contains(&PressureIndicator::CompactionStall));
            assert!(indicators.contains(&PressureIndicator::StorageStall));
            assert_eq!(indicators.len(), 5);
        }
        _ => panic!("Expected MultiplePressureIndicators, got {:?}", result),
    }
}

// ── check_admission: u64::MAX Edge Cases ───────────────────────────────────

#[test]
fn check_admission_rejects_max_u64_values() {
    let state = WritePressureState {
        writer_queue_depth: u64::MAX,
        batch_commit_latency_ms: u64::MAX,
        blob_queue_depth: u64::MAX,
        compaction_stall_active: true,
        storage_stall_active: true,
    };
    let result = check_admission(&state);
    match result {
        Err(AdmissionError::MultiplePressureIndicators { indicators }) => {
            assert!(indicators.contains(&PressureIndicator::WriterQueueDepth));
            assert!(indicators.contains(&PressureIndicator::BatchCommitLatency));
            assert!(indicators.contains(&PressureIndicator::BlobQueueDepth));
            assert!(indicators.contains(&PressureIndicator::CompactionStall));
            assert!(indicators.contains(&PressureIndicator::StorageStall));
        }
        other => panic!("Expected MultiplePressureIndicators, got {:?}", other),
    }
}

// ── check_admission_with_thresholds Tests ──────────────────────────────────

#[test]
fn check_admission_with_custom_thresholds_returns_ok_when_within_custom_limits() {
    let state = WritePressureState {
        writer_queue_depth: 200,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: 250,
        batch_commit_latency_ms_threshold: 1000,
        blob_queue_depth_threshold: 50,
max_queued_memory_bytes: 512 * 1024 * 1024,
    };
    let result = check_admission_with_thresholds(&state, &thresholds);
    assert_eq!(result, Ok(()));
}

#[test]
fn check_admission_with_custom_thresholds_returns_error_when_exceeded() {
    let state = WritePressureState {
        writer_queue_depth: 300,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: 250,
        batch_commit_latency_ms_threshold: 1000,
        blob_queue_depth_threshold: 50,
max_queued_memory_bytes: 512 * 1024 * 1024,
    };
    let result = check_admission_with_thresholds(&state, &thresholds);
    assert_eq!(
        result,
        Err(AdmissionError::WriterQueueDepthExceeded {
            current_depth: 300,
            threshold: 250,
        })
    );
}

#[test]
fn check_admission_with_custom_thresholds_rejects_over_blob_threshold() {
    let state = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 100,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: 100,
        batch_commit_latency_ms_threshold: 1000,
        blob_queue_depth_threshold: 50,
max_queued_memory_bytes: 512 * 1024 * 1024,
    };
    let result = check_admission_with_thresholds(&state, &thresholds);
    assert_eq!(
        result,
        Err(AdmissionError::BlobQueueDepthExceeded {
            current_depth: 100,
            threshold: 50,
        })
    );
}

#[test]
fn check_admission_with_zero_thresholds_accepts_zero_values() {
    let state = WritePressureState {
        writer_queue_depth: 0,
        batch_commit_latency_ms: 0,
        blob_queue_depth: 0,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: 0,
        batch_commit_latency_ms_threshold: 0,
        blob_queue_depth_threshold: 0,
max_queued_memory_bytes: 512 * 1024 * 1024,
    };
    let result = check_admission_with_thresholds(&state, &thresholds);
    assert_eq!(result, Ok(()));
}

#[test]
fn check_admission_with_zero_thresholds_rejects_nonzero_values() {
    let state = WritePressureState {
        writer_queue_depth: 1,
        batch_commit_latency_ms: 0,
        blob_queue_depth: 0,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: 0,
        batch_commit_latency_ms_threshold: 0,
        blob_queue_depth_threshold: 0,
max_queued_memory_bytes: 512 * 1024 * 1024,
    };
    let result = check_admission_with_thresholds(&state, &thresholds);
    assert_eq!(
        result,
        Err(AdmissionError::WriterQueueDepthExceeded {
            current_depth: 1,
            threshold: 0,
        })
    );
}

// ── Threshold Swap Mutation Detection ─────────────────────────────────────

#[test]
fn check_admission_detects_threshold_swap_mutation() {
    // State: writer=150 (threshold=100), blob=50 (threshold=75)
    // If comparisons were swapped: 150 > 75 (blob threshold) would trigger blob error
    // Correct behavior: 150 > 100 (writer threshold) triggers writer error
    let state = WritePressureState {
        writer_queue_depth: 150,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 50,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: 100,
        batch_commit_latency_ms_threshold: 1000,
        blob_queue_depth_threshold: 75,
max_queued_memory_bytes: 512 * 1024 * 1024,
    };
    let result = check_admission_with_thresholds(&state, &thresholds);
    // MUST be WriterQueueDepthExceeded, NOT BlobQueueDepthExceeded
    assert_eq!(
        result,
        Err(AdmissionError::WriterQueueDepthExceeded {
            current_depth: 150,
            threshold: 100,
        })
    );
}

#[test]
fn check_admission_with_thresholds_detects_blob_threshold_swap() {
    // State: writer=50 (threshold=100), blob=150 (threshold=75)
    // If comparisons were swapped: 50 > 75 would be false, but 150 > 100 would trigger writer error
    // Correct behavior: 150 > 75 triggers blob error
    let state = WritePressureState {
        writer_queue_depth: 50,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 150,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: 100,
        batch_commit_latency_ms_threshold: 1000,
        blob_queue_depth_threshold: 75,
max_queued_memory_bytes: 512 * 1024 * 1024,
    };
    let result = check_admission_with_thresholds(&state, &thresholds);
    // MUST be BlobQueueDepthExceeded, NOT WriterQueueDepthExceeded
    assert_eq!(
        result,
        Err(AdmissionError::BlobQueueDepthExceeded {
            current_depth: 150,
            threshold: 75,
        })
    );
}

// ── Boundary Condition Tests ───────────────────────────────────────────────

#[test]
fn check_admission_boundary_writer_just_below_threshold() {
    let state = WritePressureState {
        writer_queue_depth: 99, // just below threshold (100)
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let result = check_admission(&state);
    assert_eq!(result, Ok(()));
}

#[test]
fn check_admission_boundary_writer_exactly_at_threshold() {
    let state = WritePressureState {
        writer_queue_depth: 100, // exactly at threshold
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let result = check_admission(&state);
    assert_eq!(result, Ok(()));
}

#[test]
fn check_admission_boundary_writer_just_above_threshold() {
    let state = WritePressureState {
        writer_queue_depth: 101, // just above threshold
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let result = check_admission(&state);
    assert_eq!(
        result,
        Err(AdmissionError::WriterQueueDepthExceeded {
            current_depth: 101,
            threshold: 100,
        })
    );
}

// ── Idempotency Tests ──────────────────────────────────────────────────────

#[test]
fn check_admission_is_idempotent() {
    let state = WritePressureState {
        writer_queue_depth: 50,
        batch_commit_latency_ms: 500,
        blob_queue_depth: 25,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let result1 = check_admission(&state);
    let result2 = check_admission(&state);
    assert_eq!(result1, result2);
}

#[test]
fn check_admission_with_thresholds_is_idempotent() {
    let state = WritePressureState {
        writer_queue_depth: 150,
        batch_commit_latency_ms: 500,
        blob_queue_depth: 25,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: 100,
        batch_commit_latency_ms_threshold: 1000,
        blob_queue_depth_threshold: 50,
max_queued_memory_bytes: 512 * 1024 * 1024,
    };
    let result1 = check_admission_with_thresholds(&state, &thresholds);
    let result2 = check_admission_with_thresholds(&state, &thresholds);
    assert_eq!(result1, result2);
}
