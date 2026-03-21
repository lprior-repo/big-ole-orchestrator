use std::time::Duration;
use async_nats::jetstream::kv::Store;
use async_nats::jetstream::Context;
use chrono::Utc;
use futures::StreamExt;
use wtf_common::{TimerId, WorkflowEvent, WtfError};
use wtf_storage::append_event;
use crate::timer::record::TimerRecord;

pub const TIMER_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Write a timer record into the `wtf-timers` KV bucket.
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
pub async fn fire_timer(
    js: &Context,
    timers: &Store,
    record: &TimerRecord,
) -> Result<u64, WtfError> {
    let event = WorkflowEvent::TimerFired {
        timer_id: record.timer_id.as_str().to_owned(),
    };

    let seq = append_event(js, &record.namespace, &record.instance_id, &event).await?;

    tracing::debug!(
        timer_id = %record.timer_id,
        namespace = %record.namespace,
        instance_id = %record.instance_id,
        seq,
        "timer fired"
    );

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
                if result.is_err() || *shutdown_rx.borrow() {
                    tracing::info!("timer loop shutting down");
                    break;
                }
            }
        }
    }

    Ok(())
}

async fn poll_and_fire(js: &Context, timers: &Store) -> Result<(), WtfError> {
    let now = Utc::now();

    let mut keys = timers
        .keys()
        .await
        .map_err(|e| WtfError::nats_publish(format!("list timer keys: {e}")))?;

    while let Some(key_result) = keys.next().await {
        if let Ok(key) = key_result {
            if let Ok(Some(value)) = timers.get(&key).await {
                if let Ok(record) = TimerRecord::from_msgpack(&value) {
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
            }
        }
    }

    Ok(())
}
