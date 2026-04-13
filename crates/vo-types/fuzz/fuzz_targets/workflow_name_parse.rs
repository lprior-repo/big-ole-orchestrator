#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_types::WorkflowName;

/// Fuzz target for WorkflowName parsing.
///
/// Input type: &str (arbitrary UTF-8 strings)
/// Risk class: Panic/InvalidCharacters/ExceedsMaxLength
/// Tests identifier validation, consecutive hyphen checks
///
/// Corpus seeds:
/// - Empty string
/// - Single char
/// - Exactly 128 chars (max)
/// - 129 chars (exceeds max)
/// - Consecutive hyphens (--, -_, _-)
/// - Invalid characters (spaces, dots, @, etc.)
/// - Unicode identifiers
/// - Very long strings (1KB+)
fuzz_target!(|data: &str| {
    let _result = WorkflowName::parse(data);
});
