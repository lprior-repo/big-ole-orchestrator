use crate::helpers::{make_content_addr, VALID_SHA256};
use vo_storage::blob_store::ContentAddress;

#[test]
fn red_queen_content_address_valid_sha256_roundtrip() {
    let addr = make_content_addr();
    let bytes = addr.as_bytes();
    let roundtrip = ContentAddress::from_bytes(&bytes);
    assert_eq!(roundtrip.as_str(), VALID_SHA256);
}

#[test]
fn red_queen_content_address_rejects_uppercase() {
    let upper = "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789";
    let result = ContentAddress::new(upper);
    assert!(result.is_err(), "ContentAddress must reject uppercase hex");
}

#[test]
fn red_queen_content_address_rejects_wrong_length() {
    let short = "abc123";
    let result = ContentAddress::new(short);
    assert!(
        result.is_err(),
        "ContentAddress must reject non-64-char strings"
    );
}

#[test]
fn red_queen_content_address_rejects_non_hex() {
    let non_hex = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15g0f00a08";
    let result = ContentAddress::new(non_hex);
    assert!(
        result.is_err(),
        "ContentAddress must reject non-hex characters"
    );
}

#[test]
fn red_queen_content_address_empty_rejected() {
    let result = ContentAddress::new("");
    assert!(result.is_err(), "ContentAddress must reject empty string");
}