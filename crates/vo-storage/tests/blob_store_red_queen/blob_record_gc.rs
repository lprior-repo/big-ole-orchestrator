use crate::helpers::{make_blob_record, make_content_addr};

#[test]
fn red_queen_blob_record_with_nonzero_ref_count_not_gc_eligible() {
    let record = make_blob_record(1);
    assert_eq!(record.reference_count(), 1, "ref_count must be 1");
    assert!(
        !record.is_gc_eligible(2000),
        "record with ref_count=1 must not be GC eligible regardless of expiry"
    );
}

#[test]
fn red_queen_blob_record_with_high_ref_count_not_gc_eligible() {
    let content_addr = make_content_addr();
    let record = vo_storage::blob_store::BlobRecord::new(content_addr, 1024, 100, 1000, Some(1500)).unwrap();
    assert!(
        !record.is_gc_eligible(3000),
        "ref_count=100 must prevent GC even when expired"
    );
}

#[test]
fn red_queen_blob_record_zero_ref_count_expired_is_gc_eligible() {
    let content_addr = make_content_addr();
    let record = vo_storage::blob_store::BlobRecord::new(content_addr, 1024, 0, 1000, Some(1500));
    assert!(
        record.is_err(),
        "ref_count=0 should not be allowed on construction"
    );
}

#[test]
fn red_queen_blob_record_increment_ref_count_saturates() {
    let record = make_blob_record(u64::MAX);
    let new_count = record.increment_ref_count();
    assert_eq!(new_count, u64::MAX, "increment must saturate at MAX");
}

#[test]
fn red_queen_blob_record_decrement_ref_count_saturates_at_zero() {
    let record = make_blob_record(1);
    let new_count = record.decrement_ref_count();
    assert_eq!(new_count, 0, "decrement must saturate at 0");
}

#[test]
fn red_queen_blob_record_decrement_from_zero_saturates() {
    let content_addr = make_content_addr();
    let record = vo_storage::blob_store::BlobRecord::new(content_addr, 1024, 0, 1000, None);
    assert!(record.is_err(), "ref_count=0 is invalid on construction");
}

#[test]
fn red_queen_blob_record_expires_at_none_never_expires() {
    let content_addr = make_content_addr();
    let record = vo_storage::blob_store::BlobRecord::new(content_addr, 1024, 1, 1000, None).unwrap();
    assert!(
        !record.is_expired(u64::MAX),
        "expires_at=None must mean never expires"
    );
}

#[test]
fn red_queen_blob_record_expires_at_boundary() {
    let content_addr = make_content_addr();
    let record = vo_storage::blob_store::BlobRecord::new(content_addr, 1024, 1, 1000, Some(1500)).unwrap();
    assert!(!record.is_expired(1499), "1499 < 1500 → not expired");
    assert!(record.is_expired(1500), "1500 >= 1500 → expired");
}