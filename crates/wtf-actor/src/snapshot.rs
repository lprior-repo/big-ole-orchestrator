//! Snapshot trigger — write sled snapshot + `SnapshotTaken` `JetStream` event (ADR-019).
//!
//! Called when `events_since_snapshot >= SNAPSHOT_INTERVAL` (every 100 events).
//!
//! The procedure (write-ahead, ADR-015):
//! 1. Serialize actor state to msgpack bytes.
//! 2. Write `SnapshotRecord` to sled (local, fast).
//! 3. Append `SnapshotTaken { seq, checksum }` to `JetStream` (durable, crash-safe).
//!
//! On crash, recovery loads the sled snapshot and replays only events AFTER `seq`.
//! If the sled snapshot is corrupt or missing, full replay from seq=1 is the fallback.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use bytes::Bytes;
use wtf_common::{EventStore, InstanceId, NamespaceId, WorkflowEvent, WtfError};
use wtf_storage::snapshots::{write_snapshot, SnapshotRecord};

use crate::instance::handlers::SNAPSHOT_INTERVAL;

/// The result of a successful snapshot write.
#[derive(Debug, Clone)]
pub struct SnapshotResult {
    /// `JetStream` sequence number of the `SnapshotTaken` event.
    pub jetstream_seq: u64,
    /// CRC32 checksum of the written state bytes.
    pub checksum: u32,
}

/// Write a snapshot for a workflow instance.
///
/// # Arguments
/// - `event_store`: `EventStore` for appending `SnapshotTaken`.
/// - `db`: sled database for writing the `SnapshotRecord`.
/// - `namespace`: instance namespace.
/// - `instance_id`: instance being snapshotted.
/// - `last_applied_seq`: `JetStream` sequence of the last applied event.
/// - `state_bytes`: msgpack-encoded actor state (`FsmActorState` / `DagActorState` / `ProceduralActorState`).
///
/// # Errors
/// - `WtfError::SledError` if sled write fails (non-fatal: caller falls back to full replay).
/// - `WtfError::NatsPublish` if `JetStream` `SnapshotTaken` publish fails (non-fatal).
pub async fn write_instance_snapshot(
    event_store: &dyn EventStore,
    db: &sled::Db,
    namespace: &NamespaceId,
    instance_id: &InstanceId,
    last_applied_seq: u64,
    state_bytes: Bytes,
) -> Result<SnapshotResult, WtfError> {
    let record = SnapshotRecord::new(last_applied_seq, state_bytes);
    let checksum = record.checksum;

    persist_local_snapshot(db, instance_id, &record);

    let jetstream_seq = publish_snapshot_event(
        event_store,
        namespace,
        instance_id,
        last_applied_seq,
        checksum,
    )
    .await?;

    Ok(SnapshotResult {
        jetstream_seq,
        checksum,
    })
}

fn persist_local_snapshot(db: &sled::Db, instance_id: &InstanceId, record: &SnapshotRecord) {
    if let Err(e) = write_snapshot(db, instance_id, record) {
        tracing::warn!(
            instance_id = %instance_id,
            error = %e,
            "sled snapshot write failed — SnapshotTaken still published; recovery will replay from start"
        );
    }
}

async fn publish_snapshot_event(
    event_store: &dyn EventStore,
    namespace: &NamespaceId,
    instance_id: &InstanceId,
    last_applied_seq: u64,
    checksum: u32,
) -> Result<u64, WtfError> {
    let event = WorkflowEvent::SnapshotTaken {
        seq: last_applied_seq,
        checksum,
    };

    let seq = event_store.publish(namespace, instance_id, event).await?;

    tracing::debug!(
        instance_id = %instance_id,
        last_applied_seq,
        jetstream_seq = seq,
        checksum,
        "snapshot written"
    );

    Ok(seq)
}

/// Check whether a snapshot should be triggered.
///
/// Returns `true` when `events_since_snapshot` has reached `SNAPSHOT_INTERVAL`.
#[must_use]
pub fn should_snapshot(events_since_snapshot: u32) -> bool {
    events_since_snapshot >= SNAPSHOT_INTERVAL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_snapshot_below_threshold() {
        assert!(!should_snapshot(0));
        assert!(!should_snapshot(50));
        assert!(!should_snapshot(SNAPSHOT_INTERVAL - 1));
    }

    #[test]
    fn should_snapshot_at_threshold() {
        assert!(should_snapshot(SNAPSHOT_INTERVAL));
    }

    #[test]
    fn should_snapshot_above_threshold() {
        assert!(should_snapshot(SNAPSHOT_INTERVAL + 1));
        assert!(should_snapshot(200));
    }

    // write_instance_snapshot requires live NATS + sled — integration tests (wtf-rakc).
}
