#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_types::BinaryHash;

/// Fuzz target for BinaryHash parsing.
///
/// Input type: &str (arbitrary UTF-8 strings)
/// Risk class: Panic/InvalidHex/ExceedsMaxLength
/// Tests hex string validation for binary hashes
///
/// Corpus seeds:
/// - Empty string
/// - Odd length (invalid hex)
/// - Invalid hex chars (g-z except a-f)
/// - Exactly 64 chars (valid SHA-256)
/// - 65 chars (exceeds max)
/// - Mixed case
/// - Unicode
fuzz_target!(|data: &str| {
    let _result = BinaryHash::parse(data);
});
