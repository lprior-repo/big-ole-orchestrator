//! Admission and WritePressureMetrics integration tests.

use std::sync::Arc;
use std::thread;

use vo_core::admission::metrics::{BoolGauge, Gauge, WritePressureMetrics};
use vo_core::admission::types::WritePressureState;

#[test]
fn write_pressure_metrics_gauge_thread_safety() {
    let gauge = Arc::new(Gauge::new());
    let gauge_clone = gauge.clone();

    let handle = thread::spawn(move || {
        gauge_clone.set(42);
    });

    handle.join().expect("thread should complete");

    assert_eq!(
        gauge.get(),
        42,
        "gauge should reflect value set by other thread"
    );
}

#[test]
fn write_pressure_metrics_bool_gauge_thread_safety() {
    let bool_gauge = Arc::new(BoolGauge::new());
    let bool_gauge_clone = bool_gauge.clone();

    let handle = thread::spawn(move || {
        bool_gauge_clone.set(true);
    });

    handle.join().expect("thread should complete");

    assert!(
        bool_gauge.get(),
        "bool gauge should reflect value set by other thread"
    );
}

#[test]
fn write_pressure_metrics_update_from_admission_state() {
    let metrics = WritePressureMetrics::new();

    let high_pressure_state = WritePressureState {
        writer_queue_depth: 1000,
        batch_commit_latency_ms: 5000,
        blob_queue_depth: 500,
        compaction_stall_active: true,
        storage_stall_active: true,
    };

    metrics.update_from_state(&high_pressure_state);

    assert_eq!(
        metrics.writer_queue_depth.get(),
        1000,
        "writer queue depth should reflect high pressure"
    );
    assert_eq!(
        metrics.batch_commit_latency_ms.get(),
        5000,
        "batch commit latency should reflect high pressure"
    );
    assert_eq!(
        metrics.blob_queue_depth.get(),
        500,
        "blob queue depth should reflect high pressure"
    );
    assert!(
        metrics.compaction_stall_active.get(),
        "compaction stall should be active"
    );
    assert!(
        metrics.storage_stall_active.get(),
        "storage stall should be active"
    );
}

#[test]
fn write_pressure_metrics_zero_state() {
    let metrics = WritePressureMetrics::new();

    let zero_state = WritePressureState::default();
    metrics.update_from_state(&zero_state);

    assert_eq!(
        metrics.writer_queue_depth.get(),
        0,
        "writer queue depth should be zero"
    );
    assert_eq!(
        metrics.batch_commit_latency_ms.get(),
        0,
        "batch commit latency should be zero"
    );
    assert_eq!(
        metrics.blob_queue_depth.get(),
        0,
        "blob queue depth should be zero"
    );
    assert!(
        !metrics.compaction_stall_active.get(),
        "compaction stall should be inactive"
    );
    assert!(
        !metrics.storage_stall_active.get(),
        "storage stall should be inactive"
    );
}

#[test]
fn write_pressure_metrics_reported_values_match_state() {
    let metrics = WritePressureMetrics::new();

    let test_cases = [
        WritePressureState {
            writer_queue_depth: 0,
            batch_commit_latency_ms: 0,
            blob_queue_depth: 0,
            compaction_stall_active: false,
            storage_stall_active: false,
        },
        WritePressureState {
            writer_queue_depth: 50,
            batch_commit_latency_ms: 100,
            blob_queue_depth: 25,
            compaction_stall_active: false,
            storage_stall_active: false,
        },
        WritePressureState {
            writer_queue_depth: u64::MAX,
            batch_commit_latency_ms: u64::MAX,
            blob_queue_depth: u64::MAX,
            compaction_stall_active: true,
            storage_stall_active: true,
        },
    ];

    for state in test_cases {
        metrics.update_from_state(&state);

        assert_eq!(
            metrics.writer_queue_depth.get(),
            state.writer_queue_depth,
            "gauge should match state"
        );
        assert_eq!(
            metrics.batch_commit_latency_ms.get(),
            state.batch_commit_latency_ms,
            "gauge should match state"
        );
        assert_eq!(
            metrics.blob_queue_depth.get(),
            state.blob_queue_depth,
            "gauge should match state"
        );
        assert_eq!(
            metrics.compaction_stall_active.get(),
            state.compaction_stall_active,
            "gauge should match state"
        );
        assert_eq!(
            metrics.storage_stall_active.get(),
            state.storage_stall_active,
            "gauge should match state"
        );
    }
}
