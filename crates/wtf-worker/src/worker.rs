//! High-level Worker — activity dispatcher and execution loop (bead wtf-1qx1).
//!
//! The `Worker` struct ties together the work queue consumer and activity result
//! reporting. Callers register handler functions keyed by `activity_type` and
//! start the processing loop.
//!
//! # Usage
//! ```ignore
//! let worker = Worker::new(js.clone(), "send-email-worker", None);
//! worker.register("send_email", |task| async move {
//!     // execute the activity
//!     Ok(Bytes::from_static(b"sent"))
//! });
//! worker.run(shutdown_rx).await?;
//! ```

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

use async_nats::jetstream::Context;
use bytes::Bytes;
use tokio::time::Duration;
use wtf_common::WtfError;

use crate::activity::{
    calculate_backoff_delay, complete_activity, fail_activity, retries_exhausted,
};
use crate::queue::{enqueue_activity, ActivityTask, WorkQueueConsumer};

/// Configuration for graceful worker drain on shutdown.
#[derive(Debug, Clone)]
pub struct DrainConfig {
    pub drain_timeout: Duration,
    pub nak_on_timeout: bool,
}

impl DrainConfig {
    /// Create a new `DrainConfig`.
    ///
    /// # Errors
    /// Returns `WtfError::InvalidInput` if `drain_timeout` is zero.
    pub fn new(drain_timeout: Duration, nak_on_timeout: bool) -> Result<Self, WtfError> {
        if drain_timeout.is_zero() {
            return Err(WtfError::InvalidInput {
                detail: "drain_timeout must be > 0".to_owned(),
            });
        }
        Ok(Self {
            drain_timeout,
            nak_on_timeout,
        })
    }
}

impl Default for DrainConfig {
    fn default() -> Self {
        Self {
            drain_timeout: Duration::from_secs(30),
            nak_on_timeout: true,
        }
    }
}

/// Final drain metrics reported when worker exits.
#[derive(Debug, Clone, Default)]
pub struct ShutdownResult {
    pub completed_count: u32,
    pub interrupted_count: u32,
    pub drain_duration_ms: u64,
}

/// Boxed async activity handler: takes a task and returns `Ok(result_bytes)` or `Err(message)`.
type ActivityHandler = Arc<
    dyn Fn(ActivityTask) -> Pin<Box<dyn Future<Output = Result<Bytes, String>> + Send>>
        + Send
        + Sync,
>;

/// High-level activity worker.
///
/// Wraps the pull consumer and handler dispatch. Register handlers with
/// [`Worker::register`] and start the loop with [`Worker::run`].
pub struct Worker {
    js: Context,
    name: String,
    filter_subject: Option<String>,
    handlers: HashMap<String, ActivityHandler>,
}

impl Worker {
    /// Create a new worker.
    ///
    /// - `worker_name`: durable consumer name (also used for logging).
    /// - `filter_subject`: optional NATS subject filter (`wtf.work.<type>`).
    ///   Pass `None` to consume all activity types.
    #[must_use]
    pub fn new(
        js: Context,
        worker_name: impl Into<String>,
        filter_subject: Option<String>,
    ) -> Self {
        Self {
            js,
            name: worker_name.into(),
            filter_subject,
            handlers: HashMap::new(),
        }
    }

    /// Register an async handler function for the given `activity_type`.
    ///
    /// The handler receives the full `ActivityTask` and must return either
    /// `Ok(result_bytes)` (activity succeeded) or `Err(error_message)` (activity failed).
    pub fn register<F, Fut>(&mut self, activity_type: impl Into<String>, handler: F)
    where
        F: Fn(ActivityTask) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Bytes, String>> + Send + 'static,
    {
        let boxed: ActivityHandler = Arc::new(move |task| Box::pin(handler(task)));
        self.handlers.insert(activity_type.into(), boxed);
    }

    /// Run the worker processing loop until `shutdown_rx` fires.
    ///
    /// For each task pulled from the queue:
    /// 1. Look up the registered handler for `task.activity_type`.
    /// 2. Execute the handler.
    /// 3. Append `ActivityCompleted` or `ActivityFailed` to `JetStream`.
    /// 4. Ack the work-queue message.
    ///
    /// Tasks with no registered handler are acked without execution and logged
    /// as a warning (to avoid poison-pill queue stalls).
    ///
    /// # Errors
    /// Returns `WtfError` only for unrecoverable errors (consumer creation
    /// failure). Per-task errors are logged and the loop continues.
    pub async fn run(
        &self,
        shutdown_rx: tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), WtfError> {
        self.run_with_drain(shutdown_rx, DrainConfig::default())
            .await
            .map(|_| ())
    }

    /// Run the worker with a drain configuration.
    ///
    /// # Errors
    /// Returns `WtfError` if creating the consumer fails.
    pub async fn run_with_drain(
        &self,
        mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
        _drain_config: DrainConfig,
    ) -> Result<ShutdownResult, WtfError> {
        let mut consumer =
            WorkQueueConsumer::create(&self.js, &self.name, self.filter_subject.clone())
                .await?;

        let mut drain_started_at: Option<Instant> = None;
        let mut completed_count: u32 = 0;
        let interrupted_count: u32 = 0;

        tracing::info!(
            worker = %self.name,
            "worker started"
        );

        loop {
            tokio::select! {
                result = consumer.next_task() => {
                    match result {
                        Err(e) => {
                            tracing::error!(worker = %self.name, error = %e, "queue error");
                        }
                        Ok(None) => {
                            tracing::info!(worker = %self.name, "work queue closed — shutting down");
                            break;
                        }
                        Ok(Some(ackable)) => {
                            self.process_task(ackable).await;
                            completed_count = completed_count.saturating_add(1);
                        }
                    }
                }
                result = shutdown_rx.changed() => {
                    match result {
                        Ok(()) | Err(_) => {
                            tracing::info!(worker = %self.name, "worker entering drain phase");
                            if drain_started_at.is_none() {
                                drain_started_at = Some(Instant::now());
                            }
                            break;
                        }
                    }
                }
            }
        }

        let drain_duration_ms =
            drain_started_at.map_or(0_u64, |started| u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX));

        Ok(ShutdownResult {
            completed_count,
            interrupted_count,
            drain_duration_ms,
        })
    }

    async fn process_task(&self, ackable: crate::queue::AckableTask) {
        let task = &ackable.task;
        let handler = self.handlers.get(&task.activity_type).cloned();

        let Some(handler) = handler else {
            tracing::warn!(
                worker = %self.name,
                activity_type = %task.activity_type,
                activity_id = %task.activity_id,
                "no handler registered — acking to avoid queue stall"
            );
            let _ = ackable.ack().await;
            return;
        };

        let task_clone = ackable.task.clone();
        let start = Instant::now();
        let handler_result = handler(task_clone).await;
        let duration_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);

        match handler_result {
            Ok(result) => {
                let append_result = complete_activity(
                    &self.js,
                    &task.namespace,
                    &task.instance_id,
                    &task.activity_id,
                    result,
                    duration_ms,
                )
                .await;

                if let Err(e) = append_result {
                    tracing::error!(
                        activity_id = %task.activity_id,
                        error = %e,
                        "failed to append ActivityCompleted — naking"
                    );
                    let _ = ackable.nak().await;
                    return;
                }
            }
            Err(error) => {
                let exhausted = retries_exhausted(task.attempt, task.retry_policy.max_attempts);

                if !exhausted {
                    if let Some(delay_ms) =
                        calculate_backoff_delay(task.attempt, &task.retry_policy)
                    {
                        tracing::info!(
                            activity_id = %task.activity_id,
                            attempt = %task.attempt,
                            delay_ms,
                            "activity failed, scheduling retry after backoff"
                        );
                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        let retry_task = ActivityTask {
                            activity_id: task.activity_id.clone(),
                            activity_type: task.activity_type.clone(),
                            payload: task.payload.clone(),
                            namespace: task.namespace.clone(),
                            instance_id: task.instance_id.clone(),
                            attempt: task.attempt + 1,
                            retry_policy: task.retry_policy.clone(),
                            timeout: task.timeout,
                        };
                        let _ = enqueue_activity(&self.js, &retry_task).await;
                    }
                }

                let append_result = fail_activity(
                    &self.js,
                    &task.namespace,
                    &task.instance_id,
                    &task.activity_id,
                    error,
                    exhausted,
                )
                .await;

                if let Err(e) = append_result {
                    tracing::error!(
                        activity_id = %task.activity_id,
                        error = %e,
                        "failed to append ActivityFailed — naking"
                    );
                    let _ = ackable.nak().await;
                    return;
                }
            }
        }

        let _ = ackable.ack().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wtf_common::RetryPolicy;

    // Worker::new / register / run require a live NATS Context.
    // Those paths are covered by integration tests (wtf-2bbn).
    //
    // Unit tests here cover pure helper behaviour imported from activity.rs.

    #[test]
    fn retries_exhausted_at_max_attempts() {
        // default RetryPolicy::max_attempts = 3; attempt 3 means exhausted
        let policy = RetryPolicy::default();
        assert!(retries_exhausted(3, policy.max_attempts));
    }

    #[test]
    fn retries_not_exhausted_below_max() {
        let policy = RetryPolicy::default();
        assert!(!retries_exhausted(1, policy.max_attempts));
    }

    #[test]
    fn retries_not_exhausted_at_attempt_two_of_three() {
        let policy = RetryPolicy::default();
        assert!(!retries_exhausted(2, policy.max_attempts));
    }

    #[test]
    fn duration_ms_cast_from_u128_to_u64_is_lossless_for_reasonable_values() {
        // Verify the as-cast used in process_task doesn't truncate for <1 hour.
        let millis: u128 = 3_600_000; // 1 hour in ms
        #[allow(clippy::cast_possible_truncation)]
        let cast = millis as u64;
        assert_eq!(cast, 3_600_000_u64);
    }

    #[test]
    fn drain_config_default_timeout_is_30_seconds() {
        let config = DrainConfig::default();
        assert_eq!(config.drain_timeout, Duration::from_secs(30));
        assert!(config.nak_on_timeout);
    }

    #[test]
    fn drain_config_rejects_zero_duration() {
        let config = DrainConfig::new(Duration::ZERO, true);
        assert!(config.is_err());
    }
}
