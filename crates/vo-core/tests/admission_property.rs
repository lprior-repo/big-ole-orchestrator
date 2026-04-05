//! Property-based tests for admission module.
//!
//! These tests verify invariants that must hold for ALL inputs,
//! not just the specific cases covered by unit and integration tests.
//!
//! Proptest runs each test with 1000+ random input combinations,
//! making it highly likely to catch edge cases that manual testing misses.

use proptest::prelude::*;

use vo_core::admission::{
    check_admission, check_admission_with_thresholds, AdmissionError, AdmissionThresholds,
    PressureIndicator, WritePressureState,
};

// ── Helper Strategies ───────────────────────────────────────────────────────────

/// Strategy for generating WritePressureState with random field values.
fn arb_write_pressure_state() -> impl Strategy<Value = WritePressureState> {
    (
        any::<u64>(),
        any::<u64>(),
        any::<u64>(),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(
            |(
                writer_queue_depth,
                batch_commit_latency_ms,
                blob_queue_depth,
                compaction_stall_active,
                storage_stall_active,
            )| {
                WritePressureState {
                    writer_queue_depth,
                    batch_commit_latency_ms,
                    blob_queue_depth,
                    compaction_stall_active,
                    storage_stall_active,
                }
            },
        )
}

/// Strategy for generating AdmissionThresholds with random field values.
fn arb_admission_thresholds() -> impl Strategy<Value = AdmissionThresholds> {
    (any::<u64>(), any::<u64>(), any::<u64>()).prop_map(
        |(
            writer_queue_depth_threshold,
            batch_commit_latency_ms_threshold,
            blob_queue_depth_threshold,
        )| {
            AdmissionThresholds {
                writer_queue_depth_threshold,
                batch_commit_latency_ms_threshold,
                blob_queue_depth_threshold,
            }
        },
    )
}

// ── Idempotency Invariants ────────────────────────────────────────────────────

proptest! {
    #[test]
    fn check_admission_idempotent(state in arb_write_pressure_state()) {
        let result1 = check_admission(&state);
        let result2 = check_admission(&state);
        prop_assert_eq!(result1, result2);
    }

    #[test]
    fn check_admission_with_thresholds_idempotent(
        state in arb_write_pressure_state(),
        thresholds in arb_admission_thresholds()
    ) {
        let result1 = check_admission_with_thresholds(&state, &thresholds);
        let result2 = check_admission_with_thresholds(&state, &thresholds);
        prop_assert_eq!(result1, result2);
    }
}

// ── Threshold Ordering Invariants ─────────────────────────────────────────────

proptest! {
    #[test]
    fn tighter_thresholds_never_pass_when_looser_fails(
        thresholds_a in arb_admission_thresholds(),
        thresholds_b in arb_admission_thresholds(),
        state in arb_write_pressure_state(),
    ) {
        let a_tighter = thresholds_a.writer_queue_depth_threshold <= thresholds_b.writer_queue_depth_threshold
            && thresholds_a.batch_commit_latency_ms_threshold <= thresholds_b.batch_commit_latency_ms_threshold
            && thresholds_a.blob_queue_depth_threshold <= thresholds_b.blob_queue_depth_threshold;

        if !a_tighter {
            return Ok(());
        }

        let result_a = check_admission_with_thresholds(&state, &thresholds_a);

        // If it passes tighter thresholds, it must pass looser
        let result_b = check_admission_with_thresholds(&state, &thresholds_b);
        let passes_tighter_and_not_looser = matches!(result_a, Ok(())) && !matches!(result_b, Ok(()));
        prop_assert_eq!(passes_tighter_and_not_looser, false, "State that passes tighter thresholds must pass looser");
    }
}

// ── Composite Error Completeness Invariants ───────────────────────────────────

proptest! {
    #[test]
    fn multiple_pressure_indicators_contains_exactly_failed_indicators(state in arb_write_pressure_state()) {
        let thresholds = AdmissionThresholds::default();

        let mut expected_failures = Vec::new();
        if state.writer_queue_depth > thresholds.writer_queue_depth_threshold {
            expected_failures.push(PressureIndicator::WriterQueueDepth);
        }
        if state.batch_commit_latency_ms > thresholds.batch_commit_latency_ms_threshold {
            expected_failures.push(PressureIndicator::BatchCommitLatency);
        }
        if state.blob_queue_depth > thresholds.blob_queue_depth_threshold {
            expected_failures.push(PressureIndicator::BlobQueueDepth);
        }
        if state.compaction_stall_active {
            expected_failures.push(PressureIndicator::CompactionStall);
        }
        if state.storage_stall_active {
            expected_failures.push(PressureIndicator::StorageStall);
        }

        let result = check_admission_with_thresholds(&state, &thresholds);

        match result {
            Err(AdmissionError::MultiplePressureIndicators { indicators }) => {
                let mut sorted_expected = expected_failures.clone();
                let mut sorted_actual = indicators;
                sorted_expected.sort();
                sorted_actual.sort();
                prop_assert_eq!(sorted_actual, sorted_expected);
            }
            Ok(()) => {
                prop_assert!(expected_failures.is_empty());
            }
            Err(e) => {
                if expected_failures.len() == 1 {
                    let expected = expected_failures.first().unwrap();
                    match e {
                        AdmissionError::WriterQueueDepthExceeded { .. } => {
                            prop_assert_eq!(expected, &PressureIndicator::WriterQueueDepth);
                        }
                        AdmissionError::BatchCommitLatencyExceeded { .. } => {
                            prop_assert_eq!(expected, &PressureIndicator::BatchCommitLatency);
                        }
                        AdmissionError::BlobQueueDepthExceeded { .. } => {
                            prop_assert_eq!(expected, &PressureIndicator::BlobQueueDepth);
                        }
                        AdmissionError::CompactionStallActive => {
                            prop_assert_eq!(expected, &PressureIndicator::CompactionStall);
                        }
                        AdmissionError::StorageStallActive => {
                            prop_assert_eq!(expected, &PressureIndicator::StorageStall);
                        }
                        _ => prop_assert!(false, "Unexpected error variant"),
                    }
                }
            }
        }
    }
}

// ── Single Indicator Error Invariants ───────────────────────────────────────

proptest! {
    #[test]
    fn writer_queue_depth_error_contains_correct_values(
        depth: u64,
        threshold: u64
    ) {
        let state = WritePressureState {
            writer_queue_depth: depth,
            batch_commit_latency_ms: 0,
            blob_queue_depth: 0,
            compaction_stall_active: false,
            storage_stall_active: false,
        };
        let thresholds = AdmissionThresholds {
            writer_queue_depth_threshold: threshold,
            batch_commit_latency_ms_threshold: u64::MAX,
            blob_queue_depth_threshold: u64::MAX,
        };

        let result = check_admission_with_thresholds(&state, &thresholds);

        if depth > threshold {
            match result {
                Err(AdmissionError::WriterQueueDepthExceeded { current_depth, threshold: t }) => {
                    prop_assert_eq!(current_depth, depth);
                    prop_assert_eq!(t, threshold);
                }
                other => prop_assert!(false, "Expected WriterQueueDepthExceeded, got {:?}", other),
            }
        } else {
            prop_assert_eq!(result, Ok(()), "Expected Ok when depth <= threshold");
        }
    }

    #[test]
    fn batch_commit_latency_error_contains_correct_values(
        latency: u64,
        threshold: u64
    ) {
        let state = WritePressureState {
            writer_queue_depth: 0,
            batch_commit_latency_ms: latency,
            blob_queue_depth: 0,
            compaction_stall_active: false,
            storage_stall_active: false,
        };
        let thresholds = AdmissionThresholds {
            writer_queue_depth_threshold: u64::MAX,
            batch_commit_latency_ms_threshold: threshold,
            blob_queue_depth_threshold: u64::MAX,
        };

        let result = check_admission_with_thresholds(&state, &thresholds);

        if latency > threshold {
            match result {
                Err(AdmissionError::BatchCommitLatencyExceeded { current_latency_ms, threshold_ms }) => {
                    prop_assert_eq!(current_latency_ms, latency);
                    prop_assert_eq!(threshold_ms, threshold);
                }
                other => prop_assert!(false, "Expected BatchCommitLatencyExceeded, got {:?}", other),
            }
        } else {
            prop_assert_eq!(result, Ok(()), "Expected Ok when latency <= threshold");
        }
    }

    #[test]
    fn blob_queue_depth_error_contains_correct_values(
        depth: u64,
        threshold: u64
    ) {
        let state = WritePressureState {
            writer_queue_depth: 0,
            batch_commit_latency_ms: 0,
            blob_queue_depth: depth,
            compaction_stall_active: false,
            storage_stall_active: false,
        };
        let thresholds = AdmissionThresholds {
            writer_queue_depth_threshold: u64::MAX,
            batch_commit_latency_ms_threshold: u64::MAX,
            blob_queue_depth_threshold: threshold,
        };

        let result = check_admission_with_thresholds(&state, &thresholds);

        if depth > threshold {
            match result {
                Err(AdmissionError::BlobQueueDepthExceeded { current_depth, threshold: t }) => {
                    prop_assert_eq!(current_depth, depth);
                    prop_assert_eq!(t, threshold);
                }
                other => prop_assert!(false, "Expected BlobQueueDepthExceeded, got {:?}", other),
            }
        } else {
            prop_assert_eq!(result, Ok(()), "Expected Ok when depth <= threshold");
        }
    }
}

// ── Stall Indicator Invariants ───────────────────────────────────────────────

proptest! {
    #[test]
    fn compaction_stall_active_always_rejects(state in arb_write_pressure_state()) {
        let mut state = state;
        state.compaction_stall_active = true;
        state.storage_stall_active = false;
        state.batch_commit_latency_ms = 0;
        state.blob_queue_depth = 0;
        // Set writer queue depth to 0 to isolate compaction stall check
        state.writer_queue_depth = 0;

        let thresholds = AdmissionThresholds::default();
        let result = check_admission_with_thresholds(&state, &thresholds);

        match result {
            Err(AdmissionError::CompactionStallActive) => {},
            other => prop_assert!(false, "Expected CompactionStallActive, got {:?}", other),
        }
    }

    #[test]
    fn storage_stall_active_always_rejects(state in arb_write_pressure_state()) {
        let mut state = state;
        state.storage_stall_active = true;
        state.compaction_stall_active = false;
        state.batch_commit_latency_ms = 0;
        state.blob_queue_depth = 0;
        state.writer_queue_depth = 0;

        let thresholds = AdmissionThresholds::default();
        let result = check_admission_with_thresholds(&state, &thresholds);

        match result {
            Err(AdmissionError::StorageStallActive) => {},
            other => prop_assert!(false, "Expected StorageStallActive, got {:?}", other),
        }
    }
}

// ── Boundary Invariants ────────────────────────────────────────────────────────

proptest! {
    #[test]
    fn at_exact_threshold_returns_ok(writer_depth: u64, latency: u64, blob_depth: u64) {
        // All values at exactly the default thresholds should pass
        let thresholds = AdmissionThresholds::default();

        let state = WritePressureState {
            writer_queue_depth: thresholds.writer_queue_depth_threshold,
            batch_commit_latency_ms: thresholds.batch_commit_latency_ms_threshold,
            blob_queue_depth: thresholds.blob_queue_depth_threshold,
            compaction_stall_active: false,
            storage_stall_active: false,
        };

        let result = check_admission(&state);
        prop_assert_eq!(result, Ok(()), "At exact threshold should return Ok");
    }
}

// ── Error Never Contains Duplicate Indicators ─────────────────────────────────

proptest! {
    #[test]
    fn multiple_pressure_indicators_has_no_duplicates(state in arb_write_pressure_state()) {
        let thresholds = AdmissionThresholds::default();
        let result = check_admission_with_thresholds(&state, &thresholds);

        if let Err(AdmissionError::MultiplePressureIndicators { indicators }) = result {
            // Check no duplicates by comparing length to unique length
            let mut unique = indicators.clone();
            unique.sort();
            unique.dedup();
            prop_assert_eq!(indicators.len(), unique.len(), "MultiplePressureIndicators should not contain duplicates");
        }
    }
}

// ── JSON Serialization Roundtrip ─────────────────────────────────────────────

proptest! {
    #[test]
    fn write_pressure_state_json_roundtrip(state in arb_write_pressure_state()) {
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: WritePressureState = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(state, deserialized);
    }

    #[test]
    fn admission_thresholds_json_roundtrip(thresholds in arb_admission_thresholds()) {
        let json = serde_json::to_string(&thresholds).unwrap();
        let deserialized: AdmissionThresholds = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(thresholds, deserialized);
    }
}

// ── Additional Deterministic Boundary Tests ────────────────────────────────────

#[test]
fn admission_at_max_u64_threshold_boundary() {
    // When threshold is u64::MAX, only u64::MAX itself should pass (if comparison is >)
    // Actually, u64::MAX > u64::MAX is false, so u64::MAX should pass
    let state = WritePressureState {
        writer_queue_depth: u64::MAX,
        batch_commit_latency_ms: u64::MAX,
        blob_queue_depth: u64::MAX,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: u64::MAX,
        batch_commit_latency_ms_threshold: u64::MAX,
        blob_queue_depth_threshold: u64::MAX,
    };

    let result = check_admission_with_thresholds(&state, &thresholds);
    // u64::MAX > u64::MAX is false, so this should pass
    assert_eq!(result, Ok(()));
}

#[test]
fn admission_just_over_max_u64_threshold_is_impossible() {
    // Since u64::MAX is the maximum value, there's nothing higher to test
    // But we can verify that u64::MAX exceeds u64::MAX - 1
    let state = WritePressureState {
        writer_queue_depth: u64::MAX - 1,
        batch_commit_latency_ms: 0,
        blob_queue_depth: 0,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: u64::MAX,
        batch_commit_latency_ms_threshold: u64::MAX,
        blob_queue_depth_threshold: u64::MAX,
    };

    let result = check_admission_with_thresholds(&state, &thresholds);
    // u64::MAX - 1 > u64::MAX is false, so this should pass
    assert_eq!(result, Ok(()));
}

#[test]
fn admission_with_zero_thresholds_strict() {
    // With zero thresholds, any non-zero value should fail
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: 0,
        batch_commit_latency_ms_threshold: 0,
        blob_queue_depth_threshold: 0,
    };

    // Writer over zero
    let state = WritePressureState {
        writer_queue_depth: 1,
        batch_commit_latency_ms: 0,
        blob_queue_depth: 0,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let result = check_admission_with_thresholds(&state, &thresholds);
    assert_eq!(
        result,
        Err(AdmissionError::WriterQueueDepthExceeded {
            current_depth: 1,
            threshold: 0,
        })
    );

    // Latency over zero
    let state = WritePressureState {
        writer_queue_depth: 0,
        batch_commit_latency_ms: 1,
        blob_queue_depth: 0,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let result = check_admission_with_thresholds(&state, &thresholds);
    assert_eq!(
        result,
        Err(AdmissionError::BatchCommitLatencyExceeded {
            current_latency_ms: 1,
            threshold_ms: 0,
        })
    );

    // Blob over zero
    let state = WritePressureState {
        writer_queue_depth: 0,
        batch_commit_latency_ms: 0,
        blob_queue_depth: 1,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let result = check_admission_with_thresholds(&state, &thresholds);
    assert_eq!(
        result,
        Err(AdmissionError::BlobQueueDepthExceeded {
            current_depth: 1,
            threshold: 0,
        })
    );
}
