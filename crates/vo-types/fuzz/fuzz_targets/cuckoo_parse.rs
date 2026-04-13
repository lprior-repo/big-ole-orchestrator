#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_types::CuckooFilter;

/// Fuzz target for CuckooFilter parsing.
///
/// Input type: &str (arbitrary strings)
/// Risk class: Panic/InvalidBase58/InvalidLength
/// Tests cuckoo filter base58 string parsing
///
/// Corpus seeds:
/// - Empty string
/// - Invalid base58 chars
/// - Wrong length
/// - All same chars
/// - Very long strings
fuzz_target!(|data: &str| {
    let _ = CuckooFilter::parse(data);
});
