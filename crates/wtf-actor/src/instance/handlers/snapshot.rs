//! Snapshot trigger handler (ADR-019).
//!
//! Writes a snapshot of the current paradigm state when the event interval
//! threshold is reached. Snapshot writes are write-aside (the in-memory state
//! is never mutated) and failures are non-fatal.

use super::super::state::InstanceState;
use bytes::Bytes;
use ractor::ActorProcessingErr;

/// Trigger a snapshot write for the current instance state.
///
/// Requires both `event_store` and `snapshot_db` to be present in the
/// instance arguments. Returns `Err` if either is missing. If the
/// snapshot write itself fails, the error is logged but `Ok(())` is
/// returned — the counter is **not** reset, so the next interval will
/// retry.
pub(crate) async fn handle_snapshot_trigger(
    state: &mut InstanceState,
) -> Result<(), ActorProcessingErr> {
    let event_store = state
        .args
        .event_store
        .as_ref()
        .ok_or_else(|| ActorProcessingErr::from("snapshot requires event_store"))?;
    let db = state
        .args
        .snapshot_db
        .as_ref()
        .ok_or_else(|| ActorProcessingErr::from("snapshot requires snapshot_db"))?;

    let state_bytes = rmp_serde::to_vec_named(&state.paradigm_state)
        .map_err(|e| ActorProcessingErr::from(Box::new(e)))?;
    let last_applied_seq = state.total_events_applied;

    match crate::snapshot::write_instance_snapshot(
        event_store.as_ref(),
        db,
        &state.args.namespace,
        &state.args.instance_id,
        last_applied_seq,
        Bytes::from(state_bytes),
    )
    .await
    {
        Ok(result) => {
            tracing::info!(
                instance_id = %state.args.instance_id,
                seq = last_applied_seq,
                jetstream_seq = result.jetstream_seq,
                checksum = result.checksum,
                "snapshot written"
            );
            state.events_since_snapshot = 0;
        }
        Err(e) => {
            tracing::warn!(
                instance_id = %state.args.instance_id,
                error = %e,
                "snapshot write failed — continuing, will retry at next interval"
            );
        }
    }

    Ok(())
}
