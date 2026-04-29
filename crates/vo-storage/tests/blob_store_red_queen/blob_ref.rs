use crate::helpers::make_blob_ref;
use vo_types::BlobRef;

#[test]
fn red_queen_blobref_valid_construction() {
    let blob = make_blob_ref();
    assert_eq!(blob.blob_id(), "01H5JQX7K3R4T6V8W0X2Y4Z6A8");
    assert_eq!(blob.size_bytes(), 1024);
    assert_eq!(blob.content_hash(), "abcdef0123456789abcdef0123456789");
}

#[test]
fn red_queen_blobref_rejects_empty_blob_id() {
    let result = BlobRef::new("", 1024, "abcdef0123456789abcdef0123456789");
    assert!(result.is_err(), "blob_id cannot be empty");
}

#[test]
fn red_queen_blobref_rejects_invalid_ulid() {
    let result = BlobRef::new("not-a-ulid", 1024, "abcdef0123456789abcdef0123456789");
    assert!(result.is_err(), "blob_id must be valid ULID");
}

#[test]
fn red_queen_blobref_rejects_wrong_length_blob_id() {
    let result = BlobRef::new("01H5JQX7K3", 1024, "abcdef0123456789abcdef0123456789");
    assert!(result.is_err(), "blob_id must be exactly 26 chars");
}

#[test]
fn red_queen_blobref_rejects_zero_size() {
    let result = BlobRef::new(
        "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
        0,
        "abcdef0123456789abcdef0123456789",
    );
    assert!(result.is_err(), "size_bytes cannot be zero");
}

#[test]
fn red_queen_blobref_rejects_empty_content_hash() {
    let result = BlobRef::new("01H5JQX7K3R4T6V8W0X2Y4Z6A8", 1024, "");
    assert!(result.is_err(), "content_hash cannot be empty");
}

#[test]
fn red_queen_blobref_rejects_non_hex_content_hash() {
    let result = BlobRef::new("01H5JQX7K3R4T6V8W0X2Y4Z6A8", 1024, "ghijklmnopqrstuv");
    assert!(result.is_err(), "content_hash must be lowercase hex");
}

#[test]
fn red_queen_blobref_rejects_odd_length_content_hash() {
    let result = BlobRef::new("01H5JQX7K3R4T6V8W0X2Y4Z6A8", 1024, "abcde");
    assert!(result.is_err(), "content_hash must have even length");
}

#[test]
fn red_queen_blobref_rejects_short_content_hash() {
    let result = BlobRef::new("01H5JQX7K3R4T6V8W0X2Y4Z6A8", 1024, "ab");
    assert!(result.is_err(), "content_hash must be at least 8 chars");
}