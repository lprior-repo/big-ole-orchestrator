//! Test helpers for instance index Red Queen tests.

use vo_storage::codec::StorageError;
use vo_storage::instance_index::InstanceIndexEntry;

pub fn make_test_keyspace() -> (tempfile::TempDir, fjall::Database) {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");
    let database = fjall::Database::builder(dir.path())
        .open()
        .expect("Failed to open database");
    (dir, database)
}

pub fn make_test_instance_id(byte_fill: u8) -> vo_types::InstanceId {
    vo_types::InstanceId::from_bytes([byte_fill; 16])
}

pub fn make_unique_instance_id(index: u16) -> vo_types::InstanceId {
    let mut bytes = [0x01u8; 16];
    let idx_bytes = index.to_be_bytes();
    bytes[0] = idx_bytes[0];
    bytes[1] = idx_bytes[1];
    vo_types::InstanceId::from_bytes(bytes)
}

pub fn make_test_timestamp(ms: u64) -> vo_types::TimestampMs {
    vo_types::TimestampMs::try_from(ms).unwrap()
}

pub fn seed_instance(
    database: &fjall::Database,
    id: &vo_types::InstanceId,
    status: vo_types::InstanceStatus,
    ts: vo_types::TimestampMs,
) {
    use vo_storage::instance_index::instance_index_upsert;
    instance_index_upsert(database, id, status, ts, None).unwrap();
}

pub fn collect_scan_ok(
    iter: impl Iterator<Item = Result<InstanceIndexEntry, StorageError>>,
) -> Vec<InstanceIndexEntry> {
    iter.map(|r| r.expect("expected Ok entry")).collect::<Vec<_>>()
}
