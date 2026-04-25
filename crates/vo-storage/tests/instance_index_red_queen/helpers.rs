use vo_storage::codec::StorageError;
use vo_storage::instance_index::{
    decode_instance_index_key, encode_instance_index_key, instance_index_upsert,
    scan_all_instances, scan_by_status, InstanceIndexEntry,
};
use vo_types::{InstanceId, InstanceStatus, TimestampMs};

pub fn make_test_keyspace() -> (tempfile::TempDir, fjall::Database) {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");
    let database = fjall::Database::builder(dir.path())
        .open()
        .expect("Failed to open database");
    (dir, database)
}

pub fn make_test_instance_id(byte_fill: u8) -> InstanceId {
    InstanceId::from_bytes([byte_fill; 16])
}

pub fn make_unique_instance_id(index: u16) -> InstanceId {
    let mut bytes = [0x01u8; 16];
    let idx_bytes = index.to_be_bytes();
    bytes[0] = idx_bytes[0];
    bytes[1] = idx_bytes[1];
    InstanceId::from_bytes(bytes)
}

pub fn make_test_timestamp(ms: u64) -> TimestampMs {
    TimestampMs::try_from(ms).unwrap()
}

pub fn seed_instance(
    database: &fjall::Database,
    id: &InstanceId,
    status: InstanceStatus,
    ts: TimestampMs,
) {
    instance_index_upsert(database, id, status, ts, None).unwrap();
}

pub fn collect_scan_ok(
    iter: impl Iterator<Item = Result<InstanceIndexEntry, StorageError>>,
) -> Vec<InstanceIndexEntry> {
    iter.map(|r| r.expect("expected Ok entry"))
        .collect::<Vec<_>>()
}