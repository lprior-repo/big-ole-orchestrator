#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_types::SpawnId;

/// Fuzz target for SpawnId parsing.
///
/// Input type: &str (arbitrary UTF-8 strings)
/// Risk class: Panic/InvalidFormat/ExceedsMaxLength
/// Tests spawn identifier validation
///
/// Corpus seeds:
/// - Empty string
/// - Single char
/// - Exactly 26 chars (max)
/// - 27 chars (exceeds max)
/// - Consecutive hyphens
/// - Invalid characters
/// - Unicode
fuzz_target!(|data: &str| {
    let _result = SpawnId::parse(data);
});
