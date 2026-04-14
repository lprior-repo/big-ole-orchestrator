#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_types::DedupeKey;

/// Fuzz target for DedupeKey parsing.
///
/// Input type: &str (arbitrary strings)
/// Risk class: Panic/InvalidFormat/EmptyKey
/// Tests deduplication key parsing
///
/// Corpus seeds:
/// - Empty string
/// - Single char
/// - Very long strings
/// - Unicode
fuzz_target!(|data: &str| {
    let _ = DedupeKey::parse(data);
});
