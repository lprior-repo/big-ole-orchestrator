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
use wtf_common::WtfError;

use crate::activity::{complete_activity, fail_activity, retries_exhausted};
use crate::queue::{ActivityTask, WorkQueueConsumer};

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
    worker_name: String,
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
            worker_name: worker_name.into(),
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
    /// 3. Append `ActivityCompleted` or `ActivityFailed` to JetStream.
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
        mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), WtfError> {
        let mut consumer =
            WorkQueueConsumer::create(&self.js, &self.worker_name, self.filter_subject.clone())
                .await?;

        tracing::info!(
            worker = %self.worker_name,
            "worker started"
        );

        loop {
            tokio::select! {
                result = consumer.next_task() => {
                    match result {
                        Err(e) => {
                            tracing::error!(worker = %self.worker_name, error = %e, "queue error");
                        }
                        Ok(None) => {
                            tracing::info!(worker = %self.worker_name, "work queue closed — shutting down");
                            break;
                        }
                        Ok(Some(ackable)) => {
                            self.process_task(ackable).await;
                        }
                    }
                }
                result = shutdown_rx.changed() => {
                    match result {
                        Ok(()) | Err(_) => {
                            tracing::info!(worker = %self.worker_name, "worker shutting down");
                            break;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn process_task(&self, ackable: crate::queue::AckableTask) {
        let task = &ackable.task;
        let handler = self.handlers.get(&task.activity_type).cloned();

        let Some(handler) = handler else {
            tracing::warn!(
                worker = %self.worker_name,
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
        let duration_ms = start.elapsed().as_millis() as u64;

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
}
