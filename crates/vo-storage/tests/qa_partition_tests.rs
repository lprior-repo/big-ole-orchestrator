//! QA tests for vo-storage: Partition layout, ContentAddress, BlobRecord.
//!
//! All tests use real Fjall instances in temp directories. No mocks.

use vo_storage::blob_store::{BlobRecord, ContentAddress};
use vo_storage::partitions::{
    create_partition_layout, open_all_partitions, ALL_PARTITIONS, BLOB_PARTITIONS,
    COLD_PARTITIONS, HOT_PARTITIONS,
};
use vo_types::BlobStatus;

// ══════════════════════════════════════════════════════════════════════════════
// Section 5: Partition Layout
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn create_partition_layout_opens_fjall_database() {
    let dir = tempfile::tempdir().expect("tempdir");
    let layout = create_partition_layout(dir.path()).expect("layout");
    assert!(dir.path().exists());
    let _db = layout.db();
}

#[test]
fn open_all_partitions_opens_every_defined_partition() {
    let dir = tempfile::tempdir().expect("tempdir");
    let layout = create_partition_layout(dir.path()).expect("layout");

    let partitions = open_all_partitions(&layout).expect("open all");
    assert_eq!(partitions.len(), ALL_PARTITIONS.len());

    let names: Vec<&str> = partitions.iter().map(|(n, _)| *n).collect();
    for expected in ALL_PARTITIONS {
        assert!(names.contains(expected), "missing partition: {expected}");
    }
}

#[test]
fn partition_class_counts_match_constants() {
    let hot = HOT_PARTITIONS.len();
    let cold = COLD_PARTITIONS.len();
    let blob = BLOB_PARTITIONS.len();
    assert_eq!(hot + cold + blob + 1, ALL_PARTITIONS.len());
}

#[test]
fn storage_engine_opens_with_all_stores() {
    let dir = tempfile::tempdir().expect("tempdir");
    let engine = vo_storage::partitions::StorageEngine::open(dir.path()).expect("engine open");
    let _db = engine.db();
}

// ══════════════════════════════════════════════════════════════════════════════
// Section 6: ContentAddress validation
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn content_address_rejects_wrong_length() {
    let result = ContentAddress::new("too_short");
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        vo_storage::blob_store::BlobStoreError::InvalidArgument { .. }
    ));
}

#[test]
fn content_address_rejects_uppercase_hex() {
    let result =
        ContentAddress::new("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
    assert!(result.is_err());
}

#[test]
fn content_address_rejects_non_hex_chars() {
    let result =
        ContentAddress::new("zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz");
    assert!(result.is_err());
}

#[test]
fn content_address_accepts_valid_sha256_hex() {
    let result =
        ContentAddress::new("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    assert!(result.is_ok());
}

#[test]
fn content_address_from_bytes_roundtrip() {
    let bytes = [0xABu8; 32];
    let addr = ContentAddress::from_bytes(&bytes);
    assert_eq!(addr.as_str().len(), 64);

    let recovered = addr.as_bytes();
    assert_eq!(recovered, bytes);
}

// ══════════════════════════════════════════════════════════════════════════════
// Section 7: BlobRecord invariants
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn blob_record_rejects_zero_ref_count() {
    let addr =
        ContentAddress::new("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
            .unwrap();
    let result = BlobRecord::new(addr.clone(), 100, 0, 1000, None);
    assert!(result.is_err());
}

#[test]
fn blob_record_rejects_zero_created_at() {
    let addr =
        ContentAddress::new("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
            .unwrap();
    let result = BlobRecord::new(addr.clone(), 100, 1, 0, None);
    assert!(result.is_err());
}

#[test]
fn blob_record_gc_eligible_when_expired_and_zero_refs() {
    let addr =
        ContentAddress::new("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
            .unwrap();
    let record = BlobRecord::with_status(
        addr,
        100,
        0,
        1000,
        Some(2000),
        BlobStatus::DurablyStored,
    );

    assert!(record.is_expired(2000));
    assert!(record.is_gc_eligible(2000));
    assert!(!record.is_gc_eligible(1999));
}

#[test]
fn blob_record_not_gc_eligible_with_refs() {
    let addr =
        ContentAddress::new("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
            .unwrap();
    let record = BlobRecord::with_status(
        addr,
        100,
        1,
        1000,
        Some(2000),
        BlobStatus::DurablyStored,
    );

    assert!(record.is_expired(2000));
    assert!(!record.is_gc_eligible(2000));
}

#[test]
fn blob_record_saturating_ref_count_ops() {
    let addr =
        ContentAddress::new("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
            .unwrap();
    let record = BlobRecord::with_status(
        addr,
        100,
        1,
        1000,
        None,
        BlobStatus::DurablyStored,
    );

    assert_eq!(record.increment_ref_count(), 2);
    assert_eq!(record.decrement_ref_count(), 0);
    assert_eq!(record.decrement_ref_count(), 0);
}