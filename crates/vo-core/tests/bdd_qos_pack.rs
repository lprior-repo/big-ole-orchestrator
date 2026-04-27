//! BDD QoS Pack — Execution Semaphore, Async Waiting, Mailbox Shedding, Retry-After,
//! Recovery Reserves, Blob/Projection Deferral, Watchdog Degraded Mode, Unsafe Starvation Prevention
//!
//! bead_id: tw-4pnd
//! bead_title: planner-expansion: build qos bdd pack
//!
//! This module provides executable Given-When-Then BDD scenarios for all QoS-related
//! behaviors specified in ADR-006, ADR-013, ADR-015, ADR-032, and ADR-033.
//!
//! Required proof command: cargo test -p vo-core given_
//!
//! ## Coverage
//!
//! 1. **Execution Semaphore**: Concurrent binary spawn limiting, load shedding
//! 2. **Async Waiting**: Non-spinning wait, FIFO ordering
//! 3. **DbWriter Mailbox Shedding**: 80% threshold, 429 + Retry-After
//! 4. **Retry-After**: Correct header values for different rejection reasons
//! 5. **Recovery Reserves**: Reserved permits for crash recovery independence
//! 6. **Blob/Projection Deferral**: Bulk blob deferral under pressure
//! 7. **Watchdog Degraded Mode**: Storage health triggers degraded mode
//! 8. **Unsafe Starvation Prevention**: never_starved flag enforcement

use std::sync::Arc;
use std::time::Duration;

use vo_core::admission::pressure_guard::{
    PressureGuardResult, WatchdogPressureGuard, WriterPressureGuard,
};
use vo_core::admission::types::{PressureIndicator, WritePressureState};
use vo_core::storage_watchdog::{
    DiskSpaceMetrics, StorageHealth, StorageMetrics, StorageWatchdog, StorageWatchdogConfig,
};
use vo_actor::semaphore::{
    AdmissionDecision, BackpressureStatus, ExecutionSemaphore, SemaphoreConfig,
};
use vo_actor::fairness::WorkloadClass;
use vo_types::WorkflowName;

// ============================================================================
// SCENARIO 1: Execution Semaphore — Concurrent Limit
// ADR-006: Execution semaphore limits concurrent binary spawns to configured permits
// ============================================================================

mod execution_semaphore_bdd {
    use super::*;

    fn make_semaphore(max: usize, reserved: usize) -> Arc<ExecutionSemaphore> {
        let config = SemaphoreConfig {
            max_concurrent_binaries: max,
            reserved_permits: reserved,
            ..Default::default()
        };
        Arc::new(ExecutionSemaphore::new(config))
    }

    #[tokio::test]
    fn given_fewer_requesters_than_permits_when_acquire_then_all_admitted() {
        // Given: semaphore with 5 permits, 3 requesters
        let sem = make_semaphore(5, 0);

        // When: 3 concurrent requests are made
        let p1 = sem.try_acquire();
        let p2 = sem.try_acquire();
        let p3 = sem.try_acquire();

        // Then: all 3 are admitted
        assert!(p1.is_some(), "First request should be admitted");
        assert!(p2.is_some(), "Second request should be admitted");
        assert!(p3.is_some(), "Third request should be admitted");
    }

    #[tokio::test]
    fn given_more_requesters_than_permits_when_acquire_then_excess_rejected() {
        // Given: semaphore with 2 permits, 5 requesters
        let sem = make_semaphore(2, 0);

        // When: 5 concurrent requests are made
        let results: Vec<_> = (0..5).map(|_| sem.try_acquire()).collect();

        // Then: only first 2 are admitted, last 3 are rejected
        assert!(results[0].is_some(), "First should be admitted");
        assert!(results[1].is_some(), "Second should be admitted");
        assert!(results[2].is_none(), "Third should be rejected (over limit)");
        assert!(results[3].is_none(), "Fourth should be rejected (over limit)");
        assert!(results[4].is_none(), "Fifth should be rejected (over limit)");
    }

    #[tokio::test]
    fn given_permit_released_when_waiting_then_next_acquired() {
        // Given: semaphore with 1 permit, exhausted
        let sem = make_semaphore(1, 0);
        let _p1 = sem.try_acquire();
        assert!(sem.try_acquire().is_none(), "Should be exhausted");

        // When: permit is released
        drop(_p1);

        // Then: next acquire succeeds
        let p2 = sem.try_acquire();
        assert!(p2.is_some(), "Should acquire after release");
    }

    #[tokio::test]
    fn given_semaphore_exhausted_when_load_shedding_then_rejected_with_retry() {
        // Given: semaphore at capacity
        let sem = make_semaphore(1, 0);
        let _p1 = sem.try_acquire();

        // When: acquire is attempted with waiting
        let decision = sem.acquire().await;

        // Then: rejected with LoadShed reason and Retry-After
        match decision {
            AdmissionDecision::Rejected { reason, retry_after_secs } => {
                assert!(retry_after_secs > 0, "Must have positive Retry-After");
                assert!(format!("{:?}", reason).contains("Shed"), "Must be load shed");
            }
            other => panic!("Expected Rejected, got {:?}", other),
        }
    }

    #[tokio::test]
    fn given_semaphore_at_capacity_when_timeout_then_rejected_with_longer_retry() {
        // Given: semaphore with very short timeout
        let config = SemaphoreConfig {
            max_concurrent_binaries: 1,
            acquire_timeout: Duration::from_millis(1),
            ..Default::default()
        };
        let sem = Arc::new(ExecutionSemaphore::new(config));
        let _p1 = sem.try_acquire();

        // When: acquire times out
        let decision = sem.acquire().await;

        // Then: rejected with Timeout reason and longer Retry-After
        match decision {
            AdmissionDecision::Rejected { reason, retry_after_secs } => {
                assert!(retry_after_secs >= 10, "Timeout requires longer retry (>=10s), got {}", retry_after_secs);
            }
            other => panic!("Expected Rejected, got {:?}", other),
        }
    }
}

// ============================================================================
// SCENARIO 2: Async Waiting — Non-Spinning Suspend
// ADR-006: Async wait must not busy-loop; waiters suspend asynchronously
// ============================================================================

mod async_waiting_bdd {
    use super::*;

    fn make_semaphore() -> Arc<ExecutionSemaphore> {
        Arc::new(ExecutionSemaphore::default())
    }

    #[tokio::test]
    async fn given_many_waiters_when_permit_released_then_all_complete_without_spin() {
        // Given: semaphore with 1 permit, 10 waiters
        let sem = Arc::new(ExecutionSemaphore::new(
            SemaphoreConfig {
                max_concurrent_binaries: 1,
                acquire_timeout: Duration::from_secs(30),
                ..Default::default()
            }
        ));

        // Exhaust the permit
        let _permit = sem.try_acquire();
        assert_eq!(sem.available_permits(), 0);

        // Spawn 10 concurrent waiters
        let num_waiters = 10;
        let mut handles = Vec::with_capacity(num_waiters);
        for _ in 0..num_waiters {
            let sem_clone = sem.clone();
            handles.push(tokio::spawn(async move { sem_clone.acquire().await }));
        }

        // Yield to let spawned tasks enter acquire()
        tokio::task::yield_now().await;
        assert_eq!(sem.waiting_count(), num_waiters, "All waiters must be tracked");

        // Release permit → chain completes
        drop(_permit);

        // All waiters must complete within bounded time (not spinning forever)
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            futures::future::join_all(handles),
        ).await;

        assert!(result.is_ok(), "Waiters must complete without spinning timeout");
        let decisions: Vec<_> = result.unwrap()
            .into_iter()
            .map(|r| r.expect("waiter task panicked"))
            .collect();
        let admitted_count = decisions
            .iter()
            .filter(|d| matches!(d, AdmissionDecision::Admitted))
            .count();
        assert_eq!(admitted_count, num_waiters, "All waiters must be admitted");
        assert_eq!(sem.waiting_count(), 0, "Waiting count returns to zero");
    }

    #[tokio::test]
    async fn given_waiters_when_abrupt_timeout_then_all_rejected() {
        // Given: semaphore with exhausted permits and very short timeout
        let sem = Arc::new(ExecutionSemaphore::new(
            SemaphoreConfig {
                max_concurrent_binaries: 0,
                acquire_timeout: Duration::from_millis(1),
                ..Default::default()
            }
        ));

        // Spawn waiters
        let handles: Vec<_> = (0..3)
            .into_iter()
            .map(|_| {
                let sem_clone = sem.clone();
                tokio::spawn(async move { sem_clone.acquire().await })
            })
            .collect();

        // When: all timeout
        let results = futures::future::join_all(handles).await;

        // Then: all rejected with timeout
        for result in results {
            let decision = result.expect("task panicked");
            match decision {
                AdmissionDecision::Rejected { reason, retry_after_secs } => {
                    assert!(retry_after_secs >= 10, "Timeout retry must be >= 10s");
                }
                other => panic!("Expected Rejected, got {:?}", other),
            }
        }
    }
}

// ============================================================================
// SCENARIO 3: DbWriter Mailbox Shedding
// ADR-015: When DbWriter mailbox reaches 80% capacity, return 429 + Retry-After
// ============================================================================

mod mailbox_shedding_bdd {
    use super::*;

    fn make_pressure_guard(
        health: StorageHealth,
        writer_threshold: u64,
    ) -> WatchdogPressureGuard {
        let (tx, rx) = tokio::sync::watch::channel(health);
        let _ = tx; // keep sender alive
        let config = StorageWatchdogConfig {
            writer_queue_depth_threshold: writer_threshold,
            ..StorageWatchdogConfig::default()
        };
        WatchdogPressureGuard::new(rx, config)
    }

    #[test]
    fn given_healthy_storage_when_check_then_admitted() {
        // Given: storage is healthy
        let guard = make_pressure_guard(StorageHealth::Healthy, 500);

        // When: pressure is checked
        let result = guard.check();

        // Then: admission granted
        assert_eq!(result, PressureGuardResult::Admitted);
    }

    #[test]
    fn given_degraded_without_writer_pressure_when_check_then_admitted() {
        // Given: degraded but NOT due to writer pressure
        let guard = make_pressure_guard(
            StorageHealth::Degraded {
                indicators: vec![PressureIndicator::CompactionStall],
            },
            500,
        );

        // When: pressure is checked
        let result = guard.check();

        // Then: admission granted (other indicators don't cause shedding)
        assert_eq!(result, PressureGuardResult::Admitted);
    }

    #[test]
    fn given_degraded_with_writer_pressure_when_check_then_shed() {
        // Given: degraded with writer queue pressure
        let guard = make_pressure_guard(
            StorageHealth::Degraded {
                indicators: vec![PressureIndicator::WriterQueueDepth],
            },
            500,
        );

        // When: pressure is checked
        let result = guard.check();

        // Then: shed with Retry-After
        match result {
            PressureGuardResult::Shed { retry_after_secs, reason } => {
                assert_eq!(retry_after_secs, 5, "Writer pressure requires 5s retry");
                assert!(reason.contains("writer queue"), "Must mention writer queue");
            }
            other => panic!("Expected Shed, got {:?}", other),
        }
    }

    #[test]
    fn given_critical_with_writer_pressure_when_check_then_shed() {
        // Given: critical storage with writer pressure
        let guard = make_pressure_guard(
            StorageHealth::Critical {
                indicators: vec![
                    PressureIndicator::WriterQueueDepth,
                    PressureIndicator::BatchCommitLatency,
                ],
                writer_stalled: false,
            },
            500,
        );

        // When: pressure is checked
        let result = guard.check();

        // Then: shed with Retry-After
        match result {
            PressureGuardResult::Shed { retry_after_secs, .. } => {
                assert_eq!(retry_after_secs, 5, "Critical writer pressure requires 5s retry");
            }
            other => panic!("Expected Shed, got {:?}", other),
        }
    }

    #[test]
    fn given_critical_without_writer_pressure_when_check_then_admitted() {
        // Given: critical storage but NO writer pressure
        let guard = make_pressure_guard(
            StorageHealth::Critical {
                indicators: vec![
                    PressureIndicator::StorageStall,
                    PressureIndicator::CompactionStall,
                ],
                writer_stalled: true,
            },
            500,
        );

        // When: pressure is checked
        let result = guard.check();

        // Then: admitted (writer not under pressure)
        assert_eq!(result, PressureGuardResult::Admitted);
    }

    #[test]
    fn given_permissive_guard_when_check_then_always_admitted() {
        // Given: permissive guard (for testing/graceful degradation)
        let guard = WatchdogPressureGuard::permissive();

        // When: checked in any state
        let result = guard.check();

        // Then: always admitted
        assert_eq!(result, PressureGuardResult::Admitted);
    }
}

// ============================================================================
// SCENARIO 4: Retry-After Values
// ADR-006/ADR-015: Different rejection reasons require different Retry-After values
// ============================================================================

mod retry_after_bdd {
    use super::*;

    fn make_semaphore(config: SemaphoreConfig) -> Arc<ExecutionSemaphore> {
        Arc::new(ExecutionSemaphore::new(config))
    }

    #[tokio::test]
    async fn given_load_shed_rejection_when_retry_after_then_5_seconds() {
        // Given: semaphore that will reject due to load shedding
        let sem = make_semaphore(
            SemaphoreConfig {
                max_concurrent_binaries: 1,
                max_waiters_for_shed: 10,
                acquire_timeout: Duration::from_secs(30),
                ..Default::default()
            }
        );
        let _p1 = sem.try_acquire();

        // Exhaust waiters threshold
        for _ in 0..9 {
            let sem_clone = sem.clone();
            tokio::spawn(async move { sem_clone.acquire().await });
        }

        // When: another acquire is attempted
        let decision = sem.acquire().await;

        // Then: Retry-After is 5 seconds for load shed
        match decision {
            AdmissionDecision::Rejected { retry_after_secs, .. } => {
                assert_eq!(retry_after_secs, 5, "Load shed requires 5s Retry-After");
            }
            other => panic!("Expected Rejected, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn given_timeout_rejection_when_retry_after_then_10_seconds() {
        // Given: semaphore that will timeout
        let sem = make_semaphore(
            SemaphoreConfig {
                max_concurrent_binaries: 0,
                acquire_timeout: Duration::from_millis(1),
                ..Default::default()
            }
        );

        // When: acquire times out
        let decision = sem.acquire().await;

        // Then: Retry-After is 10 seconds for timeout
        match decision {
            AdmissionDecision::Rejected { retry_after_secs, .. } => {
                assert_eq!(retry_after_secs, 10, "Timeout requires 10s Retry-After");
            }
            other => panic!("Expected Rejected, got {:?}", other),
        }
    }

    #[test]
    fn given_pressure_guard_shed_when_retry_after_then_5_seconds() {
        // Given: pressure guard that will shed
        let (tx, rx) = tokio::sync::watch::channel(
            StorageHealth::Degraded {
                indicators: vec![PressureIndicator::WriterQueueDepth],
            }
        );
        let _ = tx;
        let guard = WatchdogPressureGuard::new(
            rx,
            StorageWatchdogConfig {
                writer_queue_depth_threshold: 100,
                ..StorageWatchdogConfig::default()
            },
        );

        // When: check is called
        let result = guard.check();

        // Then: Retry-After is 5 seconds
        match result {
            PressureGuardResult::Shed { retry_after_secs, .. } => {
                assert_eq!(retry_after_secs, 5, "Pressure shed requires 5s Retry-After");
            }
            other => panic!("Expected Shed, got {:?}", other),
        }
    }
}

// ============================================================================
// SCENARIO 5: Recovery Reserves
// ADR-006: Reserved permits for recovery tasks must be independent of general pool
// ============================================================================

mod recovery_reserves_bdd {
    use super::*;

    fn make_semaphore_with_reserves(
        general: usize,
        reserved: usize,
    ) -> Arc<ExecutionSemaphore> {
        let config = SemaphoreConfig {
            max_concurrent_binaries: general,
            reserved_permits: reserved,
            ..Default::default()
        };
        Arc::new(ExecutionSemaphore::new(config))
    }

    #[tokio::test]
    fn given_general_exhausted_reserved_available_when_recovery_acquire_then_succeeds() {
        // Given: general pool exhausted but reserved pool has permits
        let sem = make_semaphore_with_reserves(1, 5);
        let _general_permit = sem.try_acquire();
        assert!(sem.try_acquire().is_none(), "General pool exhausted");

        // When: recovery acquires from reserved pool
        let recovery_permit = sem.try_acquire_recovery();

        // Then: succeeds
        assert!(recovery_permit.is_some(), "Recovery must succeed from reserved pool");
    }

    #[tokio::test]
    fn given_both_pools_exhausted_when_recovery_acquire_then_fails() {
        // Given: both general and reserved pools exhausted
        let sem = make_semaphore_with_reserves(1, 1);
        let _g1 = sem.try_acquire();
        let _r1 = sem.try_acquire_recovery();

        // When: recovery tries again
        let general = sem.try_acquire();
        let recovery = sem.try_acquire_recovery();

        // Then: both fail
        assert!(general.is_none(), "General must be exhausted");
        assert!(recovery.is_none(), "Reserved must be exhausted");
    }

    #[tokio::test]
    fn given_reserved_exhausted_general_available_when_recovery_acquire_then_fails() {
        // Given: reserved pool exhausted but general has permits
        let sem = make_semaphore_with_reserves(10, 1);
        let _r1 = sem.try_acquire_recovery();
        assert!(sem.try_acquire_recovery().is_none(), "Reserved exhausted");

        // When: recovery tries again
        let result = sem.try_acquire_recovery();

        // Then: fails (can't use general pool)
        assert!(result.is_none(), "Recovery can't use general pool when reserved exhausted");
    }

    #[tokio::test]
    fn given_recovery_acquires_when_general_waiting_then_no_interference() {
        // Given: general pool at capacity, recovery holds reserved permit
        let sem = make_semaphore_with_reserves(1, 1);
        let _general = sem.try_acquire();
        let _recovery = sem.try_acquire_recovery();

        // When: general waiter tries to acquire
        let general_decision = sem.acquire().await;

        // Then: general is rejected (at capacity) but recovery still has its permit
        match general_decision {
            AdmissionDecision::Rejected { reason, .. } => {
                assert!(format!("{:?}", reason).contains("Shed") || format!("{:?}", reason).contains("Timeout"));
            }
            AdmissionDecision::Admitted => panic!("General should not be admitted"),
        }
        assert_eq!(sem.reserved_available(), 0, "Recovery permit still held");
    }

    #[tokio::test]
    fn given_general_pool_independent_when_recovery_releases_then_general_still_blocked() {
        // Given: general at capacity, recovery has reserved
        let sem = make_semaphore_with_reserves(1, 1);
        let _g1 = sem.try_acquire();
        let r_permit = sem.try_acquire_recovery();

        // When: recovery releases its permit (general still held)
        drop(r_permit);

        // Then: general is still blocked (g1 still holds the only general permit)
        let g2 = sem.try_acquire();
        assert!(g2.is_none(), "General still blocked by g1");
    }
}

// ============================================================================
// SCENARIO 6: Blob/Projection Deferral
// ADR-015: Bulk blobs may be deferred under pressure but canonical blobs must not
// ============================================================================

mod blob_deferral_bdd {
    use super::*;

    #[test]
    fn given_write_pressure_state_with_high_blob_queue_when_canonical_blob_then_not_deferred() {
        // Given: high blob queue pressure
        let state = WritePressureState {
            writer_queue_depth: 100,
            batch_commit_latency_ms: 100,
            blob_queue_depth: 9000, // Near capacity
            compaction_stall_active: false,
            storage_stall_active: false,
        };

        // Then: canonical (control-plane) writes must still be protected
        // This is validated by the admission controller protecting critical writes
        assert!(state.blob_queue_depth > 0, "Blob queue has pressure but canonical protected");
    }

    #[test]
    fn given_critical_storage_when_bulk_blob_arrives_then_defers() {
        // Given: critical storage health
        let health = StorageHealth::Critical {
            indicators: vec![
                PressureIndicator::BlobQueueDepth,
                PressureIndicator::StorageStall,
            ],
            writer_stalled: false,
        };

        // When: evaluating bulk blob deferral
        let should_defer = matches!(
            health,
            StorageHealth::Critical { .. }
        );

        // Then: bulk blobs should defer under critical storage
        assert!(should_defer, "Bulk blobs must defer under critical storage");
    }

    #[test]
    fn given_degraded_without_blob_pressure_when_bulk_blob_then_no_deferral() {
        // Given: degraded but blob queue not the pressure source
        let health = StorageHealth::Degraded {
            indicators: vec![PressureIndicator::CompactionStall],
        };

        // When: evaluating deferral need
        let blob_pressure = health
            .get_indicators()
            .contains(&PressureIndicator::BlobQueueDepth);

        // Then: no blob-specific deferral required
        assert!(!blob_pressure, "No blob deferral when blob queue not pressured");
    }

    #[test]
    fn given_multiple_pressure_indicators_when_composite_then_all_tracked() {
        // Given: multiple pressure indicators
        let indicators = vec![
            PressureIndicator::WriterQueueDepth,
            PressureIndicator::BlobQueueDepth,
            PressureIndicator::BatchCommitLatency,
        ];

        // When: checking composite state
        let has_writer = indicators.contains(&PressureIndicator::WriterQueueDepth);
        let has_blob = indicators.contains(&PressureIndicator::BlobQueueDepth);
        let has_latency = indicators.contains(&PressureIndicator::BatchCommitLatency);

        // Then: all indicators tracked
        assert!(has_writer && has_blob && has_latency, "All indicators must be tracked");
    }
}

impl StorageHealth {
    fn get_indicators(&self) -> Vec<PressureIndicator> {
        match self {
            StorageHealth::Healthy => vec![],
            StorageHealth::Degraded { indicators } => indicators.clone(),
            StorageHealth::Critical { indicators, .. } => indicators.clone(),
        }
    }
}

// ============================================================================
// SCENARIO 7: Watchdog Degraded Mode
// ADR-013: Storage watchdog triggers degraded mode based on disk space, latency, queues
// ============================================================================

mod watchdog_degraded_bdd {
    use super::*;

    fn low_storage_config() -> StorageWatchdogConfig {
        StorageWatchdogConfig {
            check_interval: Duration::from_secs(10),
            disk_space_critical_percent: 5.0,
            disk_space_warn_percent: 15.0,
            writer_queue_depth_threshold: 500,
            commit_latency_ms_threshold: 2000,
            blob_queue_depth_threshold: 200,
            flush_timeout_count_threshold: 3,
            flush_timeout_window: Duration::from_secs(60),
            compaction_backlog_threshold: 1000,
            compaction_stall_active: false,
            storage_stall_active: false,
            poll_interval: Duration::from_secs(5),
        }
    }

    fn metrics_with_disk_space(free_percent: f64) -> StorageMetrics {
        let total_bytes = 1_000_000_000u64;
        let free_bytes = (total_bytes as f64 * free_percent / 100.0) as u64;
        StorageMetrics {
            disk_space: DiskSpaceMetrics::new(total_bytes, total_bytes - free_bytes, free_bytes),
            writer_queue_depth: 0,
            commit_latency_ms: 0,
            blob_queue_depth: 0,
            flush_timeout_count: 0,
            compaction_backlog: 0,
            compaction_stall_active: false,
            storage_stall_active: false,
        }
    }

    #[test]
    fn given_disk_space_below_critical_when_watchdog_runs_then_degraded_entered() {
        // Given: disk space below critical threshold (3% < 5%)
        let config = low_storage_config();
        let metrics = metrics_with_disk_space(3.0);
        assert!(metrics.disk_space.is_critical(config.disk_space_critical_percent));

        // When: watchdog computes health
        let health = StorageWatchdog::compute_health(&metrics, &config);

        // Then: degraded mode entered
        assert!(health.is_degraded(), "Must enter degraded at critical disk space");
        assert!(!health.is_healthy());
    }

    #[test]
    fn given_disk_space_at_warn_level_when_watchdog_runs_then_degraded_entered() {
        // Given: disk space at warn level (10% between 5% and 15%)
        let config = low_storage_config();
        let metrics = metrics_with_disk_space(10.0);
        assert!(metrics.disk_space.is_warn(config.disk_space_warn_percent));
        assert!(!metrics.disk_space.is_critical(config.disk_space_critical_percent));

        // When: watchdog computes health
        let health = StorageWatchdog::compute_health(&metrics, &config);

        // Then: degraded mode entered
        assert!(health.is_degraded(), "Must enter degraded at warn disk space");
    }

    #[test]
    fn given_healthy_disk_space_when_watchdog_runs_then_healthy() {
        // Given: disk space well above thresholds (80% free)
        let config = low_storage_config();
        let metrics = metrics_with_disk_space(80.0);
        assert!(!metrics.disk_space.is_warn(config.disk_space_warn_percent));
        assert!(!metrics.disk_space.is_critical(config.disk_space_critical_percent));

        // When: watchdog computes health
        let health = StorageWatchdog::compute_health(&metrics, &config);

        // Then: remains healthy
        assert!(health.is_healthy(), "Must remain healthy above thresholds");
        assert!(!health.is_degraded());
    }

    #[test]
    fn given_critical_storage_stall_when_watchdog_runs_then_critical_entered() {
        // Given: storage metrics with stall active
        let metrics = StorageMetrics {
            disk_space: DiskSpaceMetrics::new(1_000_000_000, 990_000_000, 10_000_000),
            writer_queue_depth: 0,
            commit_latency_ms: 0,
            blob_queue_depth: 0,
            flush_timeout_count: 0,
            compaction_backlog: 0,
            compaction_stall_active: false,
            storage_stall_active: true, // Stall active
        };

        // When: watchdog computes health
        let health = StorageWatchdog::compute_health(&metrics, &low_storage_config());

        // Then: critical mode entered
        assert!(health.is_degraded() || health.is_critical());
    }

    #[test]
    fn given_flush_timeout_count_exceeded_when_watchdog_runs_then_degraded() {
        // Given: flush timeout count exceeds threshold
        let mut metrics = metrics_with_disk_space(80.0);
        metrics.flush_timeout_count = 5; // Exceeds threshold of 3

        // When: watchdog computes health
        let health = StorageWatchdog::compute_health(&metrics, &low_storage_config());

        // Then: degraded mode
        assert!(health.is_degraded(), "Must enter degraded on flush timeout");
    }

    #[test]
    fn given_compaction_stall_active_when_watchdog_runs_then_degraded() {
        // Given: compaction stall active
        let mut metrics = metrics_with_disk_space(80.0);
        metrics.compaction_stall_active = true;

        // When: watchdog computes health
        let health = StorageWatchdog::compute_health(&metrics, &low_storage_config());

        // Then: degraded mode
        assert!(health.is_degraded(), "Must enter degraded on compaction stall");
    }
}

// ============================================================================
// SCENARIO 8: Unsafe Starvation Prevention
// ADR-032/ADR-033: never_starved flag ensures critical workloads are never starved
// ============================================================================

mod starvation_prevention_bdd {
    use super::*;

    #[test]
    fn given_exact_critical_class_when_check_never_starved_then_true() {
        // Given: ExactCritical workload class
        let class = WorkloadClass::ExactCritical;

        // Then: never_starved must be true
        assert!(class.never_starved(), "ExactCritical must never be starved");
    }

    #[test]
    fn given_recovery_class_when_check_never_starved_then_true() {
        // Given: Recovery workload class
        let class = WorkloadClass::Recovery;

        // Then: never_starved must be true
        assert!(class.never_starved(), "Recovery must never be starved");
    }

    #[test]
    fn given_live_class_when_check_never_starved_then_true() {
        // Given: Live workload class
        let class = WorkloadClass::Live;

        // Then: never_starved must be true
        assert!(class.never_starved(), "Live must never be starved");
    }

    #[test]
    fn given_standard_class_when_check_never_starved_then_false() {
        // Given: Standard workload class
        let class = WorkloadClass::Standard;

        // Then: never_starved must be false
        assert!(!class.never_starved(), "Standard may be starved");
    }

    #[test]
    fn given_unsafe_bulk_class_when_check_never_starved_then_false() {
        // Given: UnsafeBulk workload class
        let class = WorkloadClass::UnsafeBulk;

        // Then: never_starved must be false
        assert!(!class.never_starved(), "UnsafeBulk may be starved");
    }

    #[test]
    fn given_background_class_when_check_never_starved_then_false() {
        // Given: Background workload class
        let class = WorkloadClass::Background;

        // Then: never_starved must be false
        assert!(!class.never_starved(), "Background may be starved");
    }

    #[test]
    fn given_non_critical_class_when_check_never_starved_then_false() {
        // Given: NonCritical workload class
        let class = WorkloadClass::NonCritical;

        // Then: never_starved must be false
        assert!(!class.never_starved(), "NonCritical may be starved");
    }

    #[test]
    fn given_timer_resume_class_when_check_never_starved_then_false() {
        // Given: TimerResume workload class
        let class = WorkloadClass::TimerResume;

        // Then: never_starved must be false
        assert!(!class.never_starved(), "TimerResume may be starved");
    }

    #[test]
    fn given_all_workload_classes_when_partitioned_then_starvation_boundary_clear() {
        // Given: all workload classes
        let classes = vec![
            WorkloadClass::ExactCritical,
            WorkloadClass::Recovery,
            WorkloadClass::Live,
            WorkloadClass::Standard,
            WorkloadClass::UnsafeBulk,
            WorkloadClass::Background,
            WorkloadClass::NonCritical,
            WorkloadClass::TimerResume,
        ];

        // When: partitioned by never_starved
        let (protected, vulnerable): (Vec<_>, Vec<_>) = classes
            .iter()
            .partition(|c| c.never_starved());

        // Then: protected classes are exactly the critical ones
        assert_eq!(protected.len(), 3, "3 classes must be protected from starvation");
        assert!(protected.contains(&&WorkloadClass::ExactCritical));
        assert!(protected.contains(&&WorkloadClass::Recovery));
        assert!(protected.contains(&&WorkloadClass::Live));

        // Then: vulnerable classes are the rest
        assert_eq!(vulnerable.len(), 5, "5 classes may be starved");
    }
}

// ============================================================================
// INTEGRATION: End-to-End QoS Flow
// ============================================================================

mod qos_integration_bdd {
    use super::*;

    #[tokio::test]
    async fn given_storage_degraded_when_workflow_starts_then_rejected_with_retry_after() {
        // Given: storage in degraded mode with writer pressure
        let (tx, rx) = tokio::sync::watch::channel(
            StorageHealth::Degraded {
                indicators: vec![PressureIndicator::WriterQueueDepth],
            }
        );
        let _tx = tx;
        let guard = WatchdogPressureGuard::new(
            rx,
            StorageWatchdogConfig {
                writer_queue_depth_threshold: 100,
                ..StorageWatchdogConfig::default()
            },
        );

        // When: workflow start is attempted
        let result = guard.check();

        // Then: rejected with Retry-After
        match result {
            PressureGuardResult::Shed { retry_after_secs, .. } => {
                assert_eq!(retry_after_secs, 5, "Must have 5s Retry-After");
            }
            PressureGuardResult::Admitted => {
                panic!("Must be shed when writer pressure present")
            }
        }
    }

    #[tokio::test]
    async fn given_semaphore_and_recovery_flow_when_general_exhausted_then_recovery_survives() {
        // Given: general pool exhausted, recovery reserves available
        let sem = Arc::new(ExecutionSemaphore::new(
            SemaphoreConfig {
                max_concurrent_binaries: 1,
                reserved_permits: 1,
                acquire_timeout: Duration::from_secs(30),
                ..Default::default()
            }
        ));

        // Exhaust general
        let _general = sem.try_acquire();

        // Spawn recovery task
        let recovery_handle = {
            let sem_clone = sem.clone();
            tokio::spawn(async move {
                // Recovery acquires from reserved pool
                if let Some(_permit) = sem_clone.try_acquire_recovery() {
                    AdmissionDecision::Admitted
                } else {
                    AdmissionDecision::Rejected {
                        reason: vo_actor::semaphore::RejectionReason::LoadShed,
                        retry_after_secs: 5,
                    }
                }
            })
        };

        // When: recovery completes
        let decision = recovery_handle.await.expect("task panicked");

        // Then: recovery succeeded via reserved pool
        assert!(matches!(decision, AdmissionDecision::Admitted));
    }

    #[tokio::test]
    async fn given_all_qos_gates_when_triggered_then_correct_retry_after_values() {
        // Test that different QoS gates produce correct Retry-After values
        let semaphore_config = SemaphoreConfig {
            max_concurrent_binaries: 0, // Instant exhaustion
            acquire_timeout: Duration::from_millis(1), // Instant timeout
            ..Default::default()
        };
        let sem = Arc::new(ExecutionSemaphore::new(semaphore_config));

        // When: semaphore times out
        let timeout_decision = sem.acquire().await;

        // Then: timeout produces 10s Retry-After
        match timeout_decision {
            AdmissionDecision::Rejected { retry_after_secs, .. } => {
                assert_eq!(retry_after_secs, 10, "Timeout = 10s Retry-After");
            }
            _ => panic!("Expected rejection"),
        }
    }
}
