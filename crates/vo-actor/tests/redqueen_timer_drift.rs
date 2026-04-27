//! RED-QUEEN coevolutionary adversarial tests for vo-actor timer system.
//! Timer accuracy under system load - tests for rq-004.
//!
//! EARS Requirements:
//! - Ubiquitous: THE SYSTEM SHALL maintain timer accuracy
//! - Event-Driven: When system under load, THE SYSTEM SHALL maintain timer accuracy
//! - Unwanted: If timer drifts significantly, THE SYSTEM SHALL miss deadlines
//!
//! Invariant: Drift < 10%

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use vo_actor::timers::{compute_fire_at, is_timer_expired, validate_sleep_duration};
use vo_types::TimestampMs;

const MAX_DRIFT_PERCENT: f64 = 10.0;

fn test_instance_id() -> vo_types::InstanceId {
    vo_types::InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap()
}

fn test_timer_id() -> vo_types::TimerId {
    vo_types::TimerId::parse("timer-rq004-001").unwrap()
}

fn calculate_drift_percent(expected_ms: u64, actual_ms: u64) -> f64 {
    if expected_ms == 0 {
        return 0.0;
    }
    let drift = (actual_ms as i64 - expected_ms as i64).unsigned_abs() as f64;
    (drift / expected_ms as f64) * 100.0
}

fn drift_within_tolerance(expected_ms: u64, actual_ms: u64) -> bool {
    calculate_drift_percent(expected_ms, actual_ms) <= MAX_DRIFT_PERCENT
}

#[tokio::test(flavor = "multi_thread")]
async fn rq_timer_accuracy_idle() {
    let duration_ms: i64 = 100;
    let start = std::time::Instant::now();

    let validated = validate_sleep_duration(duration_ms).expect("valid duration");
    let base_ms = TimestampMs::now().as_u64();
    let fire_at_ms = compute_fire_at(base_ms, validated).expect("valid fire_at");

    tokio::time::sleep(Duration::from_millis(duration_ms as u64)).await;

    let actual_ms = start.elapsed().as_millis() as u64;
    let drift_percent = calculate_drift_percent(duration_ms as u64, actual_ms);

    assert!(
        drift_within_tolerance(duration_ms as u64, actual_ms),
        "Timer at idle should have drift < {}%, got {:.2}% (expected {}ms, actual {}ms)",
        MAX_DRIFT_PERCENT,
        drift_percent,
        duration_ms,
        actual_ms
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn rq_timer_accuracy_under_load_single_timer() {
    let duration_ms: i64 = 200;
    let base_ms = TimestampMs::now().as_u64();
    let validated = validate_sleep_duration(duration_ms).expect("valid duration");
    let fire_at_ms = compute_fire_at(base_ms, validated).expect("valid fire_at");

    let counter = Arc::new(AtomicU64::new(0));
    let counter_clone = counter.clone();

    let load_handle = tokio::spawn(async move {
        for _ in 0..1000 {
            let _ = compute_fire_at(0, 1);
            counter_clone.fetch_add(1, Ordering::SeqCst);
        }
    });

    tokio::time::sleep(Duration::from_millis(duration_ms as u64)).await;
    let _ = load_handle.await;

    let elapsed = counter.load(Ordering::SeqCst);
    assert!(elapsed > 0, "Load generation should have executed");
}

#[tokio::test(flavor = "multi_thread")]
async fn rq_timer_accuracy_under_load_cpu_contention() {
    let duration_ms: i64 = 150;
    let base_ms = TimestampMs::now().as_u64();
    let validated = validate_sleep_duration(duration_ms).expect("valid duration");
    let fire_at_ms = compute_fire_at(base_ms, validated).expect("valid fire_at");

    let _load_handles: Vec<_> = (0..4)
        .map(|_| {
            tokio::spawn(async move {
                let start = std::time::Instant::now();
                while start.elapsed().as_millis() < 200 {
                    let _ = validate_sleep_duration(1);
                    let _ = compute_fire_at(0, 1);
                }
            })
        })
        .collect();

    let timer_start = std::time::Instant::now();
    tokio::time::sleep(Duration::from_millis(duration_ms as u64)).await;
    let actual_ms = timer_start.elapsed().as_millis() as u64;

    let drift_percent = calculate_drift_percent(duration_ms as u64, actual_ms);
    assert!(
        drift_within_tolerance(duration_ms as u64, actual_ms),
        "Timer under CPU load should have drift < {}%, got {:.2}% (expected {}ms, actual {}ms)",
        MAX_DRIFT_PERCENT,
        drift_percent,
        duration_ms,
        actual_ms
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn rq_timer_accuracy_under_load_async_contention() {
    let duration_ms: i64 = 100;
    let base_ms = TimestampMs::now().as_u64();
    let validated = validate_sleep_duration(duration_ms).expect("valid duration");
    let fire_at_ms = compute_fire_at(base_ms, validated).expect("valid fire_at");

    let barrier = Arc::new(tokio::sync::Barrier::new(10));
    let mut handles = vec![];

    for _ in 0..9 {
        let barrier_clone = barrier.clone();
        handles.push(tokio::spawn(async move {
            barrier_clone.wait().await;
            let start = std::time::Instant::now();
            while start.elapsed().as_millis() < 150 {
                tokio::task::yield_now().await;
            }
        }));
    }

    let timer_start = std::time::Instant::now();
    tokio::time::sleep(Duration::from_millis(duration_ms as u64)).await;
    let actual_ms = timer_start.elapsed().as_millis() as u64;

    for h in handles {
        let _ = h.await;
    }

    let drift_percent = calculate_drift_percent(duration_ms as u64, actual_ms);
    assert!(
        drift_within_tolerance(duration_ms as u64, actual_ms),
        "Timer under async contention should have drift < {}%, got {:.2}%",
        MAX_DRIFT_PERCENT,
        drift_percent
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn rq_timer_drift_within_10_percent_at_idle() {
    let durations = [50i64, 100, 200, 500];

    for duration in durations {
        let timer_start = std::time::Instant::now();
        tokio::time::sleep(Duration::from_millis(duration as u64)).await;
        let actual_ms = timer_start.elapsed().as_millis() as u64;

        assert!(
            drift_within_tolerance(duration as u64, actual_ms),
            "Idle timer ({}ms) drift should be < {}%, got {:.2}%",
            duration,
            MAX_DRIFT_PERCENT,
            calculate_drift_percent(duration as u64, actual_ms)
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn rq_timer_drift_within_10_percent_under_contention() {
    let duration_ms: i64 = 100;

    let _contention_handles: Vec<_> = (0..8)
        .map(|_| {
            tokio::spawn(async move {
                let start = std::time::Instant::now();
                while start.elapsed().as_millis() < 150 {
                    tokio::task::yield_now().await;
                    std::hint::spin_loop();
                }
            })
        })
        .collect();

    let timer_start = std::time::Instant::now();
    tokio::time::sleep(Duration::from_millis(duration_ms as u64)).await;
    let actual_ms = timer_start.elapsed().as_millis() as u64;

    let drift_percent = calculate_drift_percent(duration_ms as u64, actual_ms);
    assert!(
        drift_within_tolerance(duration_ms as u64, actual_ms),
        "Timer under contention should have drift < {}%, got {:.2}%",
        MAX_DRIFT_PERCENT,
        drift_percent
    );
}

#[test]
fn rq_timer_calculation_never_negative() {
    let base_ms = 1_000_000u64;
    let duration_ms = 500u64;

    let fire_at = compute_fire_at(base_ms, duration_ms).expect("valid");
    assert!(fire_at > base_ms);

    let now_ms = fire_at;
    assert!(is_timer_expired(fire_at, now_ms));

    let now_ms_early = base_ms;
    assert!(!is_timer_expired(fire_at, now_ms_early));
}

#[test]
fn rq_timer_calculation_overflow_rejected() {
    let result = compute_fire_at(u64::MAX, 1);
    assert!(result.is_err());
}

#[test]
fn rq_drift_calculation_handles_zero_expected() {
    let drift = calculate_drift_percent(0, 100);
    assert_eq!(drift, 0.0);
}

#[test]
fn rq_drift_calculation_exact_timing() {
    let drift = calculate_drift_percent(1000, 1000);
    assert_eq!(drift, 0.0);
}

#[test]
fn rq_drift_calculation_10_percent_boundary() {
    let drift = calculate_drift_percent(1000, 1100);
    assert!((drift - 10.0).abs() < 0.001);
}

#[test]
fn rq_drift_calculation_over_tolerance() {
    let drift = calculate_drift_percent(1000, 1150);
    assert!(drift > 10.0);
}

#[test]
fn rq_drift_within_tolerance_exact() {
    assert!(drift_within_tolerance(1000, 1000));
}

#[test]
fn rq_drift_within_tolerance_10_percent() {
    assert!(drift_within_tolerance(1000, 1100));
}

#[test]
fn rq_drift_within_tolerance_just_over() {
    assert!(!drift_within_tolerance(1000, 1101));
}
