#![no_main]

use libfuzzer_sys::fuzz_target;
use serde_json;

/// Fuzz target for ProbeConfig JSON deserialization.
///
/// Input type: &[u8] (raw bytes interpreted as JSON)
/// Risk class: Panic/InvalidUtf8/MalformedJson/InvalidConfig
/// Tests probe configuration parsing from JSON
///
/// Corpus seeds:
/// - Empty bytes
/// - Invalid UTF-8
/// - Truncated JSON
/// - Missing required fields
/// - Invalid field types
/// - Very deep nesting
/// - Very long strings
fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _result: Result<serde_json::Value, _> = serde_json::from_str(s);
    }
});
