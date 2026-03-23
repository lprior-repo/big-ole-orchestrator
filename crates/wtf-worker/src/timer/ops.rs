use super::record::TimerRecord;
use async_nats::jetstream::kv::Store;
use async_nats::jetstream::Context;
use wtf_common::{TimerId, WorkflowEvent, WtfError};
use wtf_storage::append_event;

/// Write a timer record into the `wtf-timers` KV bucket.
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
    tracing::debug!(timer_id = %record.timer_id, namespace = %record.namespace, instance_id = %record.instance_id, seq, "timer fired");

    if let Err(e) = delete_timer(timers, &record.timer_id).await {
        tracing::warn!(timer_id = %record.timer_id, error = %e, "failed to delete timer from KV after firing — may re-fire");
    }

    Ok(seq)
}
