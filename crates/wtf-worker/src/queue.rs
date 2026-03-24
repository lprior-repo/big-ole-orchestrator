//! Activity work queue consumer (bead wtf-1qx1).
//!
//! Workers pull `ActivityTask` messages from the `wtf-work` NATS `JetStream`
//! stream. Each task encapsulates everything the worker needs to execute the
//! activity and report the result.
//!
//! # Work queue flow
//! 1. Actor appends `ActivityDispatched` to `JetStream` (write-ahead — ADR-015).
//! 2. Actor publishes an `ActivityTask` payload to `wtf.work.<activity_type>`.
//! 3. Worker fetches via pull consumer, executes, reports result via `activity.rs`.
//! 4. Worker acks (success/failure) — NATS removes the message from the queue.
//!
//! # Ack / Nak behaviour
//! - On successful processing (activity ran, result reported): `ack()`.
//! - On transient worker failure (e.g. OOM): `nak()` — re-delivers after backoff.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use async_nats::jetstream::consumer::pull;
use async_nats::jetstream::AckKind;
use async_nats::jetstream::Context;
use bytes::Bytes;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use wtf_common::{ActivityId, InstanceId, NamespaceId, RetryPolicy, WtfError};

/// NATS `JetStream` stream name for the activity work queue.
pub const WORK_STREAM_NAME: &str = "wtf-work";
/// NATS subject prefix for work items: `wtf.work.<activity_type>`.
pub const WORK_SUBJECT_PREFIX: &str = "wtf.work";

/// An activity task dispatched by the engine and pulled by a worker.
///
/// Serialized as msgpack and stored as the NATS message payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityTask {
    /// Unique ID for this activity invocation (deterministic for Procedural).
    pub activity_id: ActivityId,
    /// Name of the activity handler to invoke (e.g. `"charge_card"`).
    pub activity_type: String,
    /// Msgpack-encoded input payload for the activity.
    pub payload: Bytes,
    /// Namespace of the owning workflow instance.
    pub namespace: NamespaceId,
    /// Instance ID of the owning workflow.
    pub instance_id: InstanceId,
    /// Attempt number (1-based). First attempt = 1.
    pub attempt: u32,
    /// Retry policy — used by the worker to decide whether to re-enqueue.
    pub retry_policy: RetryPolicy,
    /// Optional per-activity timeout. `None` means no timeout.
    #[serde(default, with = "timeout_serde")]
    pub timeout: Option<Duration>,
}

impl ActivityTask {
    /// Validate task invariants.
    ///
    /// # Errors
    /// Returns `WtfError::InvalidInput` when timeout is set below 1ms.
    pub fn validate(&self) -> Result<(), WtfError> {
        match self.timeout {
            Some(timeout) if timeout < Duration::from_millis(1) => Err(WtfError::InvalidInput {
                detail: "activity timeout must be >= 1ms".to_owned(),
            }),
            _ => Ok(()),
        }
    }

    /// Serialize this task to msgpack bytes for storing in the NATS work queue.
    ///
    /// # Errors
    /// Returns `WtfError::NatsPublish` if serialization fails.
    pub fn to_msgpack(&self) -> Result<Bytes, WtfError> {
        rmp_serde::to_vec_named(self)
            .map(Bytes::from)
            .map_err(|e| WtfError::nats_publish(format!("serialize ActivityTask: {e}")))
    }

    /// Deserialize an `ActivityTask` from msgpack bytes.
    ///
    /// # Errors
    /// Returns `WtfError::NatsPublish` if deserialization fails.
    pub fn from_msgpack(bytes: &[u8]) -> Result<Self, WtfError> {
        rmp_serde::from_slice(bytes)
            .map_err(|e| WtfError::nats_publish(format!("deserialize ActivityTask: {e}")))
            .and_then(|task: Self| task.validate().map(|()| task))
    }

    /// Build the NATS subject for dispatching this task.
    #[must_use]
    pub fn subject(&self) -> String {
        format!("{}.{}", WORK_SUBJECT_PREFIX, self.activity_type)
    }
}

mod timeout_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(value: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value
            .map(|timeout| timeout.as_millis() as u64)
            .serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<u64>::deserialize(deserializer).map(|v| v.map(Duration::from_millis))
    }
}

/// A pulled task together with the NATS message needed to ack or nak it.
///
/// The worker calls [`AckableTask::ack`] after successfully reporting the
/// activity result, or [`AckableTask::nak`] on transient failure.
pub struct AckableTask {
    /// The deserialized task payload.
    pub task: ActivityTask,
    message: async_nats::jetstream::Message,
}

impl AckableTask {
    /// Acknowledge this task — removes it from the work queue.
    ///
    /// Call this AFTER successfully appending the result to `JetStream`.
    ///
    /// # Errors
    /// Returns `WtfError::NatsPublish` if the ack delivery fails.
    pub async fn ack(self) -> Result<(), WtfError> {
        self.message
            .ack()
            .await
            .map_err(|e| WtfError::nats_publish(format!("ack failed: {e}")))
    }

    /// Negative-acknowledge — the task will be re-delivered after the default
    /// NATS backoff period. Use for transient worker failures.
    ///
    /// # Errors
    /// Returns `WtfError::NatsPublish` if the nak delivery fails.
    pub async fn nak(self) -> Result<(), WtfError> {
        self.message
            .ack_with(AckKind::Nak(None))
            .await
            .map_err(|e| WtfError::nats_publish(format!("nak failed: {e}")))
    }
}

impl std::fmt::Debug for AckableTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AckableTask")
            .field("task", &self.task)
            .finish_non_exhaustive()
    }
}

/// Pull consumer wrapper for the `wtf-work` `JetStream` stream.
///
/// Create via [`WorkQueueConsumer::create`] then call [`WorkQueueConsumer::next_task`]
/// in a loop to process activities.
pub struct WorkQueueConsumer {
    messages: pull::Stream,
}

impl WorkQueueConsumer {
    fn durable_for(filter_subject: &Option<String>, worker_name: &str) -> String {
        let name = filter_subject
            .as_ref().map_or_else(|| "work_all".to_owned(), |subject| {
                let stable = subject
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                    .collect::<String>();
                format!("work_{stable}")
            });

        if name.is_empty() {
            worker_name.to_owned()
        } else {
            name
        }
    }

    /// Create a durable pull consumer on `wtf-work` for the given worker name.
    ///
    /// If a consumer with `worker_name` already exists it is reused (idempotent).
    /// `filter_subject` may be set to consume only a specific activity type
    /// (`wtf.work.<activity_type>`). Pass `None` to consume all activity types.
    ///
    /// # Errors
    /// Returns `WtfError::NatsPublish` if the stream or consumer cannot be found/created.
    pub async fn create(
        js: &Context,
        worker_name: &str,
        filter_subject: Option<String>,
    ) -> Result<Self, WtfError> {
        let durable_name = Self::durable_for(&filter_subject, worker_name);
        let config = pull::Config {
            durable_name: Some(durable_name.clone()),
            filter_subject: filter_subject.unwrap_or_else(|| format!("{WORK_SUBJECT_PREFIX}.>")),
            ..pull::Config::default()
        };

        let stream = js
            .get_stream(WORK_STREAM_NAME)
            .await
            .map_err(|e| WtfError::nats_publish(format!("get stream {WORK_STREAM_NAME}: {e}")))?;

        let consumer = stream
            .get_or_create_consumer::<pull::Config>(&durable_name, config)
            .await
            .map_err(|e| WtfError::nats_publish(format!("get/create consumer: {e}")))?;

        let messages = consumer
            .messages()
            .await
            .map_err(|e| WtfError::nats_publish(format!("open messages stream: {e}")))?;

        Ok(Self { messages })
    }

    /// Pull the next task from the work queue.
    ///
    /// Returns `None` when the stream is closed (engine shutting down).
    /// Returns an `AckableTask` that must be acked or naked after processing.
    ///
    /// # Errors
    /// Returns `WtfError` on NATS errors or payload deserialization failures.
    pub async fn next_task(&mut self) -> Result<Option<AckableTask>, WtfError> {
        match self.messages.next().await {
            None => Ok(None),
            Some(Err(e)) => Err(WtfError::nats_publish(format!("receive message: {e}"))),
            Some(Ok(message)) => {
                let task = ActivityTask::from_msgpack(&message.payload)?;
                Ok(Some(AckableTask { task, message }))
            }
        }
    }
}

// ── Publish helper used by the engine actor ───────────────────────────────────

/// Publish an `ActivityTask` to the `wtf-work` `JetStream` stream.
///
/// Called by the actor AFTER appending `ActivityDispatched` to the event log
/// (ADR-015: write-ahead guarantee). This is the second step in the dispatch
/// sequence: event log first, work queue second.
///
/// # Errors
/// Returns `WtfError::NatsPublish` on serialization or publish failure.
pub async fn enqueue_activity(js: &Context, task: &ActivityTask) -> Result<u64, WtfError> {
    let subject = task.subject();
    let payload = task.to_msgpack()?;

    let ack = js
        .publish(subject, payload)
        .await
        .map_err(|e| WtfError::nats_publish(format!("enqueue publish: {e}")))?
        .await
        .map_err(|e| WtfError::nats_publish(format!("enqueue ack: {e}")))?;

    Ok(ack.sequence)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_task(activity_type: &str, attempt: u32) -> ActivityTask {
        ActivityTask {
            activity_id: ActivityId::new("act-001"),
            activity_type: activity_type.to_owned(),
            payload: Bytes::from_static(b"{\"amount\":100}"),
            namespace: NamespaceId::new("payments"),
            instance_id: InstanceId::new("inst-001"),
            attempt,
            retry_policy: RetryPolicy::default(),
            timeout: None,
        }
    }

    #[test]
    fn activity_task_subject_format() {
        let task = make_task("charge_card", 1);
        assert_eq!(task.subject(), "wtf.work.charge_card");
    }

    #[test]
    fn activity_task_subject_second_type() {
        let task = make_task("send_email", 1);
        assert_eq!(task.subject(), "wtf.work.send_email");
    }

    #[test]
    fn activity_task_msgpack_roundtrip() {
        let task = make_task("charge_card", 1);
        let bytes = task.to_msgpack().expect("serialize");
        assert!(!bytes.is_empty());
        let decoded = ActivityTask::from_msgpack(&bytes).expect("deserialize");
        assert_eq!(decoded.activity_type, "charge_card");
        assert_eq!(decoded.attempt, 1);
        assert_eq!(decoded.activity_id.as_str(), "act-001");
    }

    #[test]
    fn activity_task_msgpack_preserves_payload() {
        let task = make_task("charge_card", 1);
        let bytes = task.to_msgpack().expect("serialize");
        let decoded = ActivityTask::from_msgpack(&bytes).expect("deserialize");
        assert_eq!(decoded.payload, Bytes::from_static(b"{\"amount\":100}"));
    }

    #[test]
    fn activity_task_msgpack_preserves_namespace_and_instance() {
        let task = make_task("refund", 2);
        let bytes = task.to_msgpack().expect("serialize");
        let decoded = ActivityTask::from_msgpack(&bytes).expect("deserialize");
        assert_eq!(decoded.namespace.as_str(), "payments");
        assert_eq!(decoded.instance_id.as_str(), "inst-001");
        assert_eq!(decoded.attempt, 2);
    }

    #[test]
    fn activity_task_from_msgpack_invalid_bytes_returns_error() {
        let result = ActivityTask::from_msgpack(b"not msgpack at all!!!");
        assert!(result.is_err());
    }

    #[test]
    fn activity_task_rejects_zero_timeout() {
        let task = ActivityTask {
            timeout: Some(Duration::ZERO),
            ..make_task("charge_card", 1)
        };
        assert!(task.validate().is_err());
    }

    #[test]
    fn activity_task_accepts_one_millisecond_timeout() {
        let task = ActivityTask {
            timeout: Some(Duration::from_millis(1)),
            ..make_task("charge_card", 1)
        };
        assert!(task.validate().is_ok());
    }

    #[test]
    fn work_stream_name_constant() {
        assert_eq!(WORK_STREAM_NAME, "wtf-work");
    }

    #[test]
    fn work_subject_prefix_constant() {
        assert_eq!(WORK_SUBJECT_PREFIX, "wtf.work");
    }

    // WorkQueueConsumer::create and next_task require a live NATS server.
    // Covered by integration tests (wtf-2bbn).
}
