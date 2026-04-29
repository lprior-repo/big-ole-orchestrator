//! Integration tests for admission module.
//!
//! Tests the admission module behavior through the public API,
//! verifying correct state transitions and error handling.
//!
//! These tests exercise the full composed behavior with real types
//! and verify the admission logic works correctly end-to-end.

use vo_core::admission::{
    check_admission, check_admission_with_thresholds, AdmissionError, AdmissionThresholds,
    PressureIndicator, WritePressureState,
};

// ── Integration: Full Admission Workflow ─────────────────────────────────────

#[test]
fn admission_grants_access_when_all_pressure_indicators_within_default_thresholds() {
    // Given: A WritePressureState with all values well below default thresholds
    let state = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };

    // When: check_admission is called
    let result = check_admission(&state);

    // Then: Admission is granted
    assert_eq!(result, Ok(()));
}

#[test]
fn admission_rejects_when_writer_queue_depth_exceeds_default_threshold() {
    // Given: A WritePressureState with writer queue depth exceeding default threshold
    let state = WritePressureState {
        writer_queue_depth: 150, // Default threshold is 100
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };

    // When: check_admission is called
    let result = check_admission(&state);

    // Then: Admission is rejected with specific error
    assert_eq!(
        result,
        Err(AdmissionError::WriterQueueDepthExceeded {
            current_depth: 150,
            threshold: 100,
        })
    );
}

#[test]
fn admission_rejects_when_batch_commit_latency_exceeds_default_threshold() {
    // Given: A WritePressureState with latency exceeding default threshold
    let state = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 1500, // Default threshold is 1000
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };

    // When: check_admission is called
    let result = check_admission(&state);

    // Then: Admission is rejected with specific error
    assert_eq!(
        result,
        Err(AdmissionError::BatchCommitLatencyExceeded {
            current_latency_ms: 1500,
            threshold_ms: 1000,
        })
    );
}

#[test]
fn admission_rejects_when_blob_queue_depth_exceeds_default_threshold() {
    // Given: A WritePressureState with blob queue depth exceeding default threshold
    let state = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 100, // Default threshold is 50
        compaction_stall_active: false,
        storage_stall_active: false,
    };

    // When: check_admission is called
    let result = check_admission(&state);

    // Then: Admission is rejected with specific error
    assert_eq!(
        result,
        Err(AdmissionError::BlobQueueDepthExceeded {
            current_depth: 100,
            threshold: 50,
        })
    );
}

#[test]
fn admission_rejects_when_compaction_stall_is_active() {
    // Given: A WritePressureState with compaction stall active
    let state = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: true,
        storage_stall_active: false,
    };

    // When: check_admission is called
    let result = check_admission(&state);

    // Then: Admission is rejected
    assert_eq!(result, Err(AdmissionError::CompactionStallActive));
}

#[test]
fn admission_rejects_when_storage_stall_is_active() {
    // Given: A WritePressureState with storage stall active
    let state = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: true,
    };

    // When: check_admission is called
    let result = check_admission(&state);

    // Then: Admission is rejected
    assert_eq!(result, Err(AdmissionError::StorageStallActive));
}

// ── Integration: Multiple Pressure Indicators ───────────────────────────────────

#[test]
fn admission_rejects_with_multiple_pressure_indicators_when_two_indicators_exceed() {
    // Given: A WritePressureState with two indicators exceeding thresholds
    let state = WritePressureState {
        writer_queue_depth: 150,       // over threshold
        batch_commit_latency_ms: 1500, // over threshold
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };

    // When: check_admission is called
    let result = check_admission(&state);

    // Then: Composite error with both failing indicators
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
fn admission_rejects_with_multiple_pressure_indicators_when_all_five_exceed() {
    // Given: A WritePressureState with all five indicators in failure state
    let state = WritePressureState {
        writer_queue_depth: u64::MAX,
        batch_commit_latency_ms: u64::MAX,
        blob_queue_depth: u64::MAX,
        compaction_stall_active: true,
        storage_stall_active: true,
    };

    // When: check_admission is called
    let result = check_admission(&state);

    // Then: Composite error with all five failing indicators
    match result {
        Err(AdmissionError::MultiplePressureIndicators { indicators }) => {
            assert!(indicators.contains(&PressureIndicator::WriterQueueDepth));
            assert!(indicators.contains(&PressureIndicator::BatchCommitLatency));
            assert!(indicators.contains(&PressureIndicator::BlobQueueDepth));
            assert!(indicators.contains(&PressureIndicator::CompactionStall));
            assert!(indicators.contains(&PressureIndicator::StorageStall));
            assert_eq!(indicators.len(), 5);
        }
        other => panic!("Expected MultiplePressureIndicators, got {:?}", other),
    }
}

// ── Integration: Custom Thresholds ────────────────────────────────────────────

#[test]
fn admission_with_custom_thresholds_accepts_higher_limits() {
    // Given: A WritePressureState that exceeds default thresholds
    // But custom thresholds are high enough to allow it
    let state = WritePressureState {
        writer_queue_depth: 200, // exceeds default (100) but below custom (250)
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: 250,
        batch_commit_latency_ms_threshold: 1000,
        blob_queue_depth_threshold: 50,
    };

    // When: check_admission_with_thresholds is called
    let result = check_admission_with_thresholds(&state, &thresholds);

    // Then: Admission is granted
    assert_eq!(result, Ok(()));
}

#[test]
fn admission_with_custom_thresholds_rejects_when_custom_threshold_exceeded() {
    // Given: A WritePressureState that exceeds custom thresholds
    let state = WritePressureState {
        writer_queue_depth: 300, // exceeds custom threshold (250)
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: 250,
        batch_commit_latency_ms_threshold: 1000,
        blob_queue_depth_threshold: 50,
    };

    // When: check_admission_with_thresholds is called
    let result = check_admission_with_thresholds(&state, &thresholds);

    // Then: Admission is rejected with custom threshold reflected in error
    assert_eq!(
        result,
        Err(AdmissionError::WriterQueueDepthExceeded {
            current_depth: 300,
            threshold: 250,
        })
    );
}

#[test]
fn admission_with_custom_thresholds_rejects_blob_when_exceeds_custom_threshold() {
    // Given: A WritePressureState with blob depth exceeding custom threshold
    let state = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 100, // exceeds custom threshold (50)
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: 100,
        batch_commit_latency_ms_threshold: 1000,
        blob_queue_depth_threshold: 50,
    };

    // When: check_admission_with_thresholds is called
    let result = check_admission_with_thresholds(&state, &thresholds);

    // Then: Admission is rejected
    assert_eq!(
        result,
        Err(AdmissionError::BlobQueueDepthExceeded {
            current_depth: 100,
            threshold: 50,
        })
    );
}

// ── Integration: Zero Thresholds ──────────────────────────────────────────────

#[test]
fn admission_with_zero_thresholds_accepts_zero_values() {
    // Given: A WritePressureState with all zero values and zero thresholds
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
    };

    // When: check_admission_with_thresholds is called
    let result = check_admission_with_thresholds(&state, &thresholds);

    // Then: Admission is granted (zero is not greater than zero)
    assert_eq!(result, Ok(()));
}

#[test]
fn admission_with_zero_thresholds_rejects_any_nonzero_value() {
    // Given: A WritePressureState with nonzero writer queue depth and zero thresholds
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
    };

    // When: check_admission_with_thresholds is called
    let result = check_admission_with_thresholds(&state, &thresholds);

    // Then: Admission is rejected
    assert_eq!(
        result,
        Err(AdmissionError::WriterQueueDepthExceeded {
            current_depth: 1,
            threshold: 0,
        })
    );
}

// ── Integration: Threshold Swap Detection ─────────────────────────────────────

#[test]
fn admission_with_thresholds_detects_writer_blob_threshold_swap() {
    // State: writer=150 (threshold=100), blob=50 (threshold=75)
    // If comparisons swapped: 150 > 75 would trigger blob error (wrong!)
    // Correct: 150 > 100 triggers writer error
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
    };

    // When: check_admission_with_thresholds is called
    let result = check_admission_with_thresholds(&state, &thresholds);

    // Then: Must be WriterQueueDepthExceeded, NOT BlobQueueDepthExceeded
    assert_eq!(
        result,
        Err(AdmissionError::WriterQueueDepthExceeded {
            current_depth: 150,
            threshold: 100,
        })
    );
}

#[test]
fn admission_with_thresholds_detects_blob_writer_threshold_swap() {
    // State: writer=50 (threshold=100), blob=150 (threshold=75)
    // If comparisons swapped: 50 > 75 would be false, but 150 > 100 would trigger writer error (wrong!)
    // Correct: 150 > 75 triggers blob error
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
    };

    // When: check_admission_with_thresholds is called
    let result = check_admission_with_thresholds(&state, &thresholds);

    // Then: Must be BlobQueueDepthExceeded, NOT WriterQueueDepthExceeded
    assert_eq!(
        result,
        Err(AdmissionError::BlobQueueDepthExceeded {
            current_depth: 150,
            threshold: 75,
        })
    );
}

// ── Integration: Boundary Conditions ─────────────────────────────────────────

#[test]
fn admission_accepts_at_exact_threshold_boundary() {
    // Given: Values exactly at threshold
    let state = WritePressureState {
        writer_queue_depth: 100, // exactly at default threshold
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };

    // When: check_admission is called
    let result = check_admission(&state);

    // Then: Admission is granted (depth == threshold means within limits)
    assert_eq!(result, Ok(()));
}

#[test]
fn admission_rejects_just_over_threshold_boundary() {
    // Given: Values just over threshold
    let state = WritePressureState {
        writer_queue_depth: 101, // just over default threshold (100)
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };

    // When: check_admission is called
    let result = check_admission(&state);

    // Then: Admission is rejected
    assert_eq!(
        result,
        Err(AdmissionError::WriterQueueDepthExceeded {
            current_depth: 101,
            threshold: 100,
        })
    );
}

#[test]
fn admission_accepts_just_below_threshold_boundary() {
    // Given: Values just below threshold
    let state = WritePressureState {
        writer_queue_depth: 99, // just below default threshold (100)
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };

    // When: check_admission is called
    let result = check_admission(&state);

    // Then: Admission is granted
    assert_eq!(result, Ok(()));
}

// ── Integration: Idempotency ───────────────────────────────────────────────────

#[test]
fn check_admission_is_idempotent_on_repeated_calls() {
    // Given: A valid state
    let state = WritePressureState {
        writer_queue_depth: 50,
        batch_commit_latency_ms: 500,
        blob_queue_depth: 25,
        compaction_stall_active: false,
        storage_stall_active: false,
    };

    // When: check_admission is called multiple times
    let result1 = check_admission(&state);
    let result2 = check_admission(&state);
    let result3 = check_admission(&state);

    // Then: All results are identical
    assert_eq!(result1, result2);
    assert_eq!(result2, result3);
}

#[test]
fn check_admission_with_thresholds_is_idempotent_on_repeated_calls() {
    // Given: A valid state and thresholds
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
    };

    // When: check_admission_with_thresholds is called multiple times
    let result1 = check_admission_with_thresholds(&state, &thresholds);
    let result2 = check_admission_with_thresholds(&state, &thresholds);
    let result3 = check_admission_with_thresholds(&state, &thresholds);

    // Then: All results are identical
    assert_eq!(result1, result2);
    assert_eq!(result2, result3);
}
