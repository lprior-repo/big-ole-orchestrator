#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_types::EncryptionKey;

/// Fuzz target for EncryptionKey parsing.
///
/// Input type: &str (arbitrary strings)
/// Risk class: Panic/InvalidFormat/InvalidLength
/// Tests encryption key string parsing
///
/// Corpus seeds:
/// - Empty string
/// - Too short
/// - Too long
/// - Invalid characters
/// - Very long strings
fuzz_target!(|data: &str| {
    let _ = EncryptionKey::parse(data);
});
