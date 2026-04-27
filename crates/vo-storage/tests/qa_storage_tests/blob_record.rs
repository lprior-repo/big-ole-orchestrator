//! Section 7: BlobRecord invariants

use vo_storage::blob_store::{BlobRecord, ContentAddress};

fn make_addr() -> ContentAddress {
    ContentAddress::new("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
        .unwrap()
}

#[test]
fn blob_record_rejects_zero_ref_count() {
    let addr = make_addr();
    let result = BlobRecord::new(addr.clone(), 100, 0, 1000, None);
    assert!(result.is_err());
}

#[test]
fn blob_record_rejects_zero_created_at() {
    let addr = make_addr();
    let result = BlobRecord::new(addr.clone(), 100, 1, 0, None);
    assert!(result.is_err());
}

#[test]
fn blob_record_gc_eligible_when_expired_and_zero_refs() {
    let addr = make_addr();
    let record = BlobRecord::with_status(
        addr,
        100,
        0,
        1000,
        Some(2000),
        vo_types::BlobStatus::DurablyStored,
    );

    assert!(record.is_expired(2000));
    assert!(record.is_gc_eligible(2000));
    assert!(!record.is_gc_eligible(1999));
}

#[test]
fn blob_record_not_gc_eligible_with_refs() {
    let addr = make_addr();
    let record = BlobRecord::with_status(
        addr,
        100,
        1,
        1000,
        Some(2000),
        vo_types::BlobStatus::DurablyStored,
    );

    assert!(record.is_expired(2000));
    assert!(!record.is_gc_eligible(2000));
}

#[test]
fn blob_record_saturating_ref_count_ops() {
    let addr = make_addr();
    let record = BlobRecord::with_status(
        addr,
        100,
        1,
        1000,
        None,
        vo_types::BlobStatus::DurablyStored,
    );

    assert_eq!(record.increment_ref_count(), 2);
    assert_eq!(record.decrement_ref_count(), 0);
    assert_eq!(record.decrement_ref_count(), 0);
}
