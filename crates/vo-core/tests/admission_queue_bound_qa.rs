//! QA: Admission control queue bound verification (ve-thc00)

#![allow(clippy::unwrap_used)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use vo_core::admission::{
    check_admission_with_thresholds, AdmissionThresholds, AdmissionError, WritePressureState,
};

fn healthy_state(depth: u64) -> WritePressureState {
    WritePressureState {
        writer_queue_depth: depth,
        batch_commit_latency_ms: 0,
        blob_queue_depth: 0,
        compaction_stall_active: false,
        storage_stall_active: false,
    }
}

fn thresholds(max_depth: u64) -> AdmissionThresholds {
    AdmissionThresholds {
        writer_queue_depth_threshold: max_depth,
        batch_commit_latency_ms_threshold: 1000,
        blob_queue_depth_threshold: 50,
    }
}

#[test]
fn queue_bound_admits_at_exact_threshold() {
    // depth == threshold is allowed (not exceeded)
    let thresholds = thresholds(10);
    let state = healthy_state(10);
    assert!(
        check_admission_with_thresholds(&state, &thresholds).is_ok(),
        "depth == threshold should be admitted"
    );
}

#[test]
fn queue_bound_rejects_above_threshold() {
    let thresholds = thresholds(10);
    let state = healthy_state(11);
    assert!(check_admission_with_thresholds(&state, &thresholds).is_err());
    assert!(matches!(
        check_admission_with_thresholds(&state, &thresholds),
        Err(AdmissionError::WriterQueueDepthExceeded {
            current_depth: 11,
            threshold: 10,
        })
    ));
}

#[test]
fn queue_bound_admits_zero_depth_at_zero_threshold() {
    let thresholds = thresholds(0);
    let state = healthy_state(0);
    assert!(
        check_admission_with_thresholds(&state, &thresholds).is_ok(),
        "depth 0 at threshold 0 should be admitted"
    );
}

#[test]
fn queue_bound_concurrent_reads_never_admit_above_threshold() {
    use std::thread;

    let threshold: u64 = 10;
    let over_threshold_count = Arc::new(AtomicU64::new(0));
    let threads: Vec<_> = (0..8)
        .map(|i| {
            let over = Arc::clone(&over_threshold_count);
            thread::spawn(move || {
                let t = thresholds(threshold);
                for step in 0..100u64 {
                    let depth = i as u64 * 100 + step;
                    let state = healthy_state(depth);
                    let result = check_admission_with_thresholds(&state, &t);
                    if depth <= threshold {
                        assert!(result.is_ok(), "depth {depth} <= threshold {threshold} should admit");
                    } else {
                        assert!(result.is_err(), "depth {depth} > threshold {threshold} should reject");
                        over.fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
        })
        .collect();

    for t in threads {
        t.join().unwrap();
    }
    let total_over = over_threshold_count.load(Ordering::Relaxed);
    // Thread 0: depths 11..99 = 89 over. Threads 1-7: 100 each = 700. Total = 789.
    assert_eq!(total_over, 789, "every check above threshold must be rejected");
}
