//! Timer supervisor actor implementation
//!
//! Contains the main TimerSupervisor and TimerSupervisorHandle structs.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::{broadcast, watch};
use tokio::task::JoinHandle;
use tokio::time::{interval, MissedTickBehavior};

use vo_types::{InstanceId, TimestampMs};

use super::calc::{is_overdue, verify_dual_clock};
use super::traits::WorkQueue;
use super::types::{
    CycleResult, TimerSupervisorError, TimerSupervisorMetrics, TimerSupervisorState,
};
use vo_common::timer_storage::TimerStorage;

// =============================================================================
// `TimerSupervisor` - Actor that manages timer scanning and dispatch
// =============================================================================

/// `TimerSupervisor` - Actor that manages timer scanning and dispatch
pub struct TimerSupervisor {
    /// Interval between timer scans.
    pub tick_interval: Duration,
    /// Storage for timers.
    pub storage: Arc<dyn TimerStorage>,
    /// Work queue for dispatching.
    pub work_queue: Arc<dyn WorkQueue>,
    /// Metrics.
    pub metrics: TimerSupervisorMetrics,
    /// Running state.
    is_running: std::sync::atomic::AtomicBool,
}

impl std::fmt::Debug for TimerSupervisor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TimerSupervisor")
            .field("tick_interval", &self.tick_interval)
            .finish_non_exhaustive()
    }
}

impl TimerSupervisor {
    /// Creates a new `TimerSupervisor`.
    ///
    /// # Errors
    /// Returns `InvalidConfig` if `tick_interval` is zero.
    pub fn new(
        tick_interval: Duration,
        storage: Arc<dyn TimerStorage>,
        work_queue: Arc<dyn WorkQueue>,
    ) -> Result<Self, TimerSupervisorError> {
        // Precondition: tick_interval > 0
        if tick_interval.is_zero() {
            return Err(TimerSupervisorError::InvalidConfig(
                "tick_interval must be > 0".to_string(),
            ));
        }

        Ok(Self {
            tick_interval,
            storage,
            work_queue,
            metrics: TimerSupervisorMetrics::default(),
            is_running: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// Spawns the `TimerSupervisor` background task.
    ///
    /// # Errors
    /// Returns `AlreadyRunning` if the supervisor is already running.
    pub fn spawn(self) -> Result<TimerSupervisorHandle, TimerSupervisorError> {
        if self
            .is_running
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            return Err(TimerSupervisorError::AlreadyRunning);
        }

        let (state_sender, _) = watch::channel(TimerSupervisorState::Running);
        let (shutdown_trigger, _) = broadcast::channel(1);

        let state_sender_clone = state_sender.clone();
        let shutdown_receiver = shutdown_trigger.subscribe();

        let task_handle = tokio::runtime::Handle::current().spawn(async move {
            let result = self.run_loop(state_sender_clone, shutdown_receiver).await;
            if let Err(e) = result {
                tracing::error!("Timer supervisor loop exited with error: {}", e);
            }
        });

        Ok(TimerSupervisorHandle {
            state_sender,
            shutdown_trigger,
            task_handle: Some(task_handle),
        })
    }

    /// Processes one timer scan cycle.
    ///
    /// Scans storage for due timers, deletes them before dispatch, and enqueues
    /// resume work for each instance.
    ///
    /// # Errors
    /// Returns an error if storage operations fail.
    pub async fn process_cycle(&self) -> Result<CycleResult, TimerSupervisorError> {
        let now_ms = TimestampMs::now();
        let tick_interval_ms = self.tick_interval.as_millis() as u64;

        let from_ms = TimestampMs::new_unchecked(0);

        let due_timers = self
            .storage
            .list_expired_timers(from_ms, now_ms, 100)
            .await
            .map_err(|e| TimerSupervisorError::StorageAdapterError(e.to_string()))?
            .into_iter()
            .filter(|timer| verify_dual_clock(timer.fire_at_ms, now_ms))
            .collect::<Vec<_>>();

        let mut timers_fired = 0u32;
        let mut overdue_count = 0u32;
        let mut error_count = 0u32;

        for timer in due_timers {
            if is_overdue(timer.fire_at_ms, now_ms, tick_interval_ms) {
                self.metrics.overdue_timers.incr();
                overdue_count += 1;
            }

            let fire_at_ms = timer.fire_at_ms;

            let delete_result = self
                .storage
                .cancel_timer(&timer.instance_id, fire_at_ms)
                .await;

            match delete_result {
                Ok(()) => match self.work_queue.enqueue_resume(timer.instance_id.clone()) {
                    Ok(()) => {
                        self.metrics.timers_fired.incr();
                        timers_fired += 1;
                    }
                    Err(e) => {
                        self.metrics.dispatch_errors.incr();
                        self.metrics.timer_deleted_but_dispatch_failed.incr();
                        error_count += 1;
                        tracing::error!(
                            instance_id = %timer.instance_id,
                            fire_at_ms = %fire_at_ms,
                            error = %e,
                            "Timer deleted but dispatch failed - attempting retry"
                        );
                        let retry_fire_at_ms =
                            TimestampMs::new_unchecked(now_ms.as_u64().saturating_add(1000));
                        if let Err(retry_err) =
                            self.storage.retry_timer(&timer, retry_fire_at_ms).await
                        {
                            tracing::error!(
                                instance_id = %timer.instance_id,
                                fire_at_ms = %fire_at_ms,
                                retry_fire_at_ms = %retry_fire_at_ms,
                                error = %retry_err,
                                "CRITICAL: Timer deleted but dispatch failed AND retry failed - timer permanently lost"
                            );
                        } else {
                            tracing::warn!(
                                instance_id = %timer.instance_id,
                                fire_at_ms = %fire_at_ms,
                                retry_fire_at_ms = %retry_fire_at_ms,
                                "Timer recovered via retry queue after dispatch failure"
                            );
                        }
                    }
                },
                Err(e) => {
                    self.metrics.dispatch_errors.incr();
                    error_count += 1;
                    tracing::error!(
                        instance_id = %timer.instance_id,
                        error = %e,
                        "Failed to delete timer before dispatch"
                    );
                }
            }
        }

        Ok(CycleResult {
            timers_fired,
            overdue_count,
            error_count,
        })
    }

    async fn run_loop(
        self,
        state_sender: watch::Sender<TimerSupervisorState>,
        mut shutdown_receiver: broadcast::Receiver<()>,
    ) -> Result<(), TimerSupervisorError> {
        let mut scan_interval = interval(self.tick_interval);
        scan_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = shutdown_receiver.recv() => {
                    let _ = state_sender.send(TimerSupervisorState::ShuttingDown);
                    break;
                }
                _ = scan_interval.tick() => {
                    match self.process_cycle().await {
                        Ok(_) => {}
                        Err(e) if e.is_transient() => {
                            tracing::warn!("Transient error in timer supervisor cycle: {}", e);
                        }
                        Err(e) if e.is_fatal() => {
                            tracing::error!("Fatal error in timer supervisor cycle: {}", e);
                        }
                        Err(e) => {
                            tracing::debug!("Operational error in timer supervisor cycle: {}", e);
                        }
                    }
                }
            }
        }

        let _ = state_sender.send(TimerSupervisorState::ShutDown);
        Ok(())
    }
}

// =============================================================================
// `TimerSupervisorHandle` - Handle for controlling `TimerSupervisor`
// =============================================================================

/// Handle for controlling `TimerSupervisor`
#[derive(Debug)]
pub struct TimerSupervisorHandle {
    state_sender: watch::Sender<TimerSupervisorState>,
    shutdown_trigger: broadcast::Sender<()>,
    task_handle: Option<JoinHandle<()>>,
}

impl TimerSupervisorHandle {
    /// Returns true if the supervisor is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        *self.state_sender.borrow() == TimerSupervisorState::Running
    }

    /// Returns the current state of the supervisor.
    #[must_use]
    pub fn current_state(&self) -> TimerSupervisorState {
        self.state_sender.borrow().clone()
    }

    /// Requests the supervisor to shut down and waits for completion.
    ///
    /// # Errors
    /// Returns `ShutdownTimeout` if shutdown does not complete within the given timeout.
    pub async fn shutdown(mut self, timeout: Duration) -> Result<(), TimerSupervisorError> {
        let _ = self.shutdown_trigger.send(());

        let mut receiver = self.state_sender.subscribe();
        let start = std::time::Instant::now();
        loop {
            let remaining = timeout.saturating_sub(start.elapsed());
            if remaining.is_zero() {
                return Err(TimerSupervisorError::ShutdownTimeout(timeout));
            }

            match tokio::time::timeout(
                remaining,
                receiver.wait_for(|state| *state != TimerSupervisorState::Running),
            )
            .await
            {
                Ok(Ok(state)) => {
                    if *state == TimerSupervisorState::ShutDown {
                        break;
                    }
                }
                _ => {
                    return Err(TimerSupervisorError::ShutdownTimeout(timeout));
                }
            }
        }

        if let Some(task) = self.task_handle.take() {
            match task.await {
                Ok(()) => {}
                Err(e) => {
                    if !e.is_panic() {
                        tracing::warn!("Timer supervisor task cancelled during shutdown");
                    } else {
                        tracing::error!("Timer supervisor task panicked during shutdown");
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timer_supervisor::traits::WorkQueue;
    use async_trait::async_trait;
    use vo_common::timer_storage::{TimerStorage, TimerStorageError};

    struct MockStorage;
    #[async_trait]
    impl TimerStorage for MockStorage {
        async fn schedule_timer(&self, _record: TimerRecord) -> Result<(), TimerStorageError> {
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
        ) -> Result<TimerRecord, TimerStorageError> {
            Err(TimerStorageError::NotFound {
                instance_id: InstanceId::from_bytes([0u8; 16]),
                fire_at_ms: TimestampMs::new_unchecked(0),
            })
        }
        async fn list_timers_by_instance(
            &self,
            _instance_id: &InstanceId,
        ) -> Result<Vec<TimerRecord>, TimerStorageError> {
            Ok(Vec::new())
        }
        async fn list_expired_timers(
            &self,
            _from: TimestampMs,
            _to: TimestampMs,
            _max: u32,
        ) -> Result<Vec<TimerRecord>, TimerStorageError> {
            Ok(Vec::new())
        }
        async fn retry_timer(
            &self,
            _timer: &TimerRecord,
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

    struct MockQueue;
    #[async_trait::async_trait]
    impl WorkQueue for MockQueue {
        async fn enqueue_spawn(
            &self,
            _instance_id: InstanceId,
            _executable: std::path::PathBuf,
            _args: Vec<String>,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
        async fn enqueue_resume(
            &self,
            _instance_id: InstanceId,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
        async fn is_instance_terminal(
            &self,
            _instance_id: &InstanceId,
        ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
            Ok(false)
        }
    }

    #[tokio::test]
    async fn shutdown_returns_ok_on_clean_shutdown() {
        let storage: Arc<dyn TimerStorage> = Arc::new(MockStorage);
        let work_queue: Arc<dyn WorkQueue> = Arc::new(MockQueue);

        let supervisor = TimerSupervisor::new(Duration::from_millis(100), storage, work_queue)
            .expect("valid config should construct supervisor");

        let handle = supervisor.spawn().expect("spawn should return a handle");
        assert!(handle.is_running());
        assert_eq!(handle.current_state(), TimerSupervisorState::Running);

        let result = handle.shutdown(Duration::from_secs(5)).await;
        assert!(
            result.is_ok(),
            "shutdown should return Ok, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn shutdown_sets_state_to_shutdown() {
        let storage: Arc<dyn TimerStorage> = Arc::new(MockStorage);
        let work_queue: Arc<dyn WorkQueue> = Arc::new(MockQueue);

        let supervisor = TimerSupervisor::new(Duration::from_secs(3600), storage, work_queue)
            .expect("valid config should construct supervisor");

        let handle = supervisor.spawn().expect("spawn should return a handle");
        let state_before = handle.current_state();
        assert_eq!(state_before, TimerSupervisorState::Running);

        handle.shutdown(Duration::from_secs(5)).await.unwrap();
    }
}
