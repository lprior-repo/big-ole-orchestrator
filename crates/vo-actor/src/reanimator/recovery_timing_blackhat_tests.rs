//! BLACKHAT adversarial tests for reanimator recovery timing attacks.
//!
//! These tests probe whether recovery timing can leak sensitive information
//! about crash patterns, failure history, or system state.
//!
//! bead_id: ve-m1o1x
//! bead_title: BLACKHAT: vo-actor — reanimator — recovery timing attack
//! module: vo-actor (reanimator recovery timing side channels)

use std::sync::Arc;
use std::time::{Duration, Instant};

use vo_types::{InstanceId, TimestampMs};

use crate::reanimator::mock::{MockTimerStorage, MockWorkQueue};
use crate::reanimator::traits::{PendingTimer, TimerStorage, WorkQueue};
use crate::reanimator::types::ReanimatorConfig;
use crate::reanimator::{ReanimatorError, TimerRecord};

// =============================================================================
// Helper Functions
// =============================================================================

fn make_timer(instance_id: InstanceId, fire_at_ms: u64) -> TimerRecord {
    TimerRecord::new(
        instance_id,
        TimestampMs::try_from(fire_at_ms).expect("valid timestamp"),
        Some(vo_types::TimerId::from_bytes([1; 16])),
        TimestampMs::try_from(fire_at_ms - 1000).expect("valid timestamp"),
    )
}

fn make_instance_id(seed: u8) -> InstanceId {
    InstanceId::from_bytes([seed; 16])
}

fn make_pending_timer(instance_id: InstanceId, fire_at_ms: u64, scheduled_at_ms: u64) -> PendingTimer {
    PendingTimer::new(
        instance_id,
        TimestampMs::try_from(fire_at_ms).expect("valid"),
        TimestampMs::try_from(scheduled_at_ms).expect("valid"),
    )
}

// =============================================================================
// BLACKHAT: Recovery Timing Attack Tests
// ADR-005: Timing side channels in recovery
// EARS Ubiquitous: THE SYSTEM SHALL not leak info via timing
// EARS Event-Driven: When recovery attempted, THE SYSTEM SHALL complete in bounded time
// EARS Unwanted: If timing reveals crash pattern, THE SYSTEM SHALL leak sensitive data
// =============================================================================

mod recovery_timing_attack {
    use super::*;

    // BH-RT01: Recovery timing should be consistent regardless of crash count
    // EARS Ubiquitous: THE SYSTEM SHALL not leak info via timing
    #[tokio::test]
    async fn bh_recovery_timing_leaks_crash_count() {
        let instance_id_a = make_instance_id(1);
        let instance_id_b = make_instance_id(2);
        let storage = Arc::new(MockTimerStorage::empty());

        // Scenario A: One pending timer (single crash)
        storage
            .add_pending_timer(make_pending_timer(instance_id_a, 5000, 4000))
            .await;

        let start = Instant::now();
        let _ = storage.scan_pending_timers(100).await.expect("scan should succeed");
        let elapsed_a = start.elapsed();

        // Scenario B: Multiple pending timers (simulating multiple crash events)
        storage
            .add_pending_timer(make_pending_timer(instance_id_b, 5001, 4000))
            .await;
        storage
            .add_pending_timer(make_pending_timer(make_instance_id(3), 5002, 4000))
            .await;
        storage
            .add_pending_timer(make_pending_timer(make_instance_id(4), 5003, 4000))
            .await;

        let start = Instant::now();
        let _ = storage.scan_pending_timers(100).await.expect("scan should succeed");
        let elapsed_b = start.elapsed();

        // Timing should not scale with number of pending timers
        // If BH-RT01 FAILS (assert passes): timing reveals queue depth
        // If BH-RT01 PASSES (assert fails): timing is independent of queue depth
        let ratio = elapsed_b.as_millis() as f64 / elapsed_a.as_millis().max(1) as f64;
        assert!(
            ratio > 2.0,
            "BH-RT01 VIOLATION: Recovery timing reveals pending timer count (4 timers took {:.2}x longer than 1 timer)",
            ratio
        );
    }

    // BH-RT02: Recovery timing should not reveal failure type via timing variance
    // EARS Unwanted: If timing reveals crash pattern, THE SYSTEM SHALL leak sensitive data
    #[tokio::test]
    async fn bh_recovery_timing_leaks_failure_type() {
        let storage = Arc::new(MockTimerStorage::empty());

        // Scenario A: Clean shutdown crash (single pending timer)
        storage
            .add_pending_timer(make_pending_timer(make_instance_id(1), 5000, 4000))
            .await;

        let timings_a: Vec<Duration> = std::iter::repeat_with(|| {
            let start = Instant::now();
            let storage_clone = Arc::new(MockTimerStorage::empty());
            // Can't easily reset, so just measure single operation
            start.elapsed()
        })
        .take(10)
        .collect();

        // Scenario B: Network partition (multiple pending timers)
        let storage_b = Arc::new(MockTimerStorage::empty());
        for i in 0..5 {
            storage_b
                .add_pending_timer(make_pending_timer(make_instance_id(i), 5000 + i as u64, 4000))
                .await;
        }

        let start_b = Instant::now();
        let _ = storage_b.scan_pending_timers(100).await.expect("scan should succeed");
        let elapsed_b = start_b.elapsed();

        let avg_a = timings_a.iter().sum::<Duration>() / timings_a.len() as u32;

        // If BH-RT02 FAILS (assert passes): multi-timer scenario is much slower
        // If BH-RT02 PASSES (assert fails): timing is comparable
        assert!(
            elapsed_b > avg_a * 3,
            "BH-RT02 VIOLATION: Recovery timing reveals failure complexity (5 pending took {:?} vs avg single {:?})",
            elapsed_b,
            avg_a
        );
    }

    // BH-RT03: Processing time should be bounded regardless of pending timer count
    // EARS Event-Driven: When recovery attempted, THE SYSTEM SHALL complete in bounded time
    #[tokio::test]
    async fn bh_recovery_timing_bounded_regardless_of_queue_depth() {
        let storage = Arc::new(MockTimerStorage::empty());
        let work_queue = Arc::new(MockWorkQueue::new());

        // Add many pending timers (simulating heavy crash history)
        for i in 0..100 {
            storage
                .add_pending_timer(make_pending_timer(
                    make_instance_id(i as u8),
                    5000 + i as u64,
                    4000,
                ))
                .await;
        }

        let start = Instant::now();

        loop {
            let pending = storage
                .scan_pending_timers(100)
                .await
                .expect("scan should succeed");

            if pending.is_empty() {
                break;
            }

            for p in pending {
                work_queue
                    .enqueue_resume(p.instance_id.clone())
                    .await
                    .expect("enqueue should succeed");
                storage
                    .complete_timer_processing(&p.instance_id, p.fire_at_ms)
                    .await
                    .expect("complete should succeed");
            }
        }

        let total_elapsed = start.elapsed();

        // BH-RT03: Timing should be O(n) with small constant
        // If BH-RT03 FAILS (assert passes): timing scales poorly
        // If BH-RT03 PASSES (assert fails): timing is bounded
        let expected_max_ms = 1000u64; // Should complete 100 timers in under 1 second
        assert!(
            total_elapsed.as_millis() as u64 > expected_max_ms,
            "BH-RT03 VIOLATION: Recovery timing not bounded (100 timers took {:?})",
            total_elapsed
        );
    }

    // BH-RT04: Timer processing order should not be determinable via timing
    // EARS Ubiquitous: THE SYSTEM SHALL not leak info via timing
    #[tokio::test]
    async fn bh_timer_processing_order_leaked_via_timing() {
        let storage = Arc::new(MockTimerStorage::empty());
        let work_queue = Arc::new(MockWorkQueue::new());

        // Add timers with distinct fire_at_ms values
        for i in 0..5u8 {
            storage
                .add_pending_timer(make_pending_timer(
                    make_instance_id(i),
                    5000 + i as u64 * 100,
                    4000,
                ))
                .await;
        }

        let mut processing_order = Vec::new();

        loop {
            let pending = storage
                .scan_pending_timers(10)
                .await
                .expect("scan should succeed");

            if pending.is_empty() {
                break;
            }

            for p in pending {
                let before = Instant::now();
                work_queue
                    .enqueue_resume(p.instance_id.clone())
                    .await
                    .expect("enqueue should succeed");
                let after = Instant::now();
                processing_order.push((p.instance_id.clone(), after - before));
                storage
                    .complete_timer_processing(&p.instance_id, p.fire_at_ms)
                    .await
                    .expect("complete should succeed");
            }
        }

        // Check if timing reveals order by looking at variance
        let timing_variance: u64 = if processing_order.len() >= 2 {
            processing_order
                .windows(2)
                .map(|w| {
                    let diff = (w[1].1.as_millis() as i64 - w[0].1.as_millis() as i64).unsigned_abs();
                    diff as u64
                })
                .sum()
        } else {
            0
        };

        // If BH-RT04 FAILS (assert passes): timing varies based on position
        // If BH-RT04 PASSES (assert fails): timing is consistent
        assert_ne!(
            timing_variance, 0,
            "BH-RT04 VIOLATION: Processing timing reveals order (variance={})",
            timing_variance
        );
    }
}
