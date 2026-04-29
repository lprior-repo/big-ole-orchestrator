#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_types::CommandEnvelope;

/// Fuzz target for CommandEnvelope parsing.
///
/// Input type: &str (arbitrary strings)
/// Risk class: Panic/InvalidFormat/MissingFields
/// Tests command envelope string parsing
///
/// Corpus seeds:
/// - Empty string
/// - Malformed envelope format
/// - Invalid separator positions
/// - Missing instance_id
/// - Invalid sequence numbers
/// - Very long strings
fuzz_target!(|data: &str| {
    let _result = CommandEnvelope::from_str(data);
});
