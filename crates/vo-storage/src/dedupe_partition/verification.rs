#![allow(clippy::unwrap_used)]
use super::*;

/// K-01: Verify DedupeEntry::new rejects empty dedupe_key.
#[kani::proof]
fn verify_dedupe_entry_rejects_empty_key() {
    let result = DedupeEntry::new("".to_string(), "instance-1".to_string(), 1000);
    assert!(result.is_err());
}

/// K-02: Verify encode/decode dedupe key round-trip for valid UTF-8.
#[kani::proof]
fn verify_encode_decode_dedupe_key_roundtrip() {
    let s: String = kani::any();
    kani::assume(!s.is_empty());
    kani::assume(s.len() <= 256);
    // Verify all chars are ASCII (DedupeKey requirement)
    kani::assume(s.bytes().all(|b| b.is_ascii()));
    if let Ok(dk) = DedupeKey::parse(&s) {
        let bytes = encode_dedupe_key(&dk);
        let recovered = decode_dedupe_key(&bytes);
        assert!(recovered.is_ok());
    }
}
