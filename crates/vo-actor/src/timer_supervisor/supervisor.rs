//! Timer supervisor actor implementation
//!
//! Contains the main TimerSupervisor and TimerSupervisorHandle structs.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, watch};
use tokio::task::JoinHandle;
use tokio::time::{interval, MissedTickBehavior};

use vo_types::InstanceId;

use super::calc::{is_overdue, timer_delete_before_dispatch, verify_dual_clock};
use super::traits::{TimerStorage, WorkQueue};
use super::types::{CycleResult, TimerRecord, TimerSupervisorError, TimerSupervisorMetrics, TimerSupervisorState};

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
    pub fn process_cycle(&self) -> Result<CycleResult, TimerSupervisorError> {
        #[allow(clippy::cast_possible_truncation)]
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        #[allow(clippy::cast_possible_truncation)]
        let tick_interval_ms = self.tick_interval.as_millis() as u64;

        // Scan for due timers
        let due_timers = self
            .storage
            .scan_due_timers(0, now_ms, 100)
            .into_iter()
            .filter(|timer| {
                verify_dual_clock(
                    timer.fire_at_ms,
                    timer.trigger_time_ms,
                    timer.duration_ms,
                    now_ms,
                )
            })
            .collect::<Vec<_>>();

        let mut timers_fired = 0u32;
        let mut overdue_count = 0u32;
        let mut error_count = 0u32;

        for timer in due_timers {
            // Check if overdue
            if is_overdue(timer.fire_at_ms, now_ms, tick_interval_ms) {
                self.metrics.overdue_timers.incr();
                overdue_count += 1;
            }

            // Delete before dispatch (INV-2)
            match timer_delete_before_dispatch(&self.storage, &timer) {
                Ok(()) => {
                    // Dispatch succeeded
                    match self.work_queue.enqueue_resume(timer.instance_id.clone()) {
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
                                fire_at_ms = timer.fire_at_ms,
                                error = %e,
                                "Timer deleted but dispatch failed - attempting retry"
                            );
                            let retry_fire_at_ms = now_ms.saturating_add(1000);
                            if let Err(retry_err) = self.storage.retry_timer(&timer, retry_fire_at_ms) {
                                tracing::error!(
                                    instance_id = %timer.instance_id,
                                    fire_at_ms = timer.fire_at_ms,
                                    retry_fire_at_ms = retry_fire_at_ms,
                                    error = %retry_err,
                                    "CRITICAL: Timer deleted but dispatch failed AND retry failed - timer permanently lost"
                                );
                            } else {
                                tracing::warn!(
                                    instance_id = %timer.instance_id,
                                    fire_at_ms = timer.fire_at_ms,
                                    retry_fire_at_ms = retry_fire_at_ms,
                                    "Timer recovered via retry queue after dispatch failure"
                                );
                            }
                        }
                    }
                }
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
                    match self.process_cycle() {
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

            match tokio::time::timeout(remaining, receiver.wait_for(|state| *state != TimerSupervisorState::Running)).await {
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
    use crate::timer_supervisor::traits::{TimerStorage, WorkQueue};

    struct MockStorage;
    impl TimerStorage for MockStorage {
        fn scan_due_timers(&self, _from: u64, _to: u64, _max: u32) -> Vec<TimerRecord> {
            Vec::new()
        }
        fn delete_timer(
            &self,
            _instance_id: &InstanceId,
            _fire_at_ms: u64,
        ) -> Result<(), TimerSupervisorError> {
            Ok(())
        }
        fn retry_timer(
            &self,
            _timer: &TimerRecord,
            _new_fire_at_ms: u64,
        ) -> Result<(), TimerSupervisorError> {
            Ok(())
        }
    }

    struct MockQueue;
    impl WorkQueue for MockQueue {
        fn enqueue_resume(&self, _instance_id: InstanceId) -> Result<(), TimerSupervisorError> {
            Ok(())
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
        assert!(result.is_ok(), "shutdown should return Ok, got {:?}", result);
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
