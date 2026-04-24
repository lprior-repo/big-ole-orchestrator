#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_types::NodeId;

/// Fuzz target for NodeId parsing.
///
/// Input type: &str (arbitrary UTF-8 strings)
/// Risk class: Panic/InvalidCharacters/ExceedsMaxLength
/// Tests node identifier validation
///
/// Corpus seeds:
/// - Empty string
/// - Single char
/// - Exactly 64 chars (max)
/// - 65 chars (exceeds max)
/// - Consecutive hyphens
/// - Invalid characters
/// - Unicode
/// - Very long strings
fuzz_target!(|data: &str| {
    let _result = NodeId::parse(data);
});
