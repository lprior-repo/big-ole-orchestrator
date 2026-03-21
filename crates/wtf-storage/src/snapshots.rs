//! sled snapshot store — per-instance state snapshots for fast crash recovery (ADR-019).
//!
//! sled is the ONLY place snapshots live. NATS JetStream has the `SnapshotTaken` marker event,
//! but the actual state bytes live here. Neither is source-of-truth for workflow state —
//! JetStream replay is always correct; snapshots exist only to bound replay latency.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use std::path::Path;

use bytes::Bytes;
use chrono::{DateTime, Utc};
use crc32fast::Hasher as Crc32Hasher;
use serde::{Deserialize, Serialize};
use wtf_common::{InstanceId, WtfError};

const SNAPSHOTS_TREE: &[u8] = b"snapshots";

/// A point-in-time snapshot of actor in-memory state (ADR-019).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotRecord {
    /// The JetStream sequence number of the last event applied before this snapshot.
    /// Recovery replays from `seq + 1` to the stream tail.
    pub seq: u64,

    /// Msgpack-encoded actor state (FsmActorState | DagActorState | ProceduralActorState).
    pub state_bytes: Bytes,

    /// CRC32 checksum of `state_bytes` for corruption detection.
    pub checksum: u32,

    /// Wall-clock time the snapshot was taken (informational only — not used during replay).
    pub taken_at: DateTime<Utc>,
}

impl SnapshotRecord {
    /// Build a `SnapshotRecord`, computing the CRC32 checksum of `state_bytes`.
    #[must_use]
    pub fn new(seq: u64, state_bytes: Bytes) -> Self {
        let checksum = crc32_of(&state_bytes);
        Self {
            seq,
            state_bytes,
            checksum,
            taken_at: Utc::now(),
        }
    }

    /// Return `true` if the CRC32 checksum is valid.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        crc32_of(&self.state_bytes) == self.checksum
    }
}

/// Open (or create) the sled database at `path`.
///
/// # Errors
/// Returns [`WtfError::SledError`] if sled fails to open the database.
pub fn open_snapshot_db(path: &Path) -> Result<sled::Db, WtfError> {
    sled::open(path).map_err(|e| WtfError::sled_error(format!("open {}: {e}", path.display())))
}

/// Write a snapshot for `instance_id`.
///
/// Serializes with msgpack and stores under the `snapshots` tree.
/// Key = `instance_id` bytes. Value = msgpack-encoded `SnapshotRecord`.
///
/// # Errors
/// Returns [`WtfError::SledError`] on I/O failure.
/// A sled write failure is non-fatal for callers — fall back to full replay.
pub fn write_snapshot(
    db: &sled::Db,
    instance_id: &InstanceId,
    record: &SnapshotRecord,
) -> Result<(), WtfError> {
    let tree = db
        .open_tree(SNAPSHOTS_TREE)
        .map_err(|e| WtfError::sled_error(format!("open tree: {e}")))?;

    let value = rmp_serde::to_vec_named(record)
        .map_err(|e| WtfError::sled_error(format!("encode: {e}")))?;

    tree.insert(instance_id.as_str().as_bytes(), value)
        .map_err(|e| WtfError::sled_error(format!("insert {instance_id}: {e}")))?;

    tree.flush()
        .map_err(|e| WtfError::sled_error(format!("flush: {e}")))?;

    Ok(())
}

/// Read and validate the snapshot for `instance_id`.
///
/// Returns `None` if no snapshot exists or if the checksum is invalid (logs a WARN).
/// On checksum failure the caller should fall back to full JetStream replay.
///
/// # Errors
/// Returns [`WtfError::SledError`] on I/O failure.
pub fn read_snapshot(
    db: &sled::Db,
    instance_id: &InstanceId,
) -> Result<Option<SnapshotRecord>, WtfError> {
    let tree = db
        .open_tree(SNAPSHOTS_TREE)
        .map_err(|e| WtfError::sled_error(format!("open tree: {e}")))?;

    let bytes = match tree
        .get(instance_id.as_str().as_bytes())
        .map_err(|e| WtfError::sled_error(format!("read {instance_id}: {e}")))?
    {
        Some(b) => b,
        None => return Ok(None),
    };

    let record: SnapshotRecord = rmp_serde::from_slice(&bytes)
        .map_err(|e| WtfError::sled_error(format!("decode snapshot for {instance_id}: {e}")))?;

    if !record.is_valid() {
        tracing::warn!(
            instance_id = %instance_id,
            seq = record.seq,
            "snapshot_corrupted: checksum mismatch; falling back to full replay"
        );
        return Ok(None);
    }

    Ok(Some(record))
}

/// Delete the snapshot for `instance_id`.
///
/// Called when an instance reaches a terminal state and its log is archived.
///
/// # Errors
/// Returns [`WtfError::SledError`] on I/O failure.
pub fn delete_snapshot(db: &sled::Db, instance_id: &InstanceId) -> Result<(), WtfError> {
    let tree = db
        .open_tree(SNAPSHOTS_TREE)
        .map_err(|e| WtfError::sled_error(format!("open tree: {e}")))?;

    tree.remove(instance_id.as_str().as_bytes())
        .map_err(|e| WtfError::sled_error(format!("remove {instance_id}: {e}")))?;

    Ok(())
}

fn crc32_of(data: &[u8]) -> u32 {
    let mut hasher = Crc32Hasher::new();
    hasher.update(data);
    hasher.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_db() -> (tempfile::TempDir, sled::Db) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = open_snapshot_db(dir.path()).expect("open db");
        (dir, db)
    }

    fn test_record(seq: u64) -> SnapshotRecord {
        SnapshotRecord::new(seq, Bytes::from_static(b"state-bytes"))
    }

    #[test]
    fn snapshot_record_new_checksum_is_valid() {
        let record = test_record(42);
        assert!(record.is_valid());
    }

    #[test]
    fn snapshot_record_corrupted_checksum_is_invalid() {
        let mut record = test_record(42);
        record.checksum = record.checksum.wrapping_add(1);
        assert!(!record.is_valid());
    }

    #[test]
    fn write_then_read_roundtrips() {
        let (_dir, db) = make_db();
        let id = InstanceId::new("01ARZ");
        let record = test_record(100);

        write_snapshot(&db, &id, &record).expect("write");
        let loaded = read_snapshot(&db, &id).expect("read").expect("present");

        assert_eq!(loaded.seq, 100);
        assert_eq!(loaded.state_bytes, record.state_bytes);
        assert_eq!(loaded.checksum, record.checksum);
    }

    #[test]
    fn read_missing_returns_none() {
        let (_dir, db) = make_db();
        let id = InstanceId::new("does-not-exist");
        let result = read_snapshot(&db, &id).expect("no error");
        assert!(result.is_none());
    }

    #[test]
    fn corrupted_snapshot_returns_none() {
        let (_dir, db) = make_db();
        let id = InstanceId::new("corrupt");
        let mut record = test_record(55);
        record.checksum = 0xDEAD_BEEF; // wrong checksum

        write_snapshot(&db, &id, &record).expect("write");
        let result = read_snapshot(&db, &id).expect("no io error");
        assert!(result.is_none(), "corrupt snapshot should return None");
    }

    #[test]
    fn delete_removes_snapshot() {
        let (_dir, db) = make_db();
        let id = InstanceId::new("to-delete");
        let record = test_record(1);

        write_snapshot(&db, &id, &record).expect("write");
        delete_snapshot(&db, &id).expect("delete");
        let result = read_snapshot(&db, &id).expect("read");
        assert!(result.is_none());
    }

    #[test]
    fn write_overwrites_previous_snapshot() {
        let (_dir, db) = make_db();
        let id = InstanceId::new("overwrite");

        write_snapshot(&db, &id, &test_record(10)).expect("write first");
        write_snapshot(&db, &id, &test_record(20)).expect("write second");

        let loaded = read_snapshot(&db, &id).expect("read").expect("present");
        assert_eq!(loaded.seq, 20, "should have the latest snapshot");
    }

    #[test]
    fn two_instances_do_not_interfere() {
        let (_dir, db) = make_db();
        let id_a = InstanceId::new("instance-a");
        let id_b = InstanceId::new("instance-b");

        write_snapshot(&db, &id_a, &test_record(1)).expect("write a");
        write_snapshot(&db, &id_b, &test_record(2)).expect("write b");

        let a = read_snapshot(&db, &id_a).expect("read").expect("present");
        let b = read_snapshot(&db, &id_b).expect("read").expect("present");

        assert_eq!(a.seq, 1);
        assert_eq!(b.seq, 2);
    }
}
