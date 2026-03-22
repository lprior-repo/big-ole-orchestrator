//! Timer firing loop (bead wtf-df7a).
//!
//! Polls the `wtf-timers` NATS KV bucket every second. For each timer whose
//! `fire_at` has passed, the loop appends a `TimerFired` event to JetStream
//! and then deletes the KV entry.
//!
//! # Architecture
//! - Timer entries are written to `wtf-timers` KV by the instance actor when
//!   a `TimerScheduled` event is applied.
//! - The timer loop is a separate process (or task) that fires them.
//! - Write-ahead: `TimerFired` is appended to JetStream *before* the KV entry
//!   is deleted. If the loop crashes after appending but before deleting, the
//!   entry will be re-processed on restart — the instance actor must handle
//!   duplicate `TimerFired` events idempotently (applied_seq check).
//!
//! # Timer record format
//! Key: `<timer_id>` (e.g. `timer-01ARZ`)
//! Value: msgpack-encoded [`TimerRecord`].

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use std::time::Duration;

use async_nats::jetstream::kv::{Entry, Operation, Store};
use async_nats::jetstream::Context;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use wtf_common::{InstanceId, NamespaceId, TimerId, WorkflowEvent, WtfError};
use wtf_storage::append_event;

/// Default poll interval for the timer loop.
pub const TIMER_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// A pending timer stored in the `wtf-timers` KV bucket.
///
/// Serialized as msgpack. The KV key is the `timer_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerRecord {
    /// Unique timer ID.
    pub timer_id: TimerId,
    /// Namespace of the owning workflow instance.
    pub namespace: NamespaceId,
    /// Instance ID that scheduled the timer.
    pub instance_id: InstanceId,
    /// UTC timestamp when the timer should fire.
    pub fire_at: DateTime<Utc>,
}

impl TimerRecord {
    /// Serialize to msgpack bytes for KV storage.
    ///
    /// # Errors
    /// Returns `WtfError::NatsPublish` if serialization fails.
    pub fn to_msgpack(&self) -> Result<Bytes, WtfError> {
        rmp_serde::to_vec_named(self)
            .map(Bytes::from)
            .map_err(|e| WtfError::nats_publish(format!("serialize TimerRecord: {e}")))
    }

    /// Deserialize from msgpack bytes.
    ///
    /// # Errors
    /// Returns `WtfError::NatsPublish` if deserialization fails.
    pub fn from_msgpack(bytes: &[u8]) -> Result<Self, WtfError> {
        rmp_serde::from_slice(bytes)
            .map_err(|e| WtfError::nats_publish(format!("deserialize TimerRecord: {e}")))
    }

    /// Whether this timer is due to fire at or before `now`.
    #[must_use]
    pub fn is_due(&self, now: DateTime<Utc>) -> bool {
        self.fire_at <= now
    }
}

/// Write a timer record into the `wtf-timers` KV bucket.
///
/// Called by the instance actor when applying `TimerScheduled`. The key is
/// the `timer_id`; the value is msgpack-encoded `TimerRecord`.
///
/// # Errors
/// Returns `WtfError::NatsPublish` on serialize or KV write failure.
pub async fn store_timer(timers: &Store, record: &TimerRecord) -> Result<(), WtfError> {
    let key = record.timer_id.as_str().to_owned();
    let payload = record.to_msgpack()?;
    timers
        .put(&key, payload)
        .await
        .map_err(|e| WtfError::nats_publish(format!("store timer {}: {e}", record.timer_id)))?;
    Ok(())
}

/// Delete a timer record from the `wtf-timers` KV bucket.
///
/// Called after successfully appending `TimerFired` to JetStream.
///
/// # Errors
/// Returns `WtfError::NatsPublish` on KV delete failure.
pub async fn delete_timer(timers: &Store, timer_id: &TimerId) -> Result<(), WtfError> {
    timers
        .delete(timer_id.as_str())
        .await
        .map_err(|e| WtfError::nats_publish(format!("delete timer {timer_id}: {e}")))?;
    Ok(())
}

/// Fire a single expired timer: append `TimerFired` to JetStream, then delete from KV.
///
/// # Errors
/// Returns `WtfError` if the JetStream append or KV delete fails. If the append
/// succeeds but the delete fails the timer may re-fire — instance actors must
/// handle duplicate `TimerFired` events idempotently.
pub async fn fire_timer(
    js: &Context,
    timers: &Store,
    record: &TimerRecord,
) -> Result<u64, WtfError> {
    let event = WorkflowEvent::TimerFired {
        timer_id: record.timer_id.as_str().to_owned(),
    };

    // Write-ahead: append to JetStream BEFORE deleting from KV.
    let seq = append_event(js, &record.namespace, &record.instance_id, &event).await?;

    tracing::debug!(
        timer_id = %record.timer_id,
        namespace = %record.namespace,
        instance_id = %record.instance_id,
        seq,
        "timer fired"
    );

    // Best-effort delete — if this fails the timer will re-fire on next poll.
    if let Err(e) = delete_timer(timers, &record.timer_id).await {
        tracing::warn!(
            timer_id = %record.timer_id,
            error = %e,
            "failed to delete timer from KV after firing — may re-fire"
        );
    }

    Ok(seq)
}

/// Run the timer firing loop until `shutdown_rx` fires or the channel closes.
///
/// Polls the `wtf-timers` KV bucket every [`TIMER_POLL_INTERVAL`]. For each
/// entry whose `fire_at` has passed, calls [`fire_timer`].
///
/// This function runs forever (until shutdown). Spawn it as a Tokio task:
/// ```ignore
/// tokio::spawn(run_timer_loop(js, timers, shutdown_rx));
/// ```
///
/// # Errors
/// Returns `WtfError` only for unrecoverable errors. Per-timer errors are
/// logged and the loop continues.
pub async fn run_timer_loop(
    js: Context,
    timers: Store,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> Result<(), WtfError> {
    tracing::info!("timer loop started");
    let mut interval = tokio::time::interval(TIMER_POLL_INTERVAL);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                if let Err(e) = poll_and_fire(&js, &timers).await {
                    tracing::error!(error = %e, "timer poll error");
                }
            }
            result = shutdown_rx.changed() => {
                match result {
                    Ok(()) | Err(_) => {
                        tracing::info!("timer loop shutting down");
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Run the timer firing loop using KV watch instead of polling.
///
/// Uses `watch_all()` to observe changes to the `wtf-timers` KV bucket.
/// Performs an initial sync to catch any timers that were already due,
/// then processes subsequent changes via the watch stream.
///
/// # Advantages over polling
/// - No redundant `keys()` calls every second
/// - Only processes timers when they are created/modified
/// - Immediate processing of newly scheduled timers
///
/// # Errors
/// Returns `WtfError` only for unrecoverable errors. Per-timer errors are
/// logged and the loop continues.
pub async fn run_timer_loop_watch(
    js: Context,
    timers: Store,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> Result<(), WtfError> {
    tracing::info!("timer loop (watch mode) started");

    let now = Utc::now();
    if let Err(e) = sync_and_fire_due(&js, &timers, now).await {
        tracing::error!(error = %e, "initial timer sync failed");
    }

    let mut watch = timers
        .watch_all()
        .await
        .map_err(|e| WtfError::nats_publish(format!("watch_all failed: {e}")))?;

    loop {
        tokio::select! {
            entry = watch.next() => {
                match entry {
                    None => {
                        tracing::info!("timer watch stream closed — stopping");
                        break;
                    }
                    Some(Err(e)) => {
                        tracing::warn!(error = %e, "timer watch error — continuing");
                    }
                    Some(Ok(kv_entry)) => {
                        if let Some(record) = process_watch_entry(&kv_entry) {
                            let now = Utc::now();
                            if record.is_due(now) {
                                if let Err(e) = fire_timer(&js, &timers, &record).await {
                                    tracing::error!(
                                        timer_id = %record.timer_id,
                                        error = %e,
                                        "failed to fire timer"
                                    );
                                }
                            }
                        }
                    }
                }
            }
            result = shutdown_rx.changed() => {
                match result {
                    Ok(()) | Err(_) => {
                        tracing::info!("timer loop (watch) shutting down");
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Process a watch entry and return the TimerRecord if valid.
///
/// Returns `None` for Delete/Purge operations or deserialization errors.
fn process_watch_entry(kv_entry: &Entry) -> Option<TimerRecord> {
    match kv_entry.operation {
        Operation::Delete | Operation::Purge => {
            tracing::debug!(key = %kv_entry.key, "timer deleted — skipping");
            None
        }
        Operation::Put => {
            match TimerRecord::from_msgpack(&kv_entry.value) {
                Ok(record) => Some(record),
                Err(e) => {
                    tracing::warn!(
                        key = %kv_entry.key,
                        error = %e,
                        "failed to deserialize timer record — skipping"
                    );
                    None
                }
            }
        }
    }
}

/// Initial sync: list all timer keys and fire any that are due.
///
/// This is called once at startup to catch timers that were already
/// in KV before the watch started.
async fn sync_and_fire_due(
    js: &Context,
    timers: &Store,
    now: DateTime<Utc>,
) -> Result<(), WtfError> {
    let mut keys = timers
        .keys()
        .await
        .map_err(|e| WtfError::nats_publish(format!("list timer keys: {e}")))?;

    while let Some(key_result) = keys.next().await {
        match key_result {
            Err(e) => {
                tracing::warn!(error = %e, "error iterating timer keys during sync");
            }
            Ok(key) => {
                match timers.get(&key).await {
                    Err(e) => {
                        tracing::warn!(key = %key, error = %e, "failed to get timer entry during sync");
                    }
                    Ok(None) => {
                    }
                    Ok(Some(value)) => match TimerRecord::from_msgpack(&value) {
                        Err(e) => {
                            tracing::warn!(
                                key = %key,
                                error = %e,
                                "failed to deserialize timer record during sync — skipping"
                            );
                        }
                        Ok(record) => {
                            if record.is_due(now) {
                                if let Err(e) = fire_timer(js, timers, &record).await {
                                    tracing::error!(
                                        timer_id = %record.timer_id,
                                        error = %e,
                                        "failed to fire timer during initial sync"
                                    );
                                }
                            }
                        }
                    },
                }
            }
        }
    }

    Ok(())
}

/// Poll all KV entries and fire any that are due.
///
/// Lists all keys in `wtf-timers`, fetches each entry, and fires those due.
async fn poll_and_fire(js: &Context, timers: &Store) -> Result<(), WtfError> {
    let now = Utc::now();

    let mut keys = timers
        .keys()
        .await
        .map_err(|e| WtfError::nats_publish(format!("list timer keys: {e}")))?;

    while let Some(key_result) = keys.next().await {
        match key_result {
            Err(e) => {
                tracing::warn!(error = %e, "error iterating timer keys");
            }
            Ok(key) => {
                match timers.get(&key).await {
                    Err(e) => {
                        tracing::warn!(key = %key, error = %e, "failed to get timer entry");
                    }
                    Ok(None) => {
                        // Deleted between keys() and get() — skip.
                    }
                    Ok(Some(value)) => match TimerRecord::from_msgpack(&value) {
                        Err(e) => {
                            tracing::warn!(
                                key = %key,
                                error = %e,
                                "failed to deserialize timer record — skipping"
                            );
                        }
                        Ok(record) => {
                            if record.is_due(now) {
                                if let Err(e) = fire_timer(js, timers, &record).await {
                                    tracing::error!(
                                        timer_id = %record.timer_id,
                                        error = %e,
                                        "failed to fire timer"
                                    );
                                }
                            }
                        }
                    },
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration as ChronoDuration;

    fn make_record(timer_id: &str, fire_at: DateTime<Utc>) -> TimerRecord {
        TimerRecord {
            timer_id: TimerId::new(timer_id),
            namespace: NamespaceId::new("payments"),
            instance_id: InstanceId::new("inst-001"),
            fire_at,
        }
    }

    #[test]
    fn timer_record_is_due_when_fire_at_is_in_the_past() {
        let past = Utc::now() - ChronoDuration::seconds(5);
        let record = make_record("timer-001", past);
        assert!(record.is_due(Utc::now()));
    }

    #[test]
    fn timer_record_is_due_when_fire_at_equals_now() {
        let now = Utc::now();
        let record = make_record("timer-002", now);
        assert!(record.is_due(now));
    }

    #[test]
    fn timer_record_is_not_due_when_fire_at_is_in_the_future() {
        let future = Utc::now() + ChronoDuration::hours(1);
        let record = make_record("timer-003", future);
        assert!(!record.is_due(Utc::now()));
    }

    #[test]
    fn timer_record_msgpack_roundtrip() {
        let fire_at = Utc::now();
        let record = make_record("timer-004", fire_at);
        let bytes = record.to_msgpack().expect("serialize");
        assert!(!bytes.is_empty());
        let decoded = TimerRecord::from_msgpack(&bytes).expect("deserialize");
        assert_eq!(decoded.timer_id.as_str(), "timer-004");
        assert_eq!(decoded.namespace.as_str(), "payments");
        assert_eq!(decoded.instance_id.as_str(), "inst-001");
    }

    #[test]
    fn timer_record_from_msgpack_invalid_bytes_returns_error() {
        let result = TimerRecord::from_msgpack(b"not valid msgpack!!!");
        assert!(result.is_err());
    }

    #[test]
    fn timer_poll_interval_is_one_second() {
        assert_eq!(TIMER_POLL_INTERVAL, Duration::from_secs(1));
    }

    #[test]
    fn timer_record_far_future_not_due() {
        let far_future = Utc::now() + ChronoDuration::days(365);
        let record = make_record("timer-future", far_future);
        assert!(!record.is_due(Utc::now()));
    }

    // store_timer, delete_timer, fire_timer, run_timer_loop require live NATS.
    // Covered by integration tests (wtf-2bbn).
}
