use super::ops::fire_timer;
use super::record::TimerRecord;
use async_nats::jetstream::kv::{Entry, Operation, Store};
use async_nats::jetstream::Context;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use std::time::Duration;
use wtf_common::WtfError;

pub const TIMER_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Run the timer firing loop using polling.
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
                if result.is_ok() || result.is_err() {
                    tracing::info!("timer loop shutting down");
                    break;
                }
            }
        }
    }

    Ok(())
}

/// Run the timer firing loop using KV watch instead of polling.
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
                        handle_watch_entry(&js, &timers, &kv_entry).await;
                    }
                }
            }
            result = shutdown_rx.changed() => {
                if result.is_ok() || result.is_err() {
                    tracing::info!("timer loop (watch) shutting down");
                    break;
                }
            }
        }
    }

    Ok(())
}

async fn handle_watch_entry(js: &Context, timers: &Store, kv_entry: &Entry) {
    if let Some(record) = process_watch_entry(kv_entry) {
        let now = Utc::now();
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

/// Process a watch entry and return the TimerRecord if valid.
fn process_watch_entry(kv_entry: &Entry) -> Option<TimerRecord> {
    match kv_entry.operation {
        Operation::Delete | Operation::Purge => {
            tracing::debug!(key = %kv_entry.key, "timer deleted — skipping");
            None
        }
        Operation::Put => match TimerRecord::from_msgpack(&kv_entry.value) {
            Ok(record) => Some(record),
            Err(e) => {
                tracing::warn!(
                    key = %kv_entry.key,
                    error = %e,
                    "failed to deserialize timer record — skipping"
                );
                None
            }
        },
    }
}

/// Initial sync: list all timer keys and fire any that are due.
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
        if let Ok(key) = key_result {
            if let Ok(Some(value)) = timers.get(&key).await {
                if let Ok(record) = TimerRecord::from_msgpack(&value) {
                    if record.is_due(now) {
                        let _ = fire_timer(js, timers, &record).await;
                    }
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
                        let _ = fire_timer(js, timers, &record).await;
                    }
                }
            }
        }
    }

    Ok(())
}
