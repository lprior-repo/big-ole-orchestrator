#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_types::WriteCounter;

/// Fuzz target for WriteCounter parsing.
///
/// Input type: &str (arbitrary strings)
/// Risk class: Panic/InvalidFormat/Overflow
/// Tests write counter string parsing
///
/// Corpus seeds:
/// - Empty string
/// - Negative numbers
/// - Zero
/// - Max u64
/// - Beyond max u64
fuzz_target!(|data: &str| {
    let _ = WriteCounter::parse(data);
});
