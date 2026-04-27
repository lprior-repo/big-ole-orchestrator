//! Section 6: ContentAddress validation

use vo_storage::blob_store::{BlobStoreError, ContentAddress};

#[test]
fn content_address_rejects_wrong_length() {
    let result = ContentAddress::new("too_short");
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        BlobStoreError::InvalidArgument { .. }
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
