#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_types::CommandHistory;

/// Fuzz target for CommandHistory parsing.
///
/// Input type: &str (arbitrary strings)
/// Risk class: Panic/InvalidFormat/InvalidChars
/// Tests command history string parsing
///
/// Corpus seeds:
/// - Empty string
/// - Invalid separator
/// - Invalid character sequences
/// - Very long strings
fuzz_target!(|data: &str| {
    let _result = CommandHistory::parse(data);
});
