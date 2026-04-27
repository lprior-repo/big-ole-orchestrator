#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_actor::probe::ProbeId;

/// Fuzz target for ProbeId parsing.
///
/// Input type: &str (arbitrary strings)
/// Risk class: Panic/InvalidFormat/InvalidChars
/// Tests probe identifier parsing
///
/// Corpus seeds:
/// - Empty string
/// - Invalid probe ID format
/// - Very long strings
/// - Unicode
fuzz_target!(|data: &str| {
    let _ = ProbeId::parse(data);
});
