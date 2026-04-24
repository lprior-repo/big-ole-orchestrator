#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_types::TimerId;

/// Fuzz target for TimerId parsing.
///
/// Input type: &str (arbitrary UTF-8 strings)
/// Risk class: Panic/InvalidFormat/ExceedsMaxLength
/// Tests timer identifier validation
///
/// Corpus seeds:
/// - Empty string
/// - Single char
/// - Exactly 32 chars (max)
/// - 33 chars (exceeds max)
/// - Consecutive hyphens
/// - Invalid characters
/// - Unicode
fuzz_target!(|data: &str| {
    let _result = TimerId::parse(data);
});
