#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_types::SequenceNumber;

/// Fuzz target for SequenceNumber parsing.
///
/// Input type: &str (arbitrary strings)
/// Risk class: Panic/InvalidFormat/Overflow
/// Tests sequence number string parsing
///
/// Corpus seeds:
/// - Empty string
/// - Negative numbers
/// - Zero
/// - Max u64
/// - Beyond max u64
/// - Very long strings
fuzz_target!(|data: &str| {
    let _ = SequenceNumber::parse(data);
});
