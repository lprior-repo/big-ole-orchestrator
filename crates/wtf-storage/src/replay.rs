//! JetStream replay consumer — ordered event replay for crash recovery (ADR-016).
//!
//! Recovery procedure (ADR-019):
//! 1. Load snapshot from sled (if present and valid).
//! 2. Create an ordered consumer starting at `snapshot.seq + 1`.
//! 3. Replay events until the stream tail → switch to Live Phase.
//!
//! The consumer is ephemeral (push consumer, no durable name). It is created
//! fresh on each recovery and discarded when replay completes.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use std::time::Duration;

use async_nats::jetstream::{
    consumer::{push::Config as PushConfig, AckPolicy, DeliverPolicy, ReplayPolicy},
    Context,
};
use bytes::Bytes;
use futures::StreamExt;
use wtf_common::{InstanceId, NamespaceId, WorkflowEvent, WtfError};

use crate::journal::build_subject;

/// A single decoded event from the replay stream.
#[derive(Debug, Clone)]
pub struct ReplayedEvent {
    /// JetStream sequence number of this event.
    pub seq: u64,
    /// The decoded workflow event.
    pub event: WorkflowEvent,
}

/// Configuration for a replay consumer.
#[derive(Debug, Clone)]
pub struct ReplayConfig {
    /// Resume from this sequence number (inclusive).
    ///
    /// Set to `1` to replay from the beginning of the stream.
    /// Set to `snapshot.seq + 1` to replay only the tail after the snapshot.
    pub from_seq: u64,

    /// Maximum time to wait for each message before declaring tail reached.
    ///
    /// Default: 200ms. The actor declares Replay→Live transition when
    /// the consumer produces no message within this window.
    pub tail_timeout: Duration,
}

impl Default for ReplayConfig {
    fn default() -> Self {
        Self {
            from_seq: 1,
            tail_timeout: Duration::from_millis(200),
        }
    }
}

/// A single fetched batch from the replay consumer.
#[derive(Debug)]
pub enum ReplayBatch {
    /// One decoded event from the log.
    Event(ReplayedEvent),
    /// No more messages within the tail timeout — replay is complete.
    /// The actor should switch to Live Phase.
    TailReached,
}

/// An active replay consumer. Call `next_event()` to retrieve events one at a time.
pub struct ReplayConsumer {
    messages: async_nats::jetstream::consumer::push::Messages,
    tail_timeout: Duration,
}

impl ReplayConsumer {
    /// Fetch the next event, or detect stream tail via timeout.
    ///
    /// Returns `Ok(ReplayBatch::Event(_))` for each replayed event.
    /// Returns `Ok(ReplayBatch::TailReached)` when no message arrives
    /// within `tail_timeout` — the actor should switch to Live Phase.
    ///
    /// # Errors
    /// Returns `WtfError::NatsPublish` if the NATS stream errors.
    pub async fn next_event(&mut self) -> Result<ReplayBatch, WtfError> {
        match tokio::time::timeout(self.tail_timeout, self.messages.next()).await {
            Ok(Some(Ok(msg))) => {
                let info = msg
                    .info()
                    .map_err(|e| WtfError::nats_publish(format!("read msg info: {e}")))?;
                let seq = info.stream_sequence;
                let event = decode_event(msg.payload.clone())?;
                Ok(ReplayBatch::Event(ReplayedEvent { seq, event }))
            }
            Ok(Some(Err(e))) => Err(WtfError::nats_publish(format!("replay stream error: {e}"))),
            Ok(None) => {
                // Stream closed — treat as tail reached.
                Ok(ReplayBatch::TailReached)
            }
            Err(_timeout) => Ok(ReplayBatch::TailReached),
        }
    }
}

/// Create an ephemeral ordered JetStream push consumer for replay.
///
/// The consumer subscribes to the per-instance subject
/// (`wtf.log.<namespace>.<instance_id>`) and delivers messages in order
/// starting from `config.from_seq`.
///
/// # Errors
/// Returns `WtfError::NatsPublish` if consumer creation fails.
pub async fn create_replay_consumer(
    js: &Context,
    namespace: &NamespaceId,
    instance_id: &InstanceId,
    config: &ReplayConfig,
) -> Result<ReplayConsumer, WtfError> {
    let subject = build_subject(namespace, instance_id);

    // Use a timestamped inbox to avoid collisions on concurrent replays.
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_else(|_| 0u128);

    let deliver_subject = format!("_INBOX.wtf.replay.{}.{ts}", instance_id.as_str());

    let consumer_config = PushConfig {
        deliver_subject: deliver_subject.clone(),
        deliver_policy: DeliverPolicy::ByStartSequence {
            start_sequence: config.from_seq,
        },
        ack_policy: AckPolicy::None,
        replay_policy: ReplayPolicy::Instant,
        filter_subject: subject,
        ..Default::default()
    };

    let stream = js
        .get_stream(crate::provision::stream_names::EVENTS)
        .await
        .map_err(|e| WtfError::nats_publish(format!("get stream for replay: {e}")))?;

    let consumer = stream
        .create_consumer(consumer_config)
        .await
        .map_err(|e| WtfError::nats_publish(format!("create replay consumer: {e}")))?;

    let messages = consumer
        .messages()
        .await
        .map_err(|e| WtfError::nats_publish(format!("replay consumer messages(): {e}")))?;

    Ok(ReplayConsumer {
        messages,
        tail_timeout: config.tail_timeout,
    })
}

fn decode_event(payload: Bytes) -> Result<WorkflowEvent, WtfError> {
    WorkflowEvent::from_msgpack(&payload)
        .map_err(|e| WtfError::nats_publish(format!("decode event: {e}")))
}

/// Compute the sequence number to start replay from, given an optional snapshot.
///
/// If a snapshot is present, replay starts at `snapshot.seq + 1`.
/// If no snapshot, replay starts from sequence `1` (full replay).
#[must_use]
pub fn replay_start_seq(snapshot_seq: Option<u64>) -> u64 {
    match snapshot_seq {
        Some(seq) => seq + 1,
        None => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_start_seq_no_snapshot() {
        assert_eq!(replay_start_seq(None), 1);
    }

    #[test]
    fn replay_start_seq_with_snapshot_at_zero() {
        assert_eq!(replay_start_seq(Some(0)), 1);
    }

    #[test]
    fn replay_start_seq_with_snapshot() {
        assert_eq!(replay_start_seq(Some(100)), 101);
    }

    #[test]
    fn replay_start_seq_at_large_value() {
        assert_eq!(replay_start_seq(Some(u64::MAX - 1)), u64::MAX);
    }

    #[test]
    fn replay_config_default_from_seq_is_one() {
        let cfg = ReplayConfig::default();
        assert_eq!(cfg.from_seq, 1);
    }

    #[test]
    fn replay_config_default_tail_timeout_is_200ms() {
        let cfg = ReplayConfig::default();
        assert_eq!(cfg.tail_timeout, Duration::from_millis(200));
    }

    // create_replay_consumer and ReplayConsumer::next_event require live NATS — integration tests.
}
