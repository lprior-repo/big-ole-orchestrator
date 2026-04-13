#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_actor::Port;

/// Fuzz target for Port parsing.
///
/// Input type: &str (arbitrary strings)
/// Risk class: Panic/InvalidFormat/InvalidPort
/// Tests port number parsing and validation
///
/// Corpus seeds:
/// - Empty string
/// - Negative numbers
/// - Zero
/// - Valid port (1-65535)
/// - Invalid port (>65535)
/// - Very long strings
fuzz_target!(|data: &str| {
    let _ = Port::parse(data);
});
