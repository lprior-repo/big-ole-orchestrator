//! Unit tests for the `ScanIterator` adapter.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]

use super::*;

fn make_test_instance_id(byte_fill: u8) -> InstanceId {
    InstanceId::from_bytes([byte_fill; 16])
}

fn make_test_timestamp(ms: u64) -> TimestampMs {
    TimestampMs::try_from(ms).unwrap()
}

fn setup_partition() -> (tempfile::TempDir, fjall::Database, fjall::Keyspace) {
    let dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(dir.path()).open().unwrap();
    let partition = db
        .keyspace("instances", fjall::KeyspaceCreateOptions::default)
        .unwrap();
    (dir, db, partition)
}

#[test]
fn scan_iterator_yields_init_error_on_first_next_then_none() {
    let mut iter = ScanIterator {
        inner: None,
        init_error: Some(StorageError::Storage),
    };

    let first = iter.next();
    assert_eq!(first, Some(Err(StorageError::Storage)));

    let second = iter.next();
    assert_eq!(second, None);
}

#[test]
fn scan_iterator_decodes_valid_entries_from_real_partition() {
    let (_dir, _db, partition) = setup_partition();
    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(500);
    let key = encode_instance_index_key(InstanceStatus::Running, ts, &id).unwrap();

    partition.insert(key, Vec::new()).unwrap();

    let iter = partition.iter();
    let mut scan = ScanIterator {
        inner: Some(Box::new(iter)),
        init_error: None,
    };

    let first = scan.next();
    let entry = first.unwrap().unwrap();
    assert_eq!(entry.instance_id, id);
    assert_eq!(entry.status, InstanceStatus::Running);
    assert_eq!(entry.created_at, ts);

    let second = scan.next();
    assert_eq!(second, None);
}
