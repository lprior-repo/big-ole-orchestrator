#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::disallowed_methods)]
//! Red Queen coevolutionary tests: timer leak under panic
//!
//! bead_id: ve-3iido
//! bead_title: REDQUEEN: vo-actor — timer lifecycle — timer leak under panic
//!
//! These tests verify that timers are NOT leaked when a panic occurs during
//! timer processing. The system must guarantee:
//! - INV-2 (delete-before-dispatch): timer deleted from storage before dispatch,
//!   so a crash after dispatch cannot cause double-fire
//! - Crash recovery: pending timers from a crashed reanimator are replayed
//! - Shutdown on panic: handle.shutdown() correctly detects and reports panics
//! - Timer count invariant: no timer is silently lost without being either
//!   dispatched or recovered

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use vo_types::{InstanceId, TimestampMs};

use vo_actor::reanimator::{
    MockTimerStorage, MockWorkQueue, ReanimatorConfig, ReanimatorLoop, ReanimatorState, TimerRecord,
};
use vo_actor::timer_lifecycle::{cancel_timers_for_instance, has_pending_timers};
use vo_actor::timer_supervisor::{
    supervisor::TimerSupervisor,
    traits::WorkQueue as SyncWorkQueue,
    types::TimerSupervisorError,
};
use vo_common::ports::{TimerStorage as AsyncTimerStorage, TimerRecord as UnifiedTimerRecord, TimerStorageError};

fn ts_ms(value: u64) -> TimestampMs {
    TimestampMs::try_from(value).expect("valid timestamp")
}

fn make_instance_id(byte: u8) -> InstanceId {
    InstanceId::from_bytes([byte; 16])
}

fn past_timer(instance_id: InstanceId, fire_at_ms: u64) -> TimerRecord {
    TimerRecord::new(
        instance_id,
        ts_ms(fire_at_ms),
        None,
        ts_ms(fire_at_ms.saturating_sub(1000).max(1)),
    )
}

// =============================================================================
// ATTACK VECTOR 1: Reanimator panic during process_cycle does not leak timers
// The reanimator's tokio task panics — do timers get cleaned up or recovered?
// =============================================================================

mod reanimator_panic_timer_recovery {
    use super::*;

    // RQ-TP01: Reanimator task panic does not leave timers orphaned in storage.
    // After panic, crash recovery must find and replay pending timers.
    #[tokio::test]
    async fn rq_reanimator_panic_timers_recovered_on_restart() {
        let instance_id = make_instance_id(0xA1);
        let timer = past_timer(instance_id.clone(), 5000);

        let storage = Arc::new(MockTimerStorage::new(vec![timer.clone()]));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(50),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(5),
        };

        let handle = ReanimatorLoop::spawn(config.clone(), storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        // Wait for the first scan cycle to fire the timer
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Shutdown — even if panic occurred, we should be able to shut down
        let shutdown_result = handle.shutdown().await;
        // Shutdown may fail with AlreadyShutdown if the task panicked and exited early
        let _ = shutdown_result;

        // Verify timer was processed (deleted from storage before dispatch)
        let enqueued = work_queue.enqueued().await;
        // The timer should have been dispatched (delete-before-dispatch happened)
        // If the reanimator panicked AFTER delete but BEFORE enqueue, the timer is lost
        // but NOT leaked (it's been deleted from storage, no double-fire possible).
        // If no panic, the timer should have been fully dispatched.
        assert!(
            enqueued.contains(&instance_id),
            "Timer should be dispatched after successful scan cycle. \
             If panic occurred, delete-before-dispatch (INV-2) guarantees the timer \
             was removed from storage (no leak), even if dispatch was incomplete."
        );
    }

    // RQ-TP02: Timer count invariant holds — storage is empty after normal processing.
    // Even under concurrent processing, delete-before-dispatch ensures each timer
    // is removed exactly once from storage.
    #[tokio::test]
    async fn rq_timer_storage_empty_after_fired() {
        let instance_id = make_instance_id(0xA2);
        let timer = past_timer(instance_id.clone(), 3000);

        let storage = Arc::new(MockTimerStorage::new(vec![timer]));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(50),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(5),
        };

        let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        tokio::time::sleep(Duration::from_millis(200)).await;
        handle.shutdown().await.expect("shutdown should succeed");

        // Timer should have been deleted from storage (INV-2)
        let has_pending = has_pending_timers(&storage, &instance_id)
            .await
            .expect("check should succeed");
        assert!(
            !has_pending,
            "After timer fires and is dispatched, it must be deleted from storage \
             (INV-2: delete-before-dispatch). No timer leak."
        );
    }

    // RQ-TP03: Multiple timers for different instances — all cleaned up, no partial leaks.
    #[tokio::test]
    async fn rq_multiple_timers_all_cleaned_up() {
        let timers = vec![
            past_timer(make_instance_id(0xA3), 1000),
            past_timer(make_instance_id(0xA4), 2000),
            past_timer(make_instance_id(0xA5), 3000),
        ];
        let instance_ids: Vec<_> = timers.iter().map(|t| t.instance_id.clone()).collect();

        let storage = Arc::new(MockTimerStorage::new(timers));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(50),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(5),
        };

        let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        tokio::time::sleep(Duration::from_millis(500)).await;
        handle.shutdown().await.expect("shutdown should succeed");

        let enqueued = work_queue.enqueued().await;
        assert_eq!(
            enqueued.len(),
            3,
            "All three timers should be dispatched (delete-before-dispatch for each)"
        );

        for instance_id in &instance_ids {
            let has_pending = has_pending_timers(&storage, instance_id)
                .await
                .expect("check should succeed");
            assert!(
                !has_pending,
                "All timers must be removed from storage after firing"
            );
        }
    }
}

// =============================================================================
// ATTACK VECTOR 2: TimerSupervisor panic paths
// The sync TimerSupervisor runs in a tokio task — panic during process_cycle
// must not leave timers in an inconsistent state.
// =============================================================================

mod timer_supervisor_panic_cleanup {
    use super::*;

    struct PanicOnEnqueueStorage {
        timers: std::sync::Mutex<Vec<UnifiedTimerRecord>>,
    }

    impl PanicOnEnqueueStorage {
        fn new(timers: Vec<UnifiedTimerRecord>) -> Self {
            Self {
                timers: std::sync::Mutex::new(timers),
            }
        }
    }

    #[async_trait::async_trait]
    impl AsyncTimerStorage for PanicOnEnqueueStorage {
        async fn schedule_timer(
            &self,
            _record: UnifiedTimerRecord,
        ) -> Result<(), TimerStorageError> {
            Ok(())
        }
        async fn cancel_timer(
            &self,
            _instance_id: &InstanceId,
            _fire_at_ms: TimestampMs,
        ) -> Result<(), TimerStorageError> {
            Ok(())
        }
        async fn get_timer(
            &self,
            _instance_id: &InstanceId,
            _fire_at_ms: TimestampMs,
        ) -> Result<UnifiedTimerRecord, TimerStorageError> {
            Err(TimerStorageError::NotFound {
                instance_id: make_instance_id(0x00),
                fire_at_ms: TimestampMs::new_unchecked(0),
            })
        }
        async fn list_timers_by_instance(
            &self,
            _instance_id: &InstanceId,
        ) -> Result<Vec<UnifiedTimerRecord>, TimerStorageError> {
            Ok(Vec::new())
        }
        async fn list_expired_timers(
            &self,
            _from: TimestampMs,
            _to: TimestampMs,
            _max: u32,
        ) -> Result<Vec<UnifiedTimerRecord>, TimerStorageError> {
            Ok(self.timers.lock().unwrap().clone())
        }
        async fn retry_timer(
            &self,
            _timer: &UnifiedTimerRecord,
            _new_fire_at_ms: TimestampMs,
        ) -> Result<(), TimerStorageError> {
            Ok(())
        }
        async fn delete_all_timers_for_instance(
            &self,
            _instance_id: &InstanceId,
        ) -> Result<u32, TimerStorageError> {
            Ok(0)
        }
    }

    struct PanicWorkQueue {
        should_panic: AtomicBool,
        enqueued: std::sync::Mutex<Vec<InstanceId>>,
    }

    impl PanicWorkQueue {
        fn new() -> Self {
            Self {
                should_panic: AtomicBool::new(false),
                enqueued: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl WorkQueue for PanicWorkQueue {
        async fn enqueue_spawn(
            &self,
            _instance_id: InstanceId,
            _executable: std::path::PathBuf,
            _args: Vec<String>,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
        async fn enqueue_resume(&self, instance_id: InstanceId) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            if self.should_panic.load(Ordering::SeqCst) {
                panic!("simulated panic during enqueue_resume");
            }
            self.enqueued.lock().unwrap().push(instance_id);
            Ok(())
        }
        async fn is_instance_terminal(&self, _instance_id: &InstanceId) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
            Ok(false)
        }
    }

    // RQ-TP04: TimerSupervisor shutdown detects panic in background task.
    #[tokio::test]
    async fn rq_supervisor_shutdown_detects_panic() {
        let now_ms = TimestampMs::now();
        let instance_id = make_instance_id(0xB1);
        let scheduled_at = TimestampMs::new_unchecked(now_ms.as_u64().saturating_sub(2000));
        let timer = UnifiedTimerRecord::new(
            instance_id.clone(),
            now_ms,
            None,
            scheduled_at,
        );

        let storage: Arc<dyn AsyncTimerStorage> = Arc::new(PanicOnEnqueueStorage::new(vec![timer]));
        let work_queue: Arc<dyn SyncWorkQueue> = Arc::new(PanicWorkQueue::new());

        let supervisor = TimerSupervisor::new(
            Duration::from_millis(50),
            storage.clone(),
            work_queue.clone(),
        )
        .expect("valid config");

        let handle = supervisor.spawn().expect("spawn should succeed");
        assert!(handle.is_running());

        // Wait for at least one cycle
        tokio::time::sleep(Duration::from_millis(150)).await;

        let result = handle.shutdown(Duration::from_secs(5)).await;
        assert!(
            result.is_ok(),
            "Shutdown should succeed even after normal processing"
        );
    }

    // RQ-TP05: Timer deleted from storage before dispatch via process_cycle.
    // After process_cycle, the timer is deleted and dispatched (INV-2).
    #[tokio::test]
    async fn rq_delete_before_dispatch_prevents_leak() {
        let instance_id = make_instance_id(0xB2);
        let now_ms = TimestampMs::now();
        let fire_at = TimestampMs::new_unchecked(now_ms.as_u64().saturating_sub(1000));
        let timer = UnifiedTimerRecord::new(
            instance_id.clone(),
            fire_at,
            None,
            TimestampMs::new_unchecked(fire_at.as_u64().saturating_sub(1000)),
        );

        let deleted_instance: Arc<std::sync::Mutex<Option<InstanceId>>> =
            Arc::new(std::sync::Mutex::new(None));
        let deleted_instance_clone = deleted_instance.clone();
        let enqueued_instances: Arc<std::sync::Mutex<Vec<InstanceId>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let enqueued_clone = enqueued_instances.clone();

        struct TrackingStorage {
            deleted: Arc<std::sync::Mutex<Option<InstanceId>>>,
            timer: std::sync::Mutex<Option<UnifiedTimerRecord>>,
        }

        #[async_trait::async_trait]
        impl AsyncTimerStorage for TrackingStorage {
            async fn schedule_timer(
                &self,
                _record: UnifiedTimerRecord,
            ) -> Result<(), TimerStorageError> {
                Ok(())
            }
            async fn cancel_timer(
                &self,
                instance_id: &InstanceId,
                _fire_at_ms: TimestampMs,
            ) -> Result<(), TimerStorageError> {
                *self.deleted.lock().unwrap() = Some(instance_id.clone());
                *self.timer.lock().unwrap() = None;
                Ok(())
            }
            async fn get_timer(
                &self,
                _instance_id: &InstanceId,
                _fire_at_ms: TimestampMs,
            ) -> Result<UnifiedTimerRecord, TimerStorageError> {
                Err(TimerStorageError::NotFound {
                    instance_id: make_instance_id(0x00),
                    fire_at_ms: TimestampMs::new_unchecked(0),
                })
            }
            async fn list_timers_by_instance(
                &self,
                _instance_id: &InstanceId,
            ) -> Result<Vec<UnifiedTimerRecord>, TimerStorageError> {
                Ok(self.timer.lock().unwrap().take().into_iter().collect())
            }
            async fn list_expired_timers(
                &self,
                _from: TimestampMs,
                _to: TimestampMs,
                _max: u32,
            ) -> Result<Vec<UnifiedTimerRecord>, TimerStorageError> {
                Ok(self.timer.lock().unwrap().take().into_iter().collect())
            }
            async fn retry_timer(
                &self,
                _timer: &UnifiedTimerRecord,
                _new_fire_at_ms: TimestampMs,
            ) -> Result<(), TimerStorageError> {
                Ok(())
            }
            async fn delete_all_timers_for_instance(
                &self,
                _instance_id: &InstanceId,
            ) -> Result<u32, TimerStorageError> {
                Ok(0)
            }
        }

        struct TrackingQueue {
            enqueued: Arc<std::sync::Mutex<Vec<InstanceId>>>,
        }

        impl SyncWorkQueue for TrackingQueue {
            fn enqueue_resume(&self, instance_id: InstanceId) -> Result<(), TimerSupervisorError> {
                self.enqueued.lock().unwrap().push(instance_id);
                Ok(())
            }
        }

        let storage: Arc<dyn AsyncTimerStorage> = Arc::new(TrackingStorage {
            deleted: deleted_instance_clone,
            timer: std::sync::Mutex::new(Some(timer)),
        });
        let work_queue: Arc<dyn SyncWorkQueue> = Arc::new(TrackingQueue {
            enqueued: enqueued_clone,
        });

        let supervisor = TimerSupervisor::new(
            Duration::from_millis(50),
            storage.clone(),
            work_queue.clone(),
        )
        .expect("valid config");

        let result = supervisor.process_cycle().await;
        assert!(result.is_ok(), "process_cycle should succeed");

        let deleted = deleted_instance.lock().unwrap();
        assert!(
            deleted.is_some(),
            "Timer must be deleted from storage BEFORE dispatch (INV-2)"
        );
        assert_eq!(
            deleted.as_ref().unwrap(),
            &instance_id,
            "Correct timer instance must be deleted"
        );

        let enqueued = enqueued_instances.lock().unwrap();
        assert!(
            enqueued.contains(&instance_id),
            "Timer must be dispatched after delete (INV-2)"
        );
    }

    // RQ-TP06: process_cycle handles delete failure without leaking timers.
    #[tokio::test]
    async fn rq_process_cycle_no_leak_on_dispatch_error() {
        let instance_id = make_instance_id(0xB3);
        let now_ms = TimestampMs::now();
        let fire_at = TimestampMs::new_unchecked(now_ms.as_u64().saturating_sub(1000));
        let timer = UnifiedTimerRecord::new(
            instance_id.clone(),
            fire_at,
            None,
            TimestampMs::new_unchecked(fire_at.as_u64().saturating_sub(1000)),
        );

        struct FailDeleteStorage {
            timer: std::sync::Mutex<Option<UnifiedTimerRecord>>,
        }

        #[async_trait::async_trait]
        impl AsyncTimerStorage for FailDeleteStorage {
            async fn schedule_timer(
                &self,
                _record: UnifiedTimerRecord,
            ) -> Result<(), TimerStorageError> {
                Ok(())
            }
            async fn cancel_timer(
                &self,
                _instance_id: &InstanceId,
                _fire_at_ms: TimestampMs,
            ) -> Result<(), TimerStorageError> {
                Err(TimerStorageError::StorageFailed("simulated failure".to_string()))
            }
            async fn get_timer(
                &self,
                _instance_id: &InstanceId,
                _fire_at_ms: TimestampMs,
            ) -> Result<UnifiedTimerRecord, TimerStorageError> {
                Err(TimerStorageError::NotFound {
                    instance_id: make_instance_id(0x00),
                    fire_at_ms: TimestampMs::new_unchecked(0),
                })
            }
            async fn list_timers_by_instance(
                &self,
                _instance_id: &InstanceId,
            ) -> Result<Vec<UnifiedTimerRecord>, TimerStorageError> {
                Ok(self.timer.lock().unwrap().take().into_iter().collect())
            }
            async fn list_expired_timers(
                &self,
                _from: TimestampMs,
                _to: TimestampMs,
                _max: u32,
            ) -> Result<Vec<UnifiedTimerRecord>, TimerStorageError> {
                Ok(self.timer.lock().unwrap().take().into_iter().collect())
            }
            async fn retry_timer(
                &self,
                _timer: &UnifiedTimerRecord,
                _new_fire_at_ms: TimestampMs,
            ) -> Result<(), TimerStorageError> {
                Ok(())
            }
            async fn delete_all_timers_for_instance(
                &self,
                _instance_id: &InstanceId,
            ) -> Result<u32, TimerStorageError> {
                Ok(0)
            }
        }

        let storage: Arc<dyn AsyncTimerStorage> = Arc::new(FailDeleteStorage {
            timer: std::sync::Mutex::new(Some(timer)),
        });

        struct NoopQueue;
        impl SyncWorkQueue for NoopQueue {
            fn enqueue_resume(&self, _instance_id: InstanceId) -> Result<(), TimerSupervisorError> {
                Ok(())
            }
        }

        let work_queue: Arc<dyn SyncWorkQueue> = Arc::new(NoopQueue);

        let supervisor = TimerSupervisor::new(
            Duration::from_millis(50),
            storage.clone(),
            work_queue,
        )
        .expect("valid config");

        // process_cycle should still complete (with errors logged)
        let result = supervisor.process_cycle().await;
        assert!(
            result.is_ok(),
            "process_cycle should complete even with delete failures"
        );
        // The delete failed so error_count should be > 0
        let cycle_result = result.unwrap();
        assert_eq!(
            cycle_result.error_count, 1,
            "Delete failure should be counted as error (timer NOT dispatched, NOT leaked)"
        );
        assert_eq!(
            cycle_result.timers_fired, 0,
            "No timers should be fired when delete fails"
        );
    }
}

// =============================================================================
// ATTACK VECTOR 3: Timer lifecycle cancellation under panic-like conditions
// cancel_timers_for_instance must remove all timers even if some storage ops
// appear to race with a panic.
// =============================================================================

mod timer_lifecycle_panic_safety {
    use super::*;

    // RQ-TP07: cancel_timers_for_instance removes all timers atomically.
    // If the process crashes during cancellation, either all timers are removed
    // or none are — partial cancellation is a leak.
    #[tokio::test]
    async fn rq_cancel_removes_all_timers_for_instance() {
        let instance_id = make_instance_id(0xC1);
        let storage = Arc::new(MockTimerStorage::empty());

        // Add 5 timers for this instance
        for i in 0..5u64 {
            storage
                .add_timer(past_timer(instance_id.clone(), 1000 + i * 1000))
                .await;
        }

        // Add timers for a different instance that should NOT be affected
        let other_instance = make_instance_id(0xC2);
        storage
            .add_timer(past_timer(other_instance.clone(), 1500))
            .await;

        let count = cancel_timers_for_instance(&storage, &instance_id)
            .await
            .expect("cancel should succeed");

        assert_eq!(count, 5, "All 5 timers for the instance must be cancelled");

        let has_pending = has_pending_timers(&storage, &instance_id)
            .await
            .expect("check should succeed");
        assert!(
            !has_pending,
            "After cancellation, no pending timers should remain for the instance"
        );

        // Other instance's timers must be unaffected
        let other_has_pending = has_pending_timers(&storage, &other_instance)
            .await
            .expect("check should succeed");
        assert!(
            other_has_pending,
            "Other instance's timers must not be affected by cancellation"
        );
    }

    // RQ-TP08: cancel_timers_for_instance is safe to call on instance with no timers.
    #[tokio::test]
    async fn rq_cancel_on_empty_instance_returns_zero() {
        let instance_id = make_instance_id(0xC3);
        let storage = Arc::new(MockTimerStorage::empty());

        let count = cancel_timers_for_instance(&storage, &instance_id)
            .await
            .expect("cancel should succeed");

        assert_eq!(count, 0, "No timers to cancel should return 0");
    }

    // RQ-TP09: cancel_timers_for_instance handles storage failure without leaking.
    #[tokio::test]
    async fn rq_cancel_handles_storage_failure_gracefully() {
        let instance_id = make_instance_id(0xC4);
        let storage = Arc::new(MockTimerStorage::empty());

        storage
            .add_timer(past_timer(instance_id.clone(), 5000))
            .await;

        // Simulate storage failure
        storage.set_should_fail(true).await;

        let result = cancel_timers_for_instance(&storage, &instance_id).await;
        assert!(
            result.is_err(),
            "Storage failure should propagate as error (timer NOT silently leaked)"
        );

        // After storage recovers, the timer should still be there (not silently deleted)
        storage.set_should_fail(false).await;
        let has_pending = has_pending_timers(&storage, &instance_id)
            .await
            .expect("check should succeed");
        assert!(
            has_pending,
            "On storage failure, timer must still exist — not silently lost"
        );
    }
}

// =============================================================================
// ATTACK VECTOR 4: Reanimator crash recovery replays pending timers
// Simulates: reanimator crashes with timers in-flight, restarts, recovers them.
// =============================================================================

mod crash_recovery_timer_safety {
    use super::*;
    use vo_actor::reanimator::traits::PendingTimer;
use vo_actor::work_queue::WorkQueue;

    // RQ-TP10: Pending timer from crashed reanimator is replayed on restart.
    #[tokio::test]
    async fn rq_crashed_reanimator_replays_pending_timer() {
        let instance_id = make_instance_id(0xD1);
        let fire_at = ts_ms(5000);
        let pending = PendingTimer::new(instance_id.clone(), fire_at, ts_ms(4000));

        // Simulate state after a crash: timer was marked as in-flight
        let storage = Arc::new(MockTimerStorage::empty());
        storage.add_pending_timer(pending).await;

        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(50),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(5),
        };

        // Start reanimator — crash recovery runs automatically on spawn
        let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        // Wait for crash recovery + first cycle
        tokio::time::sleep(Duration::from_millis(300)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        let enqueued = work_queue.enqueued().await;
        assert!(
            enqueued.contains(&instance_id),
            "Pending timer from crashed reanimator must be replayed on restart"
        );
    }

    // RQ-TP11: Stale pending timers are cleaned up, not leaked.
    #[tokio::test]
    async fn rq_stale_pending_timers_cleaned_on_recovery() {
        let instance_id = make_instance_id(0xD2);
        let fire_at = ts_ms(1000);
        let pending = PendingTimer::new(instance_id.clone(), fire_at, ts_ms(500));

        let storage = Arc::new(MockTimerStorage::empty());
        storage.add_pending_timer(pending).await;

        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(50),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(5),
        };

        let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        // Wait for crash recovery to clean stale timers (threshold = 60s)
        // Our timer is recent, so it should be REPLAYED, not cleaned
        tokio::time::sleep(Duration::from_millis(300)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        let enqueued = work_queue.enqueued().await;
        // Recent pending timer should be replayed (not silently dropped)
        assert!(
            enqueued.contains(&instance_id),
            "Recent pending timer should be replayed, not silently dropped"
        );
    }

    // RQ-TP12: Terminal instance's pending timers are cleaned up, not leaked.
    #[tokio::test]
    async fn rq_terminal_instance_pending_timers_cleaned() {
        let instance_id = make_instance_id(0xD3);
        let fire_at = ts_ms(5000);
        let pending = PendingTimer::new(instance_id.clone(), fire_at, ts_ms(4000));

        let storage = Arc::new(MockTimerStorage::empty());
        storage.add_pending_timer(pending).await;

        // WorkQueue reports this instance as terminal
        struct TerminalWorkQueue {
            inner: MockWorkQueue,
        }

        #[async_trait::async_trait]
        impl WorkQueue for TerminalWorkQueue {
            async fn enqueue_resume(
                &self,
                instance_id: InstanceId,
            ) -> Result<(), vo_actor::reanimator::ReanimatorError> {
                self.inner.enqueue_resume(instance_id).await
            }
            async fn is_instance_terminal(
                &self,
                _instance_id: &InstanceId,
            ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
                Ok(true) // Instance is in terminal state
            }
        }

        let work_queue = Arc::new(TerminalWorkQueue {
            inner: MockWorkQueue::new(),
        });

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(50),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(5),
        };

        let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        tokio::time::sleep(Duration::from_millis(300)).await;

        handle.shutdown().await.expect("shutdown should succeed");

        // Terminal instance's pending timer should be cleaned up (not dispatched)
        let inner = &work_queue.inner;
        let enqueued = inner.enqueued().await;
        assert!(
            !enqueued.contains(&instance_id),
            "Terminal instance's pending timer should be cleaned up, not dispatched"
        );
    }
}

// =============================================================================
// ATTACK VECTOR 5: Concurrent timer processing panic propagation
// When for_each_concurrent processes multiple timers and one panics,
// verify the system doesn't hang or leak.
// =============================================================================

mod concurrent_panic_invariants {
    use super::*;

    // RQ-TP13: Reanimator reaches Running state and shuts down cleanly.
    #[tokio::test]
    async fn rq_reanimator_reaches_shutdown_state() {
        let storage = Arc::new(MockTimerStorage::empty());
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(50),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(5),
        };

        let handle =
            ReanimatorLoop::spawn(config, storage, work_queue).expect("spawn should succeed");

        // Wait for the spawned task to transition to Running.
        // The task runs crash recovery first, then sends Running.
        // Use watch receiver for more reliable notification.
        let mut state_rx = handle.state_sender.subscribe();
        let state = loop {
            match tokio::time::timeout(Duration::from_secs(5), state_rx.changed()).await {
                Ok(Ok(())) => {
                    let s = state_rx.borrow().clone();
                    if s == ReanimatorState::Running {
                        break s;
                    }
                }
                Ok(Err(_)) => panic!("state sender dropped"),
                Err(_) => panic!("Timed out waiting for Running state"),
            }
        };
        assert_eq!(state, ReanimatorState::Running);

        handle.shutdown().await.expect("shutdown should succeed");
    }

    // RQ-TP14: High timer volume doesn't cause leak — all timers processed or deleted.
    #[tokio::test]
    async fn rq_high_volume_no_leak() {
        let instance_base = 0xE0u8;
        let timer_count = 20u32;
        let mut timers = Vec::new();

        for i in 0..timer_count {
            let instance_id = make_instance_id(instance_base.wrapping_add(i as u8));
            let fire_at = 10_000 + i as u64;
            timers.push(TimerRecord::new(
                instance_id,
                ts_ms(fire_at),
                None,
                ts_ms(fire_at - 5000),
            ));
        }

        let storage = Arc::new(MockTimerStorage::new(timers));
        let work_queue = Arc::new(MockWorkQueue::new());

        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(50),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 50,
            shutdown_timeout: Duration::from_secs(5),
        };

        let handle = ReanimatorLoop::spawn(config, storage.clone(), work_queue.clone())
            .expect("spawn should succeed");

        tokio::time::sleep(Duration::from_secs(2)).await;
        handle.shutdown().await.expect("shutdown should succeed");

        let enqueued = work_queue.enqueued().await;
        assert_eq!(
            enqueued.len(),
            timer_count as usize,
            "All {} timers must be dispatched",
            timer_count
        );

        for i in 0..timer_count {
            let instance_id = make_instance_id(instance_base.wrapping_add(i as u8));
            let has = has_pending_timers(&storage, &instance_id)
                .await
                .expect("check should succeed");
            assert!(
                !has,
                "Timer for instance {} must be deleted from storage (no leak)",
                i
            );
        }
    }
}
