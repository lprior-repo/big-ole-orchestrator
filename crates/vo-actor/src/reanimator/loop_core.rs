//! Reanimator Loop core: background task spawn and cycle processing.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use tokio::sync::{broadcast, watch};
use tokio::task::JoinHandle;
use tokio::time::{interval, MissedTickBehavior};
use vo_types::TimestampMs;

use crate::reanimator::{
    traits::TimerStorage,
    types::{validate_timer_record, FairnessBudget, ReanimatorConfig, ReanimatorState},
    ReanimatorError,
};
use crate::work_queue::WorkQueue;

const STALE_PENDING_THRESHOLD_MS: u64 = 60_000;

/// Handle for controlling the Reanimator Loop.
#[derive(Debug)]
pub struct ReanimatorHandle {
    pub state_sender: watch::Sender<ReanimatorState>,
    pub(crate) shutdown_trigger: broadcast::Sender<()>,
    pub(crate) task_handle: Option<JoinHandle<()>>,
}

impl Clone for ReanimatorHandle {
    fn clone(&self) -> Self {
        Self {
            state_sender: self.state_sender.clone(),
            shutdown_trigger: self.shutdown_trigger.clone(),
            task_handle: None,
        }
    }
}

impl ReanimatorHandle {
    /// Requests the reanimator to shut down.
    #[tracing::instrument(skip(self))]
    pub async fn shutdown(mut self) -> Result<(), ReanimatorError> {
        // Signal shutdown
        let _ = self.shutdown_trigger.send(());

        // Wait for state change to Shutdown
        let mut receiver = self.state_sender.subscribe();
        loop {
            match receiver.changed().await {
                Ok(()) => {
                    let state = (*receiver.borrow()).clone();
                    match state {
                        ReanimatorState::ShutDown => break,
                        ReanimatorState::ShuttingDown => continue,
                        _ => {
                            return Err(ReanimatorError::AtomicityViolation(format!(
                                "Unexpected state during shutdown: {:?}",
                                state
                            )));
                        }
                    }
                }
                Err(_) => {
                    return Err(ReanimatorError::AlreadyShutdown);
                }
            }
        }

        // Await the background task to ensure clean exit
        if let Some(task) = self.task_handle.take() {
            match task.await {
                Ok(()) => {}
                Err(e) => {
                    if !e.is_panic() {
                        tracing::warn!("Reanimator task cancelled during shutdown");
                    } else {
                        tracing::error!("Reanimator task panicked during shutdown");
                    }
                }
            }
        }

        Ok(())
    }

    /// Gets the current state of the reanimator.
    #[must_use]
    pub fn current_state(&self) -> ReanimatorState {
        self.state_sender.borrow().clone()
    }
}

/// The Reanimator Loop background task.
pub struct ReanimatorLoop;

impl ReanimatorLoop {
    /// Spawns the Reanimator Loop as a background task.
    ///
    /// # Errors
    /// Returns `ReanimatorError::AlreadyRunning` if a reanimator is already running.
    pub fn spawn<S, Q>(
        config: ReanimatorConfig,
        storage: Arc<S>,
        work_queue: Arc<Q>,
    ) -> Result<ReanimatorHandle, ReanimatorError>
    where
        S: TimerStorage + 'static,
        Q: WorkQueue + 'static,
    {
        let (state_sender, _) = watch::channel(ReanimatorState::Stopped);
        let (shutdown_trigger, _) = broadcast::channel(1);

        let state_sender_clone = state_sender.clone();
        let shutdown_receiver = shutdown_trigger.subscribe();

        // Create a receiver to ensure send() succeeds - must be kept alive
        let _state_receiver = state_sender.subscribe();

        // Spawn the background task
        // Crash recovery runs inside the task before entering the main loop
        let task_handle = tokio::runtime::Handle::current().spawn(async move {
            // Run crash recovery before starting the loop
            // This ensures any pending timers from a previous crash are replayed
            if let Err(e) = Self::run_crash_recovery(&storage, &work_queue).await {
                tracing::warn!("Crash recovery completed with error: {}", e);
            }

            // Transition to Running now that there's an active receiver
            let _ = state_sender_clone.send(ReanimatorState::Running);

            let result = Self::run_loop_inner(
                config,
                storage,
                work_queue,
                state_sender_clone,
                shutdown_receiver,
            )
            .await;
            if let Err(e) = result {
                tracing::error!("Reanimator loop exited with error: {}", e);
            }
        });

        let handle = ReanimatorHandle {
            state_sender,
            shutdown_trigger: shutdown_trigger.clone(),
            task_handle: Some(task_handle),
        };

        Ok(handle)
    }

    /// Runs crash recovery to detect and replay pending timers from a previous crash.
    ///
    /// This method:
    /// 1. Scans for pending timers (timers that were in-flight when crash occurred)
    /// 2. Cleans up stale pending timers (older than STALE_PENDING_THRESHOLD_MS)
    /// 3. Replays pending timers by enqueueing resume work
    #[allow(clippy::expect_used)]
    pub async fn run_crash_recovery<S, Q>(
        storage: &Arc<S>,
        work_queue: &Arc<Q>,
    ) -> Result<(), ReanimatorError>
    where
        S: TimerStorage + 'static,
        Q: WorkQueue + 'static,
    {
        tracing::info!("Running crash recovery...");

        // First, clean up any stale pending timers from previous crashes
        let stale_threshold = TimestampMs::try_from(
            TimestampMs::now()
                .as_u64()
                .saturating_sub(STALE_PENDING_THRESHOLD_MS),
        )
        .unwrap_or_else(|_| TimestampMs::try_from(0u64).expect("0 is valid"));

        let cleaned = storage
            .cleanup_stale_pending_timers(stale_threshold)
            .await?;

        if cleaned > 0 {
            tracing::info!("Cleaned up {} stale pending timers", cleaned);
        }

        // Scan for pending timers that need to be replayed
        let pending_timers = storage.scan_pending_timers(100).await?;

        if pending_timers.is_empty() {
            tracing::info!("No pending timers found during crash recovery");
            return Ok(());
        }

        tracing::info!("Found {} pending timers to replay", pending_timers.len());

        // Replay each pending timer
        for pending in pending_timers {
            // Check if instance is in a terminal state before replaying
            match work_queue.is_instance_terminal(&pending.instance_id).await {
                Ok(true) => {
                    tracing::info!(
                        instance_id = %pending.instance_id,
                        "Skipping timer replay: instance is in terminal state"
                    );
                    // Clean up the pending timer since instance is dead
                    if let Err(e) = storage
                        .complete_timer_processing(&pending.instance_id, pending.fire_at_ms)
                        .await
                    {
                        tracing::warn!(
                            instance_id = %pending.instance_id,
                            error = %e,
                            "Failed to clean up pending timer for terminal instance"
                        );
                    }
                    continue;
                }
                Ok(false) => {
                    // Instance is active, proceed with replay
                }
                Err(e) => {
                    tracing::warn!(
                        instance_id = %pending.instance_id,
                        error = %e,
                        "Failed to check instance state, skipping timer replay"
                    );
                    continue;
                }
            }

            tracing::info!(
                instance_id = %pending.instance_id,
                fire_at_ms = %pending.fire_at_ms,
                "Replaying pending timer"
            );

            // Try to enqueue resume work
            match work_queue.enqueue_resume(pending.instance_id.clone()).await {
                Ok(()) => {
                    // Successfully replayed, mark as complete
                    if let Err(e) = storage
                        .complete_timer_processing(&pending.instance_id, pending.fire_at_ms)
                        .await
                    {
                        tracing::warn!(
                            instance_id = %pending.instance_id,
                            error = %e,
                            "Failed to complete timer processing after replay"
                        );
                    }
                }
                Err(e) => {
                    tracing::error!(
                        instance_id = %pending.instance_id,
                        error = %e,
                        "Failed to replay pending timer"
                    );
                }
            }
        }

        tracing::info!("Crash recovery completed");
        Ok(())
    }

    /// The main loop implementation.
    #[tracing::instrument(skip_all)]
    async fn run_loop_inner<S, Q>(
        config: ReanimatorConfig,
        storage: Arc<S>,
        work_queue: Arc<Q>,
        state_sender: watch::Sender<ReanimatorState>,
        mut shutdown_receiver: broadcast::Receiver<()>,
    ) -> Result<(), ReanimatorError>
    where
        S: TimerStorage + 'static,
        Q: WorkQueue + 'static,
    {
        // Transition to Running
        let _ = state_sender.send(ReanimatorState::Running);

        let mut scan_interval = interval(config.scan_interval);
        scan_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        let mut budget = FairnessBudget::with_limits(
            config.max_timers_per_cycle,
            config.max_timers_per_cycle * config.max_concurrent_resumes,
        );

        let mut max_already_processed = 0u32;

        loop {
            tokio::select! {
                _ = shutdown_receiver.recv() => {
                    let _ = state_sender.send(ReanimatorState::ShuttingDown);
                    break;
                }
                _ = scan_interval.tick() => {
                    // Perform one scan cycle
                    match Self::process_cycle(&config, &storage, &work_queue, &mut budget, max_already_processed).await {
                        Ok(processed) => {
                            max_already_processed = processed;
                        }
                        Err(e) if e.is_transient() => {
                            tracing::warn!("Transient error in reanimator cycle: {}", e);
                        }
                        Err(e) if e.is_fatal() => {
                            tracing::error!("Fatal error in reanimator cycle: {}", e);
                        }
                        Err(e) => {
                            tracing::error!("Unknown error in reanimator cycle: {}", e);
                        }
                    }
                }
            }
        }

        let _ = state_sender.send(ReanimatorState::ShutDown);
        Ok(())
    }

    /// Processes a single scan cycle.
    ///
    /// Uses delete-before-dispatch ordering (INV-2): timer is deleted from
    /// storage BEFORE any dispatch occurs. If dispatch fails after delete,
    /// the timer is lost but no double-fire is possible.
    #[tracing::instrument(skip_all, fields(processed, failed_count))]
    async fn process_cycle<S, Q>(
        config: &ReanimatorConfig,
        storage: &Arc<S>,
        work_queue: &Arc<Q>,
        budget: &mut FairnessBudget,
        max_already_processed: u32,
    ) -> Result<u32, ReanimatorError>
    where
        S: TimerStorage + 'static,
        Q: WorkQueue + 'static,
    {
        let current_time = vo_types::TimestampMs::now();

        // Scan for due timers
        let scan_result = storage
            .scan_due_timers(
                #[allow(clippy::expect_used)]
                vo_types::TimestampMs::try_from(0u64).expect("0 is valid TimestampMs"),
                current_time,
                config.max_timers_per_cycle,
            )
            .await?;

        // Reset budget for this cycle
        budget.reset();

        // No dedup - let all timers through
        let deduped_timers = scan_result;

        let concurrency_limit = config.max_concurrent_resumes as usize;
        let storage_ref = storage.clone();
        let work_queue_ref = work_queue.clone();

        let processed = Arc::new(AtomicU32::new(0));
        let failed_count = Arc::new(AtomicU32::new(0));

        use futures::StreamExt;
        futures::stream::iter(
            deduped_timers
                .into_iter()
                .take(config.max_timers_per_cycle as usize),
        )
        .filter(|timer| {
            let valid = validate_timer_record(timer).is_ok();
            let within_budget = budget.can_resume(&timer.instance_id);
            std::future::ready(valid && within_budget)
        })
        .for_each_concurrent(concurrency_limit, |timer| {
            let storage = storage_ref.clone();
            let work_queue = work_queue_ref.clone();
            let processed = Arc::clone(&processed);
            let failed_count = Arc::clone(&failed_count);
            async move {
                let delete_result = storage
                    .delete_timer(&timer.instance_id, timer.fire_at_ms)
                    .await;

                match delete_result {
                    Ok(()) => {
                        let record_result = storage
                            .record_timer_fired(&timer.instance_id, timer.fire_at_ms)
                            .await;

                        match record_result {
                            Ok(()) => {
                                match work_queue.enqueue_resume(timer.instance_id.clone()).await {
                                    Ok(()) => {
                                        processed.fetch_add(1, Ordering::Relaxed);
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            instance_id = %timer.instance_id,
                                            error = %e,
                                            "Failed to enqueue resume"
                                        );
                                        failed_count.fetch_add(1, Ordering::Relaxed);
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!(
                                    instance_id = %timer.instance_id,
                                    error = %e,
                                    "Failed to record TimerFired"
                                );
                                failed_count.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(
                            instance_id = %timer.instance_id,
                            error = %e,
                            "Failed to delete timer before dispatch"
                        );
                        failed_count.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        })
        .await;

        let processed = processed.load(Ordering::Relaxed);
        let failed_count = failed_count.load(Ordering::Relaxed);

        let new_max = if processed > 0 {
            0
        } else {
            max_already_processed + processed
        };

        tracing::debug!(processed, failed_count, "Reanimator cycle complete");

        Ok(new_max)
    }
}
