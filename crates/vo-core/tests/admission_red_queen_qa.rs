//! Red Queen adversarial tests for admission control data integrity.
//!
//! These tests apply evolutionary fuzzing and adversarial input patterns
//! to uncover data integrity bugs in the admission module.
//!
//! # Red Queen Philosophy
//!
//! Named after the Red Queen hypothesis (Van Valen), these tests run
//! many randomized operations to find bugs that only appear in
//! specific combinations of inputs. We "run as fast as we can just
//! to stay in place."

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use proptest::prelude::*;

use vo_core::admission::{
    check_admission, check_admission_with_thresholds, AdmissionError, AdmissionThresholds,
    PressureIndicator, WritePressureState,
};

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

#[derive(Debug, Clone)]
struct ThresholdSwapCase {
    state: WritePressureState,
    thresholds: AdmissionThresholds,
    expected_error: PressureIndicator,
}

fn arb_threshold_swap_case() -> impl Strategy<Value = ThresholdSwapCase> {
    (
        any::<u64>(),
        any::<u64>(),
        any::<u64>(),
        any::<bool>(),
        any::<bool>(),
        any::<u64>(),
        any::<u64>(),
        any::<u64>(),
    )
        .prop_filter(
            "One indicator must exceed threshold while others do not",
            |(writer, batch, blob, _compaction, _storage, w_thresh, b_thresh, blob_thresh)| {
                let writer_over = *writer > *w_thresh;
                let batch_over = *batch > *b_thresh;
                let blob_over = *blob > *blob_thresh;

                [writer_over, batch_over, blob_over]
                    .iter()
                    .filter(|&&x| x)
                    .count()
                    == 1
            },
        )
        .prop_map(
            |(
                writer_queue_depth,
                batch_commit_latency_ms,
                blob_queue_depth,
                compaction_stall_active,
                storage_stall_active,
                writer_threshold,
                batch_threshold,
                blob_threshold,
            )| {
                let writer_over = writer_queue_depth > writer_threshold;
                let batch_over = batch_commit_latency_ms > batch_threshold;
                let blob_over = blob_queue_depth > blob_threshold;

                let expected = if writer_over {
                    PressureIndicator::WriterQueueDepth
                } else if batch_over {
                    PressureIndicator::BatchCommitLatency
                } else {
                    PressureIndicator::BlobQueueDepth
                };

                ThresholdSwapCase {
                    state: WritePressureState {
                        writer_queue_depth,
                        batch_commit_latency_ms,
                        blob_queue_depth,
                        compaction_stall_active,
                        storage_stall_active,
                    },
                    thresholds: AdmissionThresholds {
                        writer_queue_depth_threshold: writer_threshold,
                        batch_commit_latency_ms_threshold: batch_threshold,
                        blob_queue_depth_threshold: blob_threshold,
                    },
                    expected_error: expected,
                }
            },
        )
}

proptest! {
    #[test]
    fn red_queen_threshold_swap_attack(case in arb_threshold_swap_case()) {
        let result = check_admission_with_thresholds(&case.state, &case.thresholds);

        match result {
            Err(AdmissionError::MultiplePressureIndicators { indicators }) => {
                prop_assert!(
                    indicators.contains(&case.expected_error),
                    "Expected {:?} in {:?}",
                    case.expected_error,
                    indicators
                );
            }
            Err(AdmissionError::WriterQueueDepthExceeded { .. }) => {
                prop_assert_eq!(
                    case.expected_error,
                    PressureIndicator::WriterQueueDepth,
                    "Expected WriterQueueDepthExceeded"
                );
            }
            Err(AdmissionError::BatchCommitLatencyExceeded { .. }) => {
                prop_assert_eq!(
                    case.expected_error,
                    PressureIndicator::BatchCommitLatency,
                    "Expected BatchCommitLatencyExceeded"
                );
            }
            Err(AdmissionError::BlobQueueDepthExceeded { .. }) => {
                prop_assert_eq!(
                    case.expected_error,
                    PressureIndicator::BlobQueueDepth,
                    "Expected BlobQueueDepthExceeded"
                );
            }
            Ok(()) => {
                prop_assert!(false, "Expected rejection but got Ok");
            }
            Err(_) => {
                prop_assert!(false, "Unexpected error variant");
            }
        }
    }
}

proptest! {
    #[test]
    fn red_queen_error_values_match_input(
        writer_depth: u64,
        latency: u64,
        blob_depth: u64,
        w_thresh: u64,
        b_thresh: u64,
        blob_thresh: u64,
    ) {
        let state = WritePressureState {
            writer_queue_depth: writer_depth,
            batch_commit_latency_ms: latency,
            blob_queue_depth: blob_depth,
            compaction_stall_active: false,
            storage_stall_active: false,
        };
        let thresholds = AdmissionThresholds {
            writer_queue_depth_threshold: w_thresh,
            batch_commit_latency_ms_threshold: b_thresh,
            blob_queue_depth_threshold: blob_thresh,
        };

        let result = check_admission_with_thresholds(&state, &thresholds);

        let writer_over = writer_depth > w_thresh;
        let latency_over = latency > b_thresh;
        let blob_over = blob_depth > blob_thresh;

        let over_count = [writer_over, latency_over, blob_over]
            .iter()
            .filter(|&&x| x)
            .count();

        if over_count == 1 {
            if writer_over {
                match result {
                    Err(AdmissionError::WriterQueueDepthExceeded { current_depth, threshold }) => {
                        prop_assert_eq!(current_depth, writer_depth);
                        prop_assert_eq!(threshold, w_thresh);
                    }
                    _ => prop_assert!(false, "Expected WriterQueueDepthExceeded"),
                }
            }

            if latency_over {
                match result {
                    Err(AdmissionError::BatchCommitLatencyExceeded { current_latency_ms, threshold_ms }) => {
                        prop_assert_eq!(current_latency_ms, latency);
                        prop_assert_eq!(threshold_ms, b_thresh);
                    }
                    _ => prop_assert!(false, "Expected BatchCommitLatencyExceeded"),
                }
            }

            if blob_over {
                match result {
                    Err(AdmissionError::BlobQueueDepthExceeded { current_depth, threshold }) => {
                        prop_assert_eq!(current_depth, blob_depth);
                        prop_assert_eq!(threshold, blob_thresh);
                    }
                    _ => prop_assert!(false, "Expected BlobQueueDepthExceeded"),
                }
            }
        }
    }
}

proptest! {
    #[test]
    fn red_queen_no_false_positives_on_stall_flags(
        writer_depth: u64,
        latency: u64,
        blob_depth: u64,
        w_thresh: u64,
        b_thresh: u64,
        blob_thresh: u64,
    ) {
        let state = WritePressureState {
            writer_queue_depth: writer_depth,
            batch_commit_latency_ms: latency,
            blob_queue_depth: blob_depth,
            compaction_stall_active: true,
            storage_stall_active: false,
        };
        let thresholds = AdmissionThresholds {
            writer_queue_depth_threshold: w_thresh,
            batch_commit_latency_ms_threshold: b_thresh,
            blob_queue_depth_threshold: blob_thresh,
        };

        let result = check_admission_with_thresholds(&state, &thresholds);

        match result {
            Err(AdmissionError::CompactionStallActive) => {}
            Err(AdmissionError::MultiplePressureIndicators { indicators }) => {
                prop_assert!(
                    indicators.contains(&PressureIndicator::CompactionStall),
                    "CompactionStall must be in indicators when flag is set"
                );
            }
            _ => prop_assert!(false, "CompactionStallActive must be returned when flag is true"),
        }
    }
}

proptest! {
    #[test]
    fn red_queen_storage_stall_takes_priority(
        writer_depth: u64,
        latency: u64,
        blob_depth: u64,
    ) {
        let state = WritePressureState {
            writer_queue_depth: writer_depth,
            batch_commit_latency_ms: latency,
            blob_queue_depth: blob_depth,
            compaction_stall_active: false,
            storage_stall_active: true,
        };
        let thresholds = AdmissionThresholds::default();

        let result = check_admission_with_thresholds(&state, &thresholds);

        match result {
            Err(AdmissionError::StorageStallActive) => {}
            Err(AdmissionError::MultiplePressureIndicators { indicators }) => {
                prop_assert!(
                    indicators.contains(&PressureIndicator::StorageStall),
                    "StorageStall must be in indicators when flag is set"
                );
            }
            _ => prop_assert!(false, "StorageStallActive must be returned when flag is true"),
        }
    }
}

proptest! {
    #[test]
    fn red_queen_multiple_indicators_no_duplicates(state in arb_write_pressure_state()) {
        let thresholds = AdmissionThresholds::default();
        let result = check_admission_with_thresholds(&state, &thresholds);

        if let Err(AdmissionError::MultiplePressureIndicators { indicators }) = result {
            let mut unique = indicators.clone();
            unique.sort();
            unique.dedup();
            prop_assert_eq!(
                indicators.len(),
                unique.len(),
                "MultiplePressureIndicators must not contain duplicates: {:?}",
                indicators
            );
        }
    }
}

proptest! {
    #[test]
    fn red_queen_deterministic_results(state in arb_write_pressure_state()) {
        let thresholds = AdmissionThresholds::default();

        let results: Vec<_> = (0..100)
            .map(|_| check_admission_with_thresholds(&state, &thresholds))
            .collect();

        let first = &results[0];
        for (i, r) in results.iter().enumerate().skip(1) {
            prop_assert_eq!(
                r, first,
                "check_admission_with_thresholds must be deterministic. First result: {:?}, result {}: {:?}",
                first, i, r
            );
        }
    }
}

#[test]
fn red_queen_at_max_u64_no_panic() {
    let state = WritePressureState {
        writer_queue_depth: u64::MAX,
        batch_commit_latency_ms: u64::MAX,
        blob_queue_depth: u64::MAX,
        compaction_stall_active: true,
        storage_stall_active: true,
    };

    let result = std::panic::catch_unwind(|| check_admission(&state));

    assert!(
        result.is_ok(),
        "check_admission must not panic on max u64 values"
    );
}

#[test]
fn red_queen_boundary_u64_max_minus_one() {
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: u64::MAX,
        batch_commit_latency_ms_threshold: u64::MAX,
        blob_queue_depth_threshold: u64::MAX,
    };

    let state = WritePressureState {
        writer_queue_depth: u64::MAX - 1,
        batch_commit_latency_ms: u64::MAX - 1,
        blob_queue_depth: u64::MAX - 1,
        compaction_stall_active: false,
        storage_stall_active: false,
    };

    let result = check_admission_with_thresholds(&state, &thresholds);
    assert_eq!(
        result,
        Ok(()),
        "u64::MAX - 1 should be below u64::MAX threshold"
    );
}

#[test]
fn red_queen_serialization_integrity() {
    let state = WritePressureState {
        writer_queue_depth: 42,
        batch_commit_latency_ms: 142,
        blob_queue_depth: 242,
        compaction_stall_active: true,
        storage_stall_active: false,
    };

    let json = serde_json::to_string(&state).unwrap();
    let deserialized: WritePressureState = serde_json::from_str(&json).unwrap();

    assert_eq!(
        state, deserialized,
        "Serialization must preserve all fields"
    );
}

#[test]
fn red_queen_thresholds_serialization_integrity() {
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: 100,
        batch_commit_latency_ms_threshold: 1000,
        blob_queue_depth_threshold: 50,
    };

    let json = serde_json::to_string(&thresholds).unwrap();
    let deserialized: AdmissionThresholds = serde_json::from_str(&json).unwrap();

    assert_eq!(
        thresholds, deserialized,
        "Serialization must preserve all threshold values"
    );
}

#[test]
fn red_queen_error_values_match_for_writer_queue_exceeded() {
    let error = AdmissionError::WriterQueueDepthExceeded {
        current_depth: 150,
        threshold: 100,
    };

    match error {
        AdmissionError::WriterQueueDepthExceeded {
            current_depth,
            threshold,
        } => {
            assert_eq!(current_depth, 150);
            assert_eq!(threshold, 100);
        }
        _ => panic!("Expected WriterQueueDepthExceeded"),
    }
}

#[test]
fn red_queen_error_values_match_for_batch_latency_exceeded() {
    let error = AdmissionError::BatchCommitLatencyExceeded {
        current_latency_ms: 1500,
        threshold_ms: 1000,
    };

    match error {
        AdmissionError::BatchCommitLatencyExceeded {
            current_latency_ms,
            threshold_ms,
        } => {
            assert_eq!(current_latency_ms, 1500);
            assert_eq!(threshold_ms, 1000);
        }
        _ => panic!("Expected BatchCommitLatencyExceeded"),
    }
}

#[test]
fn red_queen_error_values_match_for_blob_depth_exceeded() {
    let error = AdmissionError::BlobQueueDepthExceeded {
        current_depth: 75,
        threshold: 50,
    };

    match error {
        AdmissionError::BlobQueueDepthExceeded {
            current_depth,
            threshold,
        } => {
            assert_eq!(current_depth, 75);
            assert_eq!(threshold, 50);
        }
        _ => panic!("Expected BlobQueueDepthExceeded"),
    }
}

#[test]
fn red_queen_concurrent_reads_deterministic() {
    use std::thread;

    let state = WritePressureState {
        writer_queue_depth: 50,
        batch_commit_latency_ms: 500,
        blob_queue_depth: 25,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let thresholds = AdmissionThresholds::default();

    let results: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));
    let mut handles = vec![];

    for _ in 0..100 {
        let results_clone = Arc::clone(&results);
        let state_clone = state.clone();
        let thresholds_clone = thresholds.clone();
        let handle = thread::spawn(move || {
            let result = check_admission_with_thresholds(&state_clone, &thresholds_clone);
            if result.is_ok() {
                results_clone.fetch_add(1, Ordering::Relaxed);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let ok_count = results.load(Ordering::Relaxed);
    assert_eq!(
        ok_count, 100,
        "All 100 concurrent reads must return Ok (deterministic)"
    );
}

proptest! {
    #[test]
    fn red_queen_tighter_thresholds_never_looser(
        state in arb_write_pressure_state(),
        thresholds_a in arb_admission_thresholds(),
        thresholds_b in arb_admission_thresholds(),
    ) {
        let a_tighter = thresholds_a.writer_queue_depth_threshold <= thresholds_b.writer_queue_depth_threshold
            && thresholds_a.batch_commit_latency_ms_threshold <= thresholds_b.batch_commit_latency_ms_threshold
            && thresholds_a.blob_queue_depth_threshold <= thresholds_b.blob_queue_depth_threshold;

        if !a_tighter {
            return Ok(());
        }

        let result_a = check_admission_with_thresholds(&state, &thresholds_a);
        let result_b = check_admission_with_thresholds(&state, &thresholds_b);

        if matches!(result_a, Ok(())) && matches!(result_b, Err(_)) {
            prop_assert!(false, "State passed tighter thresholds but failed looser: tighter={:?} looser={:?}", result_a, result_b);
        }
    }
}

proptest! {
    #[test]
    fn red_queen_all_combinations_of_stall_flags(
        writer_depth: u64,
        latency: u64,
        blob_depth: u64,
    ) {
        for compaction in [false, true] {
            for storage in [false, true] {
                let state = WritePressureState {
                    writer_queue_depth: writer_depth,
                    batch_commit_latency_ms: latency,
                    blob_queue_depth: blob_depth,
                    compaction_stall_active: compaction,
                    storage_stall_active: storage,
                };

                let result = check_admission(&state);

                if compaction && storage {
                    match result {
                        Err(AdmissionError::MultiplePressureIndicators { indicators }) => {
                            prop_assert!(indicators.contains(&PressureIndicator::CompactionStall));
                            prop_assert!(indicators.contains(&PressureIndicator::StorageStall));
                        }
                        _ => prop_assert!(false, "Both stalls must produce MultiplePressureIndicators"),
                    }
                } else if compaction {
                    match result {
                        Err(AdmissionError::CompactionStallActive) => {}
                        Err(AdmissionError::MultiplePressureIndicators { indicators }) => {
                            prop_assert!(indicators.contains(&PressureIndicator::CompactionStall));
                        }
                        _ => prop_assert!(false, "CompactionStall must trigger when active"),
                    }
                } else if storage {
                    match result {
                        Err(AdmissionError::StorageStallActive) => {}
                        Err(AdmissionError::MultiplePressureIndicators { indicators }) => {
                            prop_assert!(indicators.contains(&PressureIndicator::StorageStall));
                        }
                        _ => prop_assert!(false, "StorageStall must trigger when active"),
                    }
                }
            }
        }
    }
}

#[test]
fn red_queen_zero_thresholds_exact_boundary() {
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: 0,
        batch_commit_latency_ms_threshold: 0,
        blob_queue_depth_threshold: 0,
    };

    let state_zero = WritePressureState {
        writer_queue_depth: 0,
        batch_commit_latency_ms: 0,
        blob_queue_depth: 0,
        compaction_stall_active: false,
        storage_stall_active: false,
    };

    let result = check_admission_with_thresholds(&state_zero, &thresholds);
    assert_eq!(result, Ok(()), "Zero values at zero threshold must pass");

    let state_one = WritePressureState {
        writer_queue_depth: 1,
        batch_commit_latency_ms: 0,
        blob_queue_depth: 0,
        compaction_stall_active: false,
        storage_stall_active: false,
    };

    let result = check_admission_with_thresholds(&state_one, &thresholds);
    assert!(result.is_err(), "Value 1 at threshold 0 must be rejected");
}

#[test]
fn red_queen_cross_contamination_check() {
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: 100,
        batch_commit_latency_ms_threshold: 1000,
        blob_queue_depth_threshold: 50,
    };

    let state = WritePressureState {
        writer_queue_depth: 150,
        batch_commit_latency_ms: 500,
        blob_queue_depth: 25,
        compaction_stall_active: false,
        storage_stall_active: false,
    };

    let result = check_admission_with_thresholds(&state, &thresholds);

    match result {
        Err(AdmissionError::WriterQueueDepthExceeded {
            current_depth,
            threshold,
        }) => {
            assert_eq!(current_depth, 150);
            assert_eq!(threshold, 100);
        }
        _ => panic!("Expected WriterQueueDepthExceeded with correct values"),
    }
}

proptest! {
    #[test]
    fn red_queen_fuzz_state_values(
        w1: u64,
        w2: u64,
        b1: u64,
        b2: u64,
        blob1: u64,
        blob2: u64,
    ) {
        let state1 = WritePressureState {
            writer_queue_depth: w1,
            batch_commit_latency_ms: b1,
            blob_queue_depth: blob1,
            compaction_stall_active: false,
            storage_stall_active: false,
        };

        let state2 = WritePressureState {
            writer_queue_depth: w2,
            batch_commit_latency_ms: b2,
            blob_queue_depth: blob2,
            compaction_stall_active: false,
            storage_stall_active: false,
        };

        let thresholds = AdmissionThresholds::default();

        let r1 = check_admission_with_thresholds(&state1, &thresholds);
        let r2 = check_admission_with_thresholds(&state2, &thresholds);

        if w1 <= thresholds.writer_queue_depth_threshold && w2 > thresholds.writer_queue_depth_threshold {
            prop_assert!(r1.is_ok() && r2.is_err(), "w1={} passes, w2={} fails writer threshold", w1, w2);
        }
    }
}
