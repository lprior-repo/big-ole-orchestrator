//! Unit tests for the `ScanIterator` adapter.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]

use super::*;

// ---- Test helpers ----

fn make_test_instance_id(byte_fill: u8) -> InstanceId {
    InstanceId::from_bytes([byte_fill; 16])
}

fn make_test_timestamp(ms: u64) -> TimestampMs {
    TimestampMs::try_from(ms).unwrap()
}

// ---- B33u: ScanIterator yields init_error on first next() then terminates ----

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

// ---- B34u: ScanIterator yields StorageError::Storage when inner iterator errors ----

#[test]
fn scan_iterator_yields_storage_error_when_inner_iterator_returns_err() {
    let mut iter = ScanIterator {
        inner: Some(Box::new(
            vec![Err(fjall::Error::Poisoned) as fjall::Result<fjall::KvPair>].into_iter(),
        )),
        init_error: None,
    };

    let first = iter.next();
    assert_eq!(first, Some(Err(StorageError::Storage)));
}

// ---- B35u: ScanIterator stops after storage error (self.inner = None) ----

#[test]
fn scan_iterator_stops_after_storage_error() {
    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(1000);
    let valid_key = encode_instance_index_key(InstanceStatus::Pending, ts, &id).unwrap();
    let valid_kv_pair: fjall::KvPair = (
        fjall::Slice::from(valid_key.to_vec()),
        fjall::Slice::from(Vec::new()),
    );

    let mut iter = ScanIterator {
        inner: Some(Box::new(
            vec![
                Err(fjall::Error::Poisoned) as fjall::Result<fjall::KvPair>,
                Ok(valid_kv_pair),
            ]
            .into_iter(),
        )),
        init_error: None,
    };

    let first = iter.next();
    assert_eq!(first, Some(Err(StorageError::Storage)));

    // Must be None because inner was set to None (MUTATION KILL target)
    let second = iter.next();
    assert_eq!(
        second, None,
        "Iterator must terminate after StorageError::Storage"
    );
}

// ---- B36u: ScanIterator correctly decodes valid entries from inner iterator ----

#[test]
fn scan_iterator_decodes_valid_entries_from_inner_iterator() {
    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(500);
    let key = encode_instance_index_key(InstanceStatus::Running, ts, &id).unwrap();
    let kv_pair: fjall::KvPair = (
        fjall::Slice::from(key.to_vec()),
        fjall::Slice::from(Vec::new()),
    );

    let mut iter = ScanIterator {
        inner: Some(Box::new(vec![Ok(kv_pair)].into_iter())),
        init_error: None,
    };

    let first = iter.next();
    let entry = first.unwrap().unwrap();
    assert_eq!(entry.instance_id, id);
    assert_eq!(entry.status, InstanceStatus::Running);
    assert_eq!(entry.created_at, ts);

    let second = iter.next();
    assert_eq!(second, None);
}
