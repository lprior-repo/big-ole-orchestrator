use super::*;

/// K-01: Verify DedupeEntry::new rejects empty dedupe_key.
#[kani::proof]
fn verify_dedupe_entry_rejects_empty_key_returns_invalid_argument() {
    let result = DedupeEntry::new("".to_string(), "instance-1".to_string(), 1000);
    assert_eq!(result, Err(DedupeStoreError::InvalidArgument));
}

/// K-02: Verify encode/decode dedupe key round-trip for valid UTF-8.
#[kani::proof]
fn verify_encode_decode_dedupe_key_roundtrip_returns_original_key() {
    // Test with concrete valid UTF-8 string
    let test_string = "valid_dedupe_key_123";
    let parsed = DedupeKey::parse(test_string);
    assert!(parsed.is_ok());
    let dedupe_key = parsed.unwrap();
    let encoded = encode_dedupe_key(&dedupe_key);
    assert_eq!(decode_dedupe_key(&encoded), Ok(dedupe_key));
}
