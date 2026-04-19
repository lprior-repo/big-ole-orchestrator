//! TDD-RED: Failing tests for recovery queue throttling and orphan detection.
//!
//! Bead: ve-381x8
//! Parent: ve-1jl78 (recovery queue throttling and orphan detection)
//! Test plan: ve-9aonj
//! Test review: ve-3f87m (REJECTED — these tests address findings)
//!
//! # Test Areas
//!
//! 1. Throttle rate limiting (concurrent storm, refill edge, sustained load)
//! 2. Orphan detection after timeout (sweep timeout, boundary, batch limiting)
//! 3. Queue priority ordering (mixed priority/fire time, large queue)
//! 4. Graceful degradation under load (concurrent enqueue, partial rejection)
//! 5. Proptest invariants (queue bounds, token conservation)
//!
//! ALL TESTS MUST FAIL INITIALLY (TDD-RED phase).

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use vo_core::recovery::{
    OrphanDetector, OrphanProcess, OrphanQuery, RecoveryError, RecoveryItem, RecoveryThrottle,
    RecoveryThrottleConfig,
};

fn make_orphan(instance_id: &str) -> OrphanProcess {
    OrphanProcess {
        instance_id: instance_id.to_string(),
        lineage_id: "lineage-1".to_string(),
        failed_at: Duration::from_secs(0),
    }
}

fn make_item(instance_id: &str) -> RecoveryItem {
    RecoveryItem {
        orphan: make_orphan(instance_id),
        enqueued_at: Duration::from_secs(0),
    }
}

// =========================================================================
// SECTION 1: Throttle Rate Limiting
// =========================================================================

#[tokio::test]
async fn throttle_concurrent_storm_all_succeed_until_capacity() {
    let config = RecoveryThrottleConfig {
        capacity: 5,
        refill_rate: 1,
        refill_period: Duration::from_secs(1),
    };
    let mut throttle = RecoveryThrottle::new(config);

    let mut successes = 0usize;
    let mut rejections = 0usize;

    for i in 0..10 {
        let item = make_item(&format!("storm-{i}"));
        match throttle.enqueue(item).await {
            Ok(()) => successes += 1,
            Err(RecoveryError::QueueFull) => rejections += 1,
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    assert_eq!(successes, 5, "exactly capacity items should succeed");
    assert_eq!(rejections, 5, "remaining items should be rejected");
}

#[tokio::test]
async fn throttle_refill_at_exact_boundary() {
    let config = RecoveryThrottleConfig {
        capacity: 1,
        refill_rate: 1,
        refill_period: Duration::from_millis(100),
    };
    let mut throttle = RecoveryThrottle::new(config);

    let item1 = make_item("first");
    let r1 = throttle.enqueue(item1).await;
    assert!(r1.is_ok());

    let item2 = make_item("second");
    let r2 = throttle.enqueue(item2).await;
    let is_queue_full = match r2 {
        Err(RecoveryError::QueueFull) => true,
        _ => false,
    };
    assert!(is_queue_full, "should be full before refill");

    throttle.advance_time(Duration::from_millis(100));

    let item3 = make_item("third");
    let r3 = throttle.enqueue(item3).await;
    assert!(
        r3.is_ok(),
        "at exact refill boundary, token should be available"
    );
}

#[tokio::test]
async fn throttle_sustained_load_drains_faster_than_refill() {
    let config = RecoveryThrottleConfig {
        capacity: 3,
        refill_rate: 1,
        refill_period: Duration::from_millis(50),
    };
    let mut throttle = RecoveryThrottle::new(config);

    for i in 0..3 {
        let item = make_item(&format!("drain-{i}"));
        assert!(throttle.enqueue(item).await.is_ok());
    }

    let overflow = throttle.enqueue(make_item("overflow")).await;
    let is_full = match overflow {
        Err(RecoveryError::QueueFull) => true,
        _ => false,
    };
    assert!(is_full, "queue should be full after initial drain");

    throttle.advance_time(Duration::from_millis(50));
    let refill1 = throttle.enqueue(make_item("refill-1")).await;
    assert!(refill1.is_ok(), "one refill allows one enqueue");

    let overflow2 = throttle.enqueue(make_item("overflow-2")).await;
    let is_full2 = match overflow2 {
        Err(RecoveryError::QueueFull) => true,
        _ => false,
    };
    assert!(is_full2, "queue should be full again after single refill");

    throttle.advance_time(Duration::from_millis(50));
    let refill2 = throttle.enqueue(make_item("refill-2")).await;
    assert!(
        refill2.is_ok(),
        "second refill cycle allows another enqueue"
    );
}

#[tokio::test]
async fn throttle_tokens_never_exceed_max_capacity_after_large_refill() {
    let config = RecoveryThrottleConfig {
        capacity: 3,
        refill_rate: 10,
        refill_period: Duration::from_millis(100),
    };
    let mut throttle = RecoveryThrottle::new(config);

    for i in 0..3 {
        let r = throttle.enqueue(make_item(&format!("cap-{i}"))).await;
        assert!(r.is_ok());
    }
    let over = throttle.enqueue(make_item("over")).await;
    let is_full = match over {
        Err(RecoveryError::QueueFull) => true,
        _ => false,
    };
    assert!(is_full);

    throttle.advance_time(Duration::from_secs(10));

    assert_eq!(
        throttle.available_capacity(),
        3,
        "available tokens must not exceed max capacity even after large time advance"
    );

    for i in 0..3 {
        let r = throttle
            .enqueue(make_item(&format!("after-refill-{i}")))
            .await;
        assert!(r.is_ok(), "should be able to enqueue up to capacity again");
    }
    let over_again = throttle.enqueue(make_item("over-again")).await;
    let is_full_again = match over_again {
        Err(RecoveryError::QueueFull) => true,
        _ => false,
    };
    assert!(is_full_again);
}

#[tokio::test]
async fn throttle_zero_capacity_is_always_full() {
    let config = RecoveryThrottleConfig {
        capacity: 0,
        refill_rate: 1,
        refill_period: Duration::from_secs(1),
    };
    let mut throttle = RecoveryThrottle::new(config);

    let result = throttle.enqueue(make_item("zero-cap")).await;
    let is_full = match result {
        Err(RecoveryError::QueueFull) => true,
        _ => false,
    };
    assert!(is_full, "zero-capacity throttle must reject all enqueues");
}

#[tokio::test]
async fn throttle_release_slot_allows_reuse() {
    let config = RecoveryThrottleConfig {
        capacity: 1,
        refill_rate: 0,
        refill_period: Duration::from_secs(10),
    };
    let mut throttle = RecoveryThrottle::new(config);

    let r1 = throttle.enqueue(make_item("first")).await;
    assert!(r1.is_ok());

    let r2 = throttle.enqueue(make_item("second")).await;
    let full = match r2 {
        Err(RecoveryError::QueueFull) => true,
        _ => false,
    };
    assert!(full);

    // TDD-RED: release() does not exist yet on RecoveryThrottle
    throttle.release();

    let r3 = throttle.enqueue(make_item("after-release")).await;
    assert!(
        r3.is_ok(),
        "after releasing a slot, enqueue should succeed even without refill"
    );
}

#[tokio::test]
async fn throttle_current_depth_tracking() {
    let config = RecoveryThrottleConfig {
        capacity: 5,
        refill_rate: 0,
        refill_period: Duration::from_secs(10),
    };
    let mut throttle = RecoveryThrottle::new(config);

    assert_eq!(throttle.current_depth(), 0);

    throttle.enqueue(make_item("1")).await.unwrap();
    assert_eq!(throttle.current_depth(), 1);

    throttle.enqueue(make_item("2")).await.unwrap();
    assert_eq!(throttle.current_depth(), 2);

    throttle.release();
    assert_eq!(throttle.current_depth(), 1);
}

#[tokio::test]
async fn throttle_rejection_count_tracking() {
    let config = RecoveryThrottleConfig {
        capacity: 1,
        refill_rate: 0,
        refill_period: Duration::from_secs(10),
    };
    let mut throttle = RecoveryThrottle::new(config);

    assert_eq!(throttle.total_rejections(), 0);

    throttle.enqueue(make_item("only")).await.unwrap();

    let _ = throttle.enqueue(make_item("rej-1")).await;
    let _ = throttle.enqueue(make_item("rej-2")).await;
    let _ = throttle.enqueue(make_item("rej-3")).await;

    assert_eq!(
        throttle.total_rejections(),
        3,
        "should track total rejections across all enqueue attempts"
    );
}

// =========================================================================
// SECTION 2: Orphan Detection After Timeout
// =========================================================================

struct MockOrphanQuery {
    results: Vec<OrphanProcess>,
    query_count: Arc<AtomicUsize>,
    fail_after: Option<usize>,
}

impl MockOrphanQuery {
    fn new(results: Vec<OrphanProcess>) -> Self {
        Self {
            results,
            query_count: Arc::new(AtomicUsize::new(0)),
            fail_after: None,
        }
    }

    fn with_fail_after(results: Vec<OrphanProcess>, fail_after: usize) -> Self {
        Self {
            results,
            query_count: Arc::new(AtomicUsize::new(0)),
            fail_after: Some(fail_after),
        }
    }
}

impl OrphanQuery for MockOrphanQuery {
    async fn query_orphans(&self) -> Result<Vec<OrphanProcess>, String> {
        let count = self.query_count.fetch_add(1, Ordering::SeqCst);
        if let Some(fail_after) = self.fail_after {
            if count >= fail_after {
                return Err("simulated query failure".to_string());
            }
        }
        Ok(self.results.clone())
    }
}

#[tokio::test]
async fn sweep_timeout_returns_error_when_detector_exceeds_deadline() {
    let query = MockOrphanQuery::new(vec![]);
    let detector = OrphanDetector::new(Duration::from_millis(10), query);
    let (tx, _rx) = tokio::sync::mpsc::channel(10);

    let result = detector
        .run_with_timeout(tx, Duration::from_millis(1))
        .await;

    let is_sweep_timeout = match result {
        Err(RecoveryError::SweepTimeout { .. }) => true,
        _ => false,
    };
    assert!(
        is_sweep_timeout,
        "sweep should return SweepTimeout when it exceeds the deadline"
    );
}

#[tokio::test]
async fn sweep_timeout_error_carries_elapsed_duration() {
    let query = MockOrphanQuery::new(vec![]);
    let detector = OrphanDetector::new(Duration::from_millis(10), query);
    let (tx, _rx) = tokio::sync::mpsc::channel(10);

    let timeout = Duration::from_millis(5);
    let result = detector.run_with_timeout(tx, timeout).await;

    match result {
        Err(RecoveryError::SweepTimeout { elapsed }) => {
            assert!(
                elapsed >= timeout,
                "elapsed duration should be >= the timeout value"
            );
        }
        _ => panic!("expected SweepTimeout error with elapsed duration"),
    }
}

#[tokio::test]
async fn sweep_batches_orphans_respecting_max_batch_size() {
    let orphans: Vec<OrphanProcess> = (0..20)
        .map(|i| OrphanProcess {
            instance_id: format!("batch-{i}"),
            lineage_id: "lin".to_string(),
            failed_at: Duration::from_secs(0),
        })
        .collect();

    let query = MockOrphanQuery::new(orphans);
    let detector = OrphanDetector::new(Duration::from_millis(10), query);
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);

    let result = detector.run_with_batch_limit(tx, 5).await;

    assert!(result.is_ok(), "sweep should succeed");

    let mut received = Vec::new();
    while let Ok(item) = rx.try_recv() {
        received.push(item);
    }

    assert_eq!(
        received.len(),
        5,
        "sweep should respect max_batch_size limit of 5, got {}",
        received.len()
    );
}

#[tokio::test]
async fn sweep_tracks_metrics_for_enqueued_and_rejected() {
    let orphans: Vec<OrphanProcess> = (0..10)
        .map(|i| OrphanProcess {
            instance_id: format!("metric-{i}"),
            lineage_id: "lin".to_string(),
            failed_at: Duration::from_secs(0),
        })
        .collect();

    let query = MockOrphanQuery::new(orphans);
    let detector = OrphanDetector::new(Duration::from_millis(10), query);
    let (tx, _rx) = tokio::sync::mpsc::channel(3);

    let metrics = detector.run_and_collect_metrics(tx).await;

    assert_eq!(metrics.detected, 10, "should detect all 10 orphans");
    assert_eq!(metrics.enqueued, 3, "should enqueue up to channel capacity");
    assert_eq!(metrics.rejected, 7, "should reject the rest");
}

#[tokio::test]
async fn sweep_partial_query_failure_does_not_lose_previously_found_orphans() {
    let orphans: Vec<OrphanProcess> = (0..5)
        .map(|i| OrphanProcess {
            instance_id: format!("partial-{i}"),
            lineage_id: "lin".to_string(),
            failed_at: Duration::from_secs(0),
        })
        .collect();

    let query = MockOrphanQuery::with_fail_after(orphans, 1);
    let detector = OrphanDetector::new(Duration::from_millis(10), query);
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);

    let first_result = detector.run_single_sweep(tx.clone()).await;
    assert!(first_result.is_ok(), "first sweep should succeed");

    let second_result = detector.run_single_sweep(tx.clone()).await;
    assert!(
        second_result.is_err(),
        "second sweep should fail (query configured to fail after 1 call)"
    );

    let mut received = Vec::new();
    while let Ok(item) = rx.try_recv() {
        received.push(item);
    }

    assert_eq!(
        received.len(),
        5,
        "orphans from first successful sweep should not be lost after second sweep fails"
    );
}

#[tokio::test]
async fn sweep_returns_empty_results_gracefully() {
    let query = MockOrphanQuery::new(vec![]);
    let detector = OrphanDetector::new(Duration::from_millis(10), query);
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);

    let result = detector.run_single_sweep(tx).await;
    assert!(result.is_ok(), "empty sweep should succeed");

    let mut received = Vec::new();
    while let Ok(item) = rx.try_recv() {
        received.push(item);
    }
    assert!(
        received.is_empty(),
        "no orphans should be sent on empty result"
    );
}

#[tokio::test]
async fn sweep_channel_closed_returns_channel_closed_error() {
    let orphans = vec![make_orphan("orphan-1")];
    let query = MockOrphanQuery::new(orphans);
    let detector = OrphanDetector::new(Duration::from_millis(10), query);
    let (tx, rx) = tokio::sync::mpsc::channel(10);

    drop(rx);

    let result = detector.run_single_sweep(tx).await;
    let is_channel_closed = match result {
        Err(RecoveryError::SweepChannelClosed) => true,
        _ => false,
    };
    assert!(
        is_channel_closed,
        "should return SweepChannelClosed when receiver is dropped"
    );
}

// =========================================================================
// SECTION 3: Graceful Degradation Under Load
// =========================================================================

#[tokio::test]
async fn concurrent_enqueue_contention_with_single_capacity() {
    let config = RecoveryThrottleConfig {
        capacity: 1,
        refill_rate: 0,
        refill_period: Duration::from_secs(10),
    };
    let throttle = Arc::new(tokio::sync::Mutex::new(RecoveryThrottle::new(config)));

    let successes = Arc::new(AtomicUsize::new(0));
    let rejections = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();
    for i in 0..10 {
        let throttle = throttle.clone();
        let successes = successes.clone();
        let rejections = rejections.clone();
        handles.push(tokio::spawn(async move {
            let mut t = throttle.lock().await;
            let item = make_item(&format!("concurrent-{i}"));
            match t.enqueue(item).await {
                Ok(()) => {
                    successes.fetch_add(1, Ordering::SeqCst);
                }
                Err(RecoveryError::QueueFull) => {
                    rejections.fetch_add(1, Ordering::SeqCst);
                }
                Err(e) => panic!("unexpected error: {e}"),
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let s = successes.load(Ordering::SeqCst);
    let r = rejections.load(Ordering::SeqCst);
    assert_eq!(s, 1, "exactly 1 should succeed with capacity=1, got {s}");
    assert_eq!(r, 9, "9 should be rejected, got {r}");
}

#[tokio::test]
async fn graceful_degradation_rejects_cleanly_without_panicking() {
    let config = RecoveryThrottleConfig {
        capacity: 1,
        refill_rate: 0,
        refill_period: Duration::from_secs(10),
    };
    let mut throttle = RecoveryThrottle::new(config);

    throttle.enqueue(make_item("first")).await.unwrap();

    for i in 0..100 {
        let result = throttle.enqueue(make_item(&format!("reject-{i}"))).await;
        match result {
            Err(RecoveryError::QueueFull) => {}
            Err(e) => panic!("unexpected error on rejection {i}: {e}"),
            Ok(()) => panic!("enqueue {i} should have been rejected"),
        }
    }
}

#[tokio::test]
async fn sweep_partial_rejection_records_accurate_metrics() {
    let orphans: Vec<OrphanProcess> = (0..8)
        .map(|i| OrphanProcess {
            instance_id: format!("partial-rej-{i}"),
            lineage_id: "lin".to_string(),
            failed_at: Duration::from_secs(0),
        })
        .collect();

    let query = MockOrphanQuery::new(orphans);
    let detector = OrphanDetector::new(Duration::from_millis(10), query);
    let (tx, _rx) = tokio::sync::mpsc::channel(3);

    let metrics = detector.run_and_collect_metrics(tx).await;

    assert_eq!(metrics.detected, 8);
    assert!(
        metrics.enqueued + metrics.rejected == metrics.detected,
        "enqueued ({}) + rejected ({}) should equal detected ({})",
        metrics.enqueued,
        metrics.rejected,
        metrics.detected
    );
}

// =========================================================================
// SECTION 4: Error Display and Trait Verification
// =========================================================================

#[test]
fn recovery_error_queue_full_display_message() {
    let err = RecoveryError::QueueFull;
    let msg = err.to_string();
    assert!(
        msg.contains("full"),
        "QueueFull display should contain 'full', got: {msg}"
    );
    assert!(
        msg.contains("throttle"),
        "QueueFull display should mention throttle, got: {msg}"
    );
}

#[test]
fn recovery_error_sweep_channel_closed_display_message() {
    let err = RecoveryError::SweepChannelClosed;
    let msg = err.to_string();
    assert!(
        msg.contains("channel") && msg.contains("closed"),
        "SweepChannelClosed display should mention channel and closed, got: {msg}"
    );
}

#[test]
fn recovery_error_sweep_query_failed_display_includes_reason() {
    let err = RecoveryError::SweepQueryFailed("database timeout".to_string());
    let msg = err.to_string();
    assert!(
        msg.contains("database timeout"),
        "SweepQueryFailed should include reason, got: {msg}"
    );
}

#[test]
fn recovery_error_sweep_timeout_display_includes_duration() {
    let err = RecoveryError::SweepTimeout {
        elapsed: Duration::from_secs(30),
    };
    let msg = err.to_string();
    assert!(
        msg.contains("30"),
        "SweepTimeout display should include elapsed duration, got: {msg}"
    );
}

#[test]
fn orphan_process_equality_and_clone() {
    let p1 = OrphanProcess {
        instance_id: "test".to_string(),
        lineage_id: "lin".to_string(),
        failed_at: Duration::from_secs(5),
    };
    let p2 = p1.clone();
    assert_eq!(p1, p2, "clone should produce equal value");
}

#[test]
fn recovery_item_equality_and_clone() {
    let i1 = make_item("eq-test");
    let i2 = i1.clone();
    assert_eq!(i1, i2, "clone should produce equal value");
}

#[test]
fn orphan_detector_implements_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<OrphanDetector<MockOrphanQuery>>();
}

#[test]
fn recovery_throttle_config_builder() {
    let config = RecoveryThrottleConfig::new(42, 3, Duration::from_millis(500));
    assert_eq!(config.capacity, 42);
    assert_eq!(config.refill_rate, 3);
    assert_eq!(config.refill_period, Duration::from_millis(500));
}

// =========================================================================
// SECTION 5: Proptest Invariants
// =========================================================================

mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(500))]

        #[test]
        fn throttle_available_never_exceeds_capacity(
            capacity in 1usize..100,
            refill_rate in 1usize..10,
            refill_period_ms in 10u64..1000,
            time_advance_ms in 0u64..10000,
            enqueues in 0usize..50,
        ) {
            let config = RecoveryThrottleConfig {
                capacity,
                refill_rate,
                refill_period: Duration::from_millis(refill_period_ms),
            };
            let mut throttle = RecoveryThrottle::new(config);

            throttle.advance_time(Duration::from_millis(time_advance_ms));

            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                for i in 0..enqueues {
                    let _ = throttle.enqueue(make_item(&format!("p-{i}"))).await;
                }
            });

            prop_assert!(
                throttle.available_capacity() <= capacity,
                "available ({}) must not exceed capacity ({})",
                throttle.available_capacity(),
                capacity
            );
        }

        #[test]
        fn throttle_refill_never_overflows_to_above_capacity(
            capacity in 1usize..50,
            refill_rate in 1usize..100,
            refill_period_ms in 1u64..10,
            time_advance_ms in 0u64..100000,
        ) {
            let config = RecoveryThrottleConfig {
                capacity,
                refill_rate,
                refill_period: Duration::from_millis(refill_period_ms),
            };
            let mut throttle = RecoveryThrottle::new(config);

            throttle.advance_time(Duration::from_millis(time_advance_ms));

            prop_assert!(
                throttle.available_capacity() <= capacity,
                "after large time advance, available ({}) must not exceed capacity ({})",
                throttle.available_capacity(),
                capacity
            );
        }
    }
}
