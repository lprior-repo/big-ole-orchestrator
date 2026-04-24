#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_types::SuffixArray;

/// Fuzz target for SuffixArray parsing.
///
/// Input type: &str (arbitrary strings)
/// Risk class: Panic/InvalidFormat/InvalidChars
/// Tests suffix array string parsing
///
/// Corpus seeds:
/// - Empty string
/// - Invalid characters
/// - Wrong length
/// - Very long strings
fuzz_target!(|data: &str| {
    let _ = SuffixArray::from_str(data);
});
